//! 表结构与迁移。
//!
//! M0 只需要两张表:`providers`(供应商配置)和 `settings`(KV 杂项)。
//! 注意 `providers` 表刻意**不存 API Key 明文**,只有 `keychain_id` 引用。
//!
//! v2:供应商升级为跨分组的逻辑实体,端点按协议存入 `endpoints`
//! JSON 列({anthropic|openai|gemini: base_url}),同名旧行合并。

use anyhow::Result;
use rusqlite::{params, Connection};

/// 当前 schema 版本,用于未来迁移校验。
#[allow(dead_code)]
pub const SCHEMA_VERSION: u32 = 3;

pub fn create_tables(conn: &Connection) -> Result<()> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS providers (
            id          TEXT PRIMARY KEY,
            app_type    TEXT NOT NULL,
            name        TEXT NOT NULL,
            base_url    TEXT NOT NULL,
            -- keychain 引用标识;真实 Key 不入库,只存这里作为取用凭证
            keychain_id TEXT,
            models      TEXT NOT NULL DEFAULT '[]',
            is_current  INTEGER NOT NULL DEFAULT 0,
            is_healthy  INTEGER NOT NULL DEFAULT 1,
            sort_index  INTEGER NOT NULL DEFAULT 0,
            created_at  INTEGER NOT NULL DEFAULT 0,
            meta        TEXT NOT NULL DEFAULT '{}',
            endpoints   TEXT NOT NULL DEFAULT '{}'
        );
        CREATE INDEX IF NOT EXISTS idx_providers_app     ON providers(app_type);
        CREATE INDEX IF NOT EXISTS idx_providers_current ON providers(app_type, is_current);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- M2:代理层用量计量(每次请求一行,流结束落库)
        CREATE TABLE IF NOT EXISTS usage_log (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            app_type      TEXT NOT NULL,
            provider_id   TEXT NOT NULL,
            model         TEXT,
            input_tokens  INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            -- v3:上游 HTTP 状态码(<400 视为成功;旧行默认 200)
            status        INTEGER NOT NULL DEFAULT 200,
            created_at    INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_usage_provider ON usage_log(provider_id);

        -- v2:每个分组的当前供应商(合并后一行供应商可能是多个分组的当前)
        CREATE TABLE IF NOT EXISTS current_map (
            app_type    TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL
        );
        "#,
    )?;

    if version < 2 {
        migrate_v1_to_v2(conn)?;
    }
    if version < 3 {
        migrate_v2_to_v3(conn)?;
    }
    conn.execute_batch("PRAGMA user_version = 3;")?;
    Ok(())
}

/// v2 → v3:usage_log 增加 status 列(上游 HTTP 状态码)。
fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    let has_status = conn.prepare("SELECT status FROM usage_log LIMIT 1").is_ok();
    if !has_status {
        conn.execute_batch(
            "ALTER TABLE usage_log ADD COLUMN status INTEGER NOT NULL DEFAULT 200;",
        )?;
    }
    Ok(())
}

/// v1 → v2:新增 endpoints 列;按名称(大小写/首尾空白归一)合并同名供应商,
/// 每条旧行的 base_url 按其 app_type 对应协议写入主体的 endpoints;
/// 各分组 `is_current` 指向合并后的主体;keychain 只保留主体的引用。
/// 注意:调用前已确认 user_version < 2,且外层(db/mod.rs)在启动时仅执行一次。
fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    // 旧库可能没有 endpoints 列(新装库由 CREATE TABLE 直接带上)
    let has_endpoints = conn.prepare("SELECT endpoints FROM providers LIMIT 1").is_ok();
    if !has_endpoints {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN endpoints TEXT NOT NULL DEFAULT '{}';")?;
    }

    #[derive(Debug)]
    struct OldRow {
        id: String,
        app_type: String,
        name: String,
        base_url: String,
        is_current: bool,
    }
    let rows: Vec<OldRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, app_type, name, base_url, is_current FROM providers ORDER BY created_at, sort_index",
        )?;
        let iter = stmt.query_map([], |r| {
            Ok(OldRow {
                id: r.get(0)?,
                app_type: r.get(1)?,
                name: r.get(2)?,
                base_url: r.get(3)?,
                is_current: r.get::<_, i64>(4)? != 0,
            })
        })?;
        iter.collect::<std::result::Result<_, _>>()?
    };

    let normalize = |s: &str| s.trim().to_lowercase();
    let protocol_of = |app: &str| -> &'static str {
        match crate::types::AppType::from_str(app) {
            Some(a) => a.protocol().as_str(),
            None => "anthropic",
        }
    };

    // 主体 = 同名组里最早创建的行;归集端点与各分组 current
    let mut master: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut endpoints: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
        std::collections::HashMap::new();
    let mut current_of: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // app_type -> master id(后写覆盖,保留最近切换)
    for row in &rows {
        let key = normalize(&row.name);
        let mid = master.entry(key).or_insert_with(|| row.id.clone());
        endpoints
            .entry(mid.clone())
            .or_default()
            .insert(protocol_of(&row.app_type).to_string(), row.base_url.clone());
        if row.is_current {
            current_of.insert(row.app_type.clone(), mid.clone());
        }
    }

    // 1) 主体的 endpoints 写库(同步 base_url 为首个端点,保持旧字段可用)
    {
        let mut up = conn.prepare("UPDATE providers SET endpoints = ?1, base_url = ?2 WHERE id = ?3")?;
        for (mid, eps) in &endpoints {
            let json = serde_json::to_string(eps)?;
            let primary = eps.values().next().cloned().unwrap_or_default();
            up.execute(params![json, primary, mid])?;
        }
    }
    // 2) 删除非主体同名行;usage_log 的 provider_id 归并到主体
    {
        let master_ids: std::collections::HashSet<&String> = master.values().collect();
        let del: Vec<&OldRow> = rows.iter().filter(|r| !master_ids.contains(&r.id)).collect();
        {
            let mut st = conn.prepare("DELETE FROM providers WHERE id = ?1")?;
            for r in &del {
                st.execute(params![r.id])?;
            }
        }
        let mut re = conn.prepare("UPDATE usage_log SET provider_id = ?1 WHERE provider_id = ?2")?;
        for r in &del {
            let mid = &master[&normalize(&r.name)];
            re.execute(params![mid, r.id])?;
        }
    }
    // 3) 各分组的当前供应商写入 current_map(旧的 is_current 列停用,统一清零)
    conn.execute_batch("UPDATE providers SET is_current = 0")?;
    {
        let mut st = conn.prepare(
            "INSERT OR REPLACE INTO current_map (app_type, provider_id) VALUES (?1, ?2)",
        )?;
        for (app, mid) in &current_of {
            st.execute(params![app, mid])?;
        }
    }
    Ok(())
}
