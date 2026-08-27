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
    /// 匹配优先级(小者靠前,首条命中生效)
    pub priority: i64,
}

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

pub fn list_by_app(pool: &Pool, app: &str) -> Result<Vec<RouteRule>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_type, pattern, provider_id, enabled, match_type, fallback_provider_id, priority
         FROM route_rules WHERE app_type = ?1 ORDER BY priority, id",
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
            priority: r.get(7)?,
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
    // 新规则排到该分组末尾(priority 取当前最大值 +1)
    conn.execute(
        "INSERT INTO route_rules (app_type, pattern, provider_id, match_type, priority, created_at)
         VALUES (?1,?2,?3,?4,
                 COALESCE((SELECT MAX(priority)+1 FROM route_rules WHERE app_type = ?1), 0), ?5)",
        params![
            app,
            pattern,
            provider_id,
            match_type,
            chrono::Utc::now().timestamp()
        ],
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

/// 上移/下移规则:与相邻规则(priority 序)交换 priority。
/// dir = -1 上移 / +1 下移;到头时静默成功(no-op)。
pub fn move_rule(pool: &Pool, id: i64, dir: i64) -> Result<()> {
    let mut conn = get_conn(pool)?;
    let app: String = conn
        .query_row(
            "SELECT app_type FROM route_rules WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|_| anyhow!("规则不存在"))?;
    let rules: Vec<(i64, i64)> = conn
        .prepare("SELECT id, priority FROM route_rules WHERE app_type = ?1 ORDER BY priority, id")?
        .query_map(params![app], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();
    let pos = rules.iter().position(|(rid, _)| *rid == id);
    let Some(pos) = pos else { return Ok(()) };
    let target = if dir < 0 {
        pos.checked_sub(1)
    } else {
        Some(pos + 1).filter(|&t| t < rules.len())
    };
    let Some(target) = target else { return Ok(()) };
    let (id_a, p_a) = rules[pos];
    let (id_b, p_b) = rules[target];
    // 两条 priority 可能相同(历史数据),交换前先归一化为互不相同的值
    let tx = conn.transaction()?;
    if p_a == p_b {
        tx.execute(
            "UPDATE route_rules SET priority = priority + ROWID WHERE app_type = ?1",
            params![app],
        )?;
        let p_a: i64 = tx.query_row(
            "SELECT priority FROM route_rules WHERE id = ?1",
            params![id_a],
            |r| r.get(0),
        )?;
        let p_b: i64 = tx.query_row(
            "SELECT priority FROM route_rules WHERE id = ?1",
            params![id_b],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE route_rules SET priority = ?2 WHERE id = ?1",
            params![id_a, p_b],
        )?;
        tx.execute(
            "UPDATE route_rules SET priority = ?2 WHERE id = ?1",
            params![id_b, p_a],
        )?;
    } else {
        tx.execute(
            "UPDATE route_rules SET priority = ?2 WHERE id = ?1",
            params![id_a, p_b],
        )?;
        tx.execute(
            "UPDATE route_rules SET priority = ?2 WHERE id = ?1",
            params![id_b, p_a],
        )?;
    }
    tx.commit()?;
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

/// 后台轻量分流预设(某 app):Claude Code 后台小任务等 Haiku 档请求走指定供应商。
pub fn get_background(pool: &Pool, app: &str) -> Result<Option<String>> {
    let pid = crate::db::kv::get(pool, &format!("route.background.{app}"))
        .ok()
        .flatten()
        .unwrap_or_default();
    if pid.is_empty() {
        Ok(None)
    } else {
        Ok(Some(pid))
    }
}

pub fn set_background(pool: &Pool, app: &str, provider_id: &str) -> Result<()> {
    crate::db::kv::set(pool, &format!("route.background.{app}"), provider_id)
}

pub fn clear_background(pool: &Pool, app: &str) -> Result<()> {
    crate::db::kv::del(pool, &format!("route.background.{app}"))
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
