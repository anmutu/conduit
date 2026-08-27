//! 配置备份/恢复。
//!
//! 导出:全部供应商(名称/端点/模型)+ 路由规则/长上下文分流 → app 数据目录下的 JSON 文件。
//! 出于安全考虑 **不导出 API Key**(keychain 条目与本机绑定,跨机无意义)。
//! 导入:同名跳过,异名新建(端点/模型还原),Key 需重新粘贴一次;
//!       规则按供应商名称重新挂接,已存在的 (app, 匹配词) 跳过。

use anyhow::{anyhow, Result};

use crate::db::{provider_dao, route_dao, Pool};
use crate::types::Provider;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupFile {
    pub version: u32,
    pub exported_at: i64,
    pub providers: Vec<BackupProvider>,
    /// v2:路由规则(按供应商名称挂接,导入端解析回 id)
    #[serde(default)]
    pub rules: Vec<BackupRule>,
    /// v2:长上下文分流预设(按 app)
    #[serde(default)]
    pub longctx: Vec<BackupLongctx>,
    /// v3:MCP 服务器
    #[serde(default)]
    pub mcp: Vec<BackupMcp>,
    /// v3:Skills vault
    #[serde(default)]
    pub skills: Vec<BackupSkill>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupProvider {
    pub name: String,
    pub base_url: String,
    pub endpoints: std::collections::HashMap<String, String>,
    pub models: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupRule {
    pub app_type: String,
    pub pattern: String,
    pub provider: String,
    pub match_type: String,
    pub enabled: bool,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub priority: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupLongctx {
    pub app_type: String,
    pub provider: String,
    pub threshold: i64,
}

/// v3:MCP 服务器(统一管理)
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupMcp {
    pub id: String,
    pub name: String,
    pub config: serde_json::Value,
    pub apps: Vec<String>,
    pub enabled: bool,
}

/// v3:Skills vault 内容(files 为相对路径 → base64;导入后自动同步)
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupSkill {
    pub id: String,
    pub files: Vec<BackupSkillFile>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupSkillFile {
    pub path: String,
    /// base64(可能有二进制附属文件)
    pub content: String,
}

/// 导出到指定路径,返回导出的供应商数量(规则随文件一并带出)。
pub fn export(pool: &Pool, path: &std::path::Path) -> Result<usize> {
    let list = provider_dao::list_all(pool)?;
    let id_to_name = |id: &str| -> String {
        list.iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    };
    let mut rules = Vec::new();
    let mut longctx = Vec::new();
    for app in crate::types::AppType::all() {
        for r in route_dao::list_by_app(pool, app.as_str()).unwrap_or_default() {
            rules.push(BackupRule {
                app_type: r.app_type,
                pattern: r.pattern,
                provider: id_to_name(&r.provider_id),
                match_type: r.match_type,
                enabled: r.enabled,
                fallback: r
                    .fallback_provider_id
                    .as_deref()
                    .map(id_to_name)
                    .filter(|n| !n.is_empty()),
                priority: r.priority,
            });
        }
        if let Some((pid, threshold)) = route_dao::get_longctx(pool, app.as_str()).ok().flatten() {
            let provider = id_to_name(&pid);
            if !provider.is_empty() {
                longctx.push(BackupLongctx {
                    app_type: app.as_str().to_string(),
                    provider,
                    threshold,
                });
            }
        }
    }
    let file = BackupFile {
        version: 3,
        exported_at: chrono::Utc::now().timestamp(),
        providers: list
            .iter()
            .map(|p| BackupProvider {
                name: p.name.clone(),
                base_url: p.base_url.clone(),
                endpoints: p.endpoints.clone(),
                models: p.models.clone(),
            })
            .collect(),
        rules,
        longctx,
        mcp: crate::db::mcp_dao::list(pool)?
            .into_iter()
            .map(|m| BackupMcp {
                id: m.id,
                name: m.name,
                config: m.config,
                apps: m.apps,
                enabled: m.enabled,
            })
            .collect(),
        skills: crate::services::skills::export_vault()?,
    };
    let json = serde_json::to_string_pretty(&file)?;
    std::fs::write(path, json)?;
    Ok(list.len())
}

/// 从指定路径导入:同名跳过,异名新建。返回 (新建数, 跳过数)。
pub fn import(pool: &Pool, path: &std::path::Path) -> Result<(usize, usize)> {
    let raw = std::fs::read_to_string(path).map_err(|e| anyhow!("读取备份失败: {e}"))?;
    let file: BackupFile =
        serde_json::from_str(&raw).map_err(|e| anyhow!("备份文件格式错误: {e}"))?;
    let mut created = 0;
    let mut skipped = 0;
    for bp in file.providers {
        if provider_dao::find_by_name(pool, &bp.name)?.is_some() {
            skipped += 1;
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let p = Provider {
            id,
            app_type: crate::types::AppType::Claude, // 逻辑实体跨分组,首分组字段仅为兼容
            name: bp.name,
            base_url: bp.base_url,
            endpoints: bp.endpoints,
            keychain_id: None,
            models: bp.models,
            is_current: false,
            is_healthy: true,
            sort_index: 0,
            created_at: chrono::Utc::now().timestamp(),
            has_key: false,
            meta_has_key: Some(false),
        };
        provider_dao::insert(pool, &p)?;
        created += 1;
    }
    // v2:规则与长上下文预设按供应商名称重新挂接;找不到对应供应商的条目静默跳过
    let name_to_id = |name: &str| {
        provider_dao::find_by_name(pool, name)
            .ok()
            .flatten()
            .map(|p| p.id)
    };
    for r in &file.rules {
        let (Some(pid), app) = (name_to_id(&r.provider), r.app_type.as_str()) else {
            continue;
        };
        let exists = route_dao::list_by_app(pool, app)
            .map(|rs| {
                rs.iter()
                    .any(|x| x.pattern.eq_ignore_ascii_case(&r.pattern))
            })
            .unwrap_or(true);
        if exists {
            continue;
        }
        let id = route_dao::insert(pool, app, &r.pattern, &pid, &r.match_type).unwrap_or(-1);
        if id >= 0 {
            let _ = route_dao::set_enabled(pool, id, r.enabled);
            let _ = route_dao::set_fallback(
                pool,
                id,
                r.fallback.as_deref().and_then(name_to_id).as_deref(),
            );
        }
    }
    for lc in &file.longctx {
        if let Some(pid) = name_to_id(&lc.provider) {
            let _ = route_dao::set_longctx(pool, &lc.app_type, &pid, lc.threshold);
        }
    }
    // v3:MCP 与 Skills(已存在同 id 跳过;导入 Skills 后立即同步到各 CLI)
    let mut skills_imported = 0;
    for m in &file.mcp {
        let exists = crate::db::mcp_dao::list(pool)
            .map(|l| l.iter().any(|x| x.id == m.id))
            .unwrap_or(true);
        if exists {
            continue;
        }
        let _ = crate::db::mcp_dao::upsert(
            pool,
            &crate::db::mcp_dao::McpServer {
                id: m.id.clone(),
                name: m.name.clone(),
                config: m.config.clone(),
                apps: m.apps.clone(),
                enabled: m.enabled,
                created_at: chrono::Utc::now().timestamp(),
            },
        );
    }
    for s in &file.skills {
        if crate::services::skills::import_backup_skill(&s.id, &s.files)? {
            skills_imported += 1;
        }
    }
    if skills_imported > 0 {
        let _ = crate::services::skills::sync_all();
    }
    Ok((created, skipped))
}

/// 默认备份路径:app 数据目录/conduit-backup-YYYYMMDD-HHMMSS.json
pub fn default_backup_path(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    app_data_dir.join(format!("conduit-backup-{ts}.json"))
}
