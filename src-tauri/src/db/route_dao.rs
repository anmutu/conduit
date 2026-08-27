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
    /// 启用开关(停用后不参与匹配,保留配置)
    pub enabled: bool,
    /// 匹配方式:contains(包含)| starts_with(前缀)
    pub match_type: String,
    /// 规则级降级供应商(命中供应商 5xx 时优先回退到它;None = 用全局候选链)
    pub fallback_provider_id: Option<String>,
}

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

pub fn list_by_app(pool: &Pool, app: &str) -> Result<Vec<RouteRule>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, pattern, provider_id, enabled, match_type, fallback_provider_id
         FROM route_rules WHERE app_type = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![app], |r| {
        Ok(RouteRule {
            id: r.get(0)?,
            app_type: r.get(1)?,
            pattern: r.get(2)?,
            provider_id: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            match_type: r.get(5)?,
            fallback_provider_id: r.get(6)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn insert(
    pool: &Pool,
    app: &str,
    pattern: &str,
    provider_id: &str,
    match_type: &str,
) -> Result<i64> {
    let conn = get_conn(pool)?;
    conn.execute(
        "INSERT INTO route_rules (app_type, pattern, provider_id, match_type, created_at) VALUES (?1,?2,?3,?4,?5)",
        params![app, pattern, provider_id, match_type, chrono::Utc::now().timestamp()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete(pool: &Pool, id: i64) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute("DELETE FROM route_rules WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_enabled(pool: &Pool, id: i64, enabled: bool) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "UPDATE route_rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
    )?;
    Ok(())
}

/// 设置规则级降级供应商;None 清除(回退全局候选链)。
pub fn set_fallback(pool: &Pool, id: i64, fallback: Option<&str>) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "UPDATE route_rules SET fallback_provider_id = ?1 WHERE id = ?2",
        params![fallback, id],
    )?;
    Ok(())
}

/// 长上下文分流预设(KV 存储,非表):超过 token 阈值的请求优先走指定供应商。
/// value = JSON {provider_id, threshold}
pub fn get_longctx(pool: &Pool, app: &str) -> Result<Option<(String, i64)>> {
    let raw = crate::db::kv::get(pool, &format!("route.longctx.{app}"))?;
    let Some(json) = raw else { return Ok(None) };
    let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    let pid = v.get("provider_id").and_then(|x| x.as_str()).unwrap_or("");
    if pid.is_empty() {
        return Ok(None);
    }
    let threshold = v
        .get("threshold")
        .and_then(|x| x.as_i64())
        .unwrap_or(60_000);
    Ok(Some((pid.to_string(), threshold)))
}

pub fn set_longctx(pool: &Pool, app: &str, provider_id: &str, threshold: i64) -> Result<()> {
    let json = serde_json::json!({ "provider_id": provider_id, "threshold": threshold });
    crate::db::kv::set(pool, &format!("route.longctx.{app}"), &json.to_string())
}

pub fn clear_longctx(pool: &Pool, app: &str) -> Result<()> {
    crate::db::kv::del(pool, &format!("route.longctx.{app}"))
}

/// 命中查找:仅启用规则,contains(包含)/starts_with(前缀)均不区分大小写。
/// 返回 (provider_id, pattern, fallback_provider_id) —— pattern 供日志侧展示命中来源。
/// 规则少,内存匹配即可;返回首条命中。
pub fn match_provider(
    pool: &Pool,
    app: &str,
    model: Option<&str>,
) -> Result<Option<(String, String, Option<String>)>> {
    let Some(model) = model else { return Ok(None) };
    let rules = list_by_app(pool, app)?;
    let m = model.to_lowercase();
    Ok(rules
        .into_iter()
        .filter(|r| r.enabled && !r.pattern.trim().is_empty())
        .find(|r| {
            let p = r.pattern.to_lowercase();
            if r.match_type == "starts_with" {
                m.starts_with(&p)
            } else {
                m.contains(&p)
            }
        })
        .map(|r| (r.provider_id, r.pattern, r.fallback_provider_id)))
}
