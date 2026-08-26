//! 模型路由规则 DAO。

use anyhow::{anyhow, Result};
use rusqlite::params;

use crate::db::{Pool, PooledConn};

#[derive(Debug, serde::Serialize)]
pub struct RouteRule {
    pub id: i64,
    pub app_type: String,
    pub pattern: String,
    pub provider_id: String,
}

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

pub fn list_by_app(pool: &Pool, app: &str) -> Result<Vec<RouteRule>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, pattern, provider_id FROM route_rules
         WHERE app_type = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![app], |r| {
        Ok(RouteRule {
            id: r.get(0)?,
            app_type: r.get(1)?,
            pattern: r.get(2)?,
            provider_id: r.get(3)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn insert(pool: &Pool, app: &str, pattern: &str, provider_id: &str) -> Result<i64> {
    let conn = get_conn(pool)?;
    conn.execute(
        "INSERT INTO route_rules (app_type, pattern, provider_id, created_at) VALUES (?1,?2,?3,?4)",
        params![app, pattern, provider_id, chrono::Utc::now().timestamp()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete(pool: &Pool, id: i64) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute("DELETE FROM route_rules WHERE id = ?1", params![id])?;
    Ok(())
}

/// 命中查找:model 包含 pattern(不区分大小写)→ 返回 provider_id。
/// 规则少,内存匹配即可;返回首条命中。
pub fn match_provider(
    pool: &Pool,
    app: &str,
    model: Option<&str>,
) -> Result<Option<String>> {
    let Some(model) = model else { return Ok(None) };
    let rules = list_by_app(pool, app)?;
    let m = model.to_lowercase();
    Ok(rules
        .into_iter()
        .find(|r| !r.pattern.trim().is_empty() && m.contains(&r.pattern.to_lowercase()))
        .map(|r| r.provider_id))
}
