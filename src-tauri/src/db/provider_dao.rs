//! Provider 数据访问层。
//!
//! 所有读写经 r2d2 池获取连接。`Provider` 行与 [`crate::types::Provider`] 的映射在此完成;
//! 注意 DB 里 `models` 以 JSON 字符串存储,读取时反序列化为 `Vec<String>`。

use anyhow::{anyhow, Result};
use rusqlite::{params, Row};

use crate::db::{Pool, PooledConn};
use crate::types::{AppType, Provider};

fn row_to_provider(row: &Row<'_>) -> rusqlite::Result<Provider> {
    let app_type_str: String = row.get("app_type")?;
    let models_json: String = row.get("models")?;
    let models: Vec<String> = serde_json::from_str(&models_json).unwrap_or_default();
    Ok(Provider {
        id: row.get("id")?,
        app_type: AppType::from_str(&app_type_str).unwrap_or(AppType::Claude),
        name: row.get("name")?,
        base_url: row.get("base_url")?,
        keychain_id: row.get("keychain_id")?,
        models,
        is_current: row.get::<_, i64>("is_current")? != 0,
        is_healthy: row.get::<_, i64>("is_healthy")? != 0,
        sort_index: row.get("sort_index")?,
        created_at: row.get("created_at")?,
        has_key: false, // 运行时由 service 层填充
    })
}

pub fn list_all(pool: &Pool) -> Result<Vec<Provider>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, name, base_url, keychain_id, models, is_current,
                is_healthy, sort_index, created_at
         FROM providers ORDER BY sort_index, created_at",
    )?;
    let rows = stmt.query_map([], row_to_provider)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_by_app(pool: &Pool, app: AppType) -> Result<Vec<Provider>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, name, base_url, keychain_id, models, is_current,
                is_healthy, sort_index, created_at
         FROM providers WHERE app_type = ?1 ORDER BY sort_index, created_at",
    )?;
    let rows = stmt.query_map(params![app.as_str()], row_to_provider)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_by_id(pool: &Pool, id: &str) -> Result<Option<Provider>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, name, base_url, keychain_id, models, is_current,
                is_healthy, sort_index, created_at
         FROM providers WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_provider)?;
    Ok(rows.next().and_then(|r| r.ok()))
}

pub fn insert(pool: &Pool, p: &Provider) -> Result<()> {
    let conn = get_conn(pool)?;
    let models_json = serde_json::to_string(&p.models).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO providers
            (id, app_type, name, base_url, keychain_id, models, is_current,
             is_healthy, sort_index, created_at, meta)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'{}')",
        params![
            p.id,
            p.app_type.as_str(),
            p.name,
            p.base_url,
            p.keychain_id,
            models_json,
            p.is_current as i64,
            p.is_healthy as i64,
            p.sort_index,
            p.created_at,
        ],
    )?;
    Ok(())
}

/// 切换当前供应商(同 app_type 下仅一个 current)。
///
/// 在单事务内完成:清除同组 current → 置目标 current,保证一致性。
pub fn set_current(pool: &Pool, id: &str, app: AppType) -> Result<()> {
    let mut conn = get_conn(pool)?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE providers SET is_current = 0 WHERE app_type = ?1",
        params![app.as_str()],
    )?;
    let affected = tx.execute(
        "UPDATE providers SET is_current = 1 WHERE id = ?1 AND app_type = ?2",
        params![id, app.as_str()],
    )?;
    if affected == 0 {
        return Err(anyhow!("未找到供应商 {id} (app={})", app.as_str()));
    }
    tx.commit()?;
    Ok(())
}

pub fn delete(pool: &Pool, id: &str) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
    Ok(())
}

/// 更新供应商基础字段(name / base_url)。
pub fn update(pool: &Pool, id: &str, name: &str, base_url: &str) -> Result<()> {
    let conn = get_conn(pool)?;
    let affected = conn.execute(
        "UPDATE providers SET name = ?2, base_url = ?3 WHERE id = ?1",
        params![id, name, base_url],
    )?;
    if affected == 0 {
        return Err(anyhow!("供应商不存在: {id}"));
    }
    Ok(())
}

/// 读取指定 app_type 的当前供应商(代理转发时使用)。
pub fn get_current(pool: &Pool, app: AppType) -> Result<Option<Provider>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, name, base_url, keychain_id, models, is_current,
                is_healthy, sort_index, created_at
         FROM providers WHERE app_type = ?1 AND is_current = 1 LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![app.as_str()], row_to_provider)?;
    Ok(rows.next().and_then(|r| r.ok()))
}

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

/// 故障转移候选:当前供应商 + 其余已配置 Key 的供应商(按排序),上限 3。
pub fn failover_candidates(pool: &Pool, app: AppType) -> Result<Vec<Provider>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, name, base_url, keychain_id, models, is_current,
                is_healthy, sort_index, created_at
         FROM providers
         WHERE app_type = ?1 AND (is_current = 1 OR keychain_id IS NOT NULL)
         ORDER BY is_current DESC, sort_index, created_at
         LIMIT 3",
    )?;
    let rows = stmt.query_map(params![app.as_str()], row_to_provider)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
