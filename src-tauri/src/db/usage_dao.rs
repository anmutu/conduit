//! 用量计量 DAO。

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::db::Pool;

#[derive(Debug, Serialize, Clone, Copy, Default)]
pub struct UsageSummary {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// 失败请求数(上游 HTTP >= 400)
    #[serde(default)]
    pub errors: i64,
}

#[allow(clippy::too_many_arguments)]
pub fn insert(
    pool: &Pool,
    app_type: &str,
    provider_id: &str,
    model: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
    status: u16,
    rule_pattern: Option<&str>,
    duration_ms: i64,
) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO usage_log(app_type, provider_id, model, input_tokens, output_tokens, status, rule_pattern, duration_ms, created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            app_type,
            provider_id,
            model,
            input_tokens,
            output_tokens,
            status as i64,
            rule_pattern,
            duration_ms,
            chrono::Utc::now().timestamp(),
        ],
    )?;
    Ok(())
}

/// 按供应商聚合。key = provider_id
pub fn summarize_map(
    pool: &Pool,
    app_type: &str,
) -> Result<std::collections::HashMap<String, UsageSummary>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT provider_id,
                COUNT(*),
                COALESCE(SUM(input_tokens),0),
                COALESCE(SUM(output_tokens),0),
                COALESCE(SUM(CASE WHEN status >= 400 THEN 1 ELSE 0 END),0)
         FROM usage_log WHERE app_type = ?1 GROUP BY provider_id",
    )?;
    let rows = stmt.query_map(rusqlite::params![app_type], |r| {
        Ok((
            r.get::<_, String>(0)?,
            UsageSummary {
                requests: r.get(1)?,
                input_tokens: r.get(2)?,
                output_tokens: r.get(3)?,
                errors: r.get(4)?,
            },
        ))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows.flatten() {
        map.insert(row.0, row.1);
    }
    Ok(map)
}

// ---------- 仪表盘聚合 ----------

/// 单条请求日志(请求浏览器用)
#[derive(Debug, Serialize)]
pub struct UsageEntry {
    pub id: i64,
    pub provider_id: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub status: u16,
    /// 命中的路由规则匹配词(未命中为 None)
    pub rule_pattern: Option<String>,
    /// 请求耗时(毫秒;旧行为 0)
    #[serde(default)]
    pub duration_ms: i64,
    pub created_at: i64,
}

/// 最近 N 条请求日志(新→旧)
pub fn recent(pool: &Pool, app_type: &str, limit: i64) -> Result<Vec<UsageEntry>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, model, input_tokens, output_tokens, status, rule_pattern, duration_ms, created_at
         FROM usage_log WHERE app_type = ?1
         ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![app_type, limit], |r| {
        Ok(UsageEntry {
            id: r.get(0)?,
            provider_id: r.get(1)?,
            model: r.get(2)?,
            input_tokens: r.get(3)?,
            output_tokens: r.get(4)?,
            status: r.get::<_, i64>(5)? as u16,
            rule_pattern: r.get(6)?,
            duration_ms: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

/// 删除超过 days 天的日志行(本地观察用途,防库无限膨胀)。返回删除行数。
pub fn prune(pool: &Pool, days: i64) -> Result<usize> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let cutoff = chrono::Utc::now().timestamp() - days * 86400;
    let n = conn.execute(
        "DELETE FROM usage_log WHERE created_at < ?1",
        rusqlite::params![cutoff],
    )?;
    Ok(n)
}

/// 最近实际服务过请求的供应商(app_type 一起返回,供托盘「最近使用」)。
/// 每个供应商取最近一次,按最近排序,最多 n 个。
pub fn recent_providers(pool: &Pool, n: i64) -> Result<Vec<(String, String)>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT provider_id, app_type FROM usage_log
         GROUP BY provider_id ORDER BY MAX(id) DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![n], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    Ok(rows.flatten().collect())
}

#[derive(Debug, Serialize, Clone)]
pub struct NamedUsage {
    pub key: String,
    #[serde(flatten)]
    pub summary: UsageSummary,
}

#[derive(Debug, Serialize)]
pub struct DayUsage {
    pub date: String,
    pub requests: i64,
    pub tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct UsageDashboard {
    pub total: UsageSummary,
    pub by_provider: Vec<NamedUsage>,
    pub by_model: Vec<NamedUsage>,
    pub by_day: Vec<DayUsage>,
}

fn app_total(pool: &Pool, app_type: &str) -> Result<UsageSummary> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                COALESCE(SUM(CASE WHEN status >= 400 THEN 1 ELSE 0 END),0)
         FROM usage_log WHERE app_type = ?1",
        rusqlite::params![app_type],
        |r| {
            Ok(UsageSummary {
                requests: r.get(0)?,
                input_tokens: r.get(1)?,
                output_tokens: r.get(2)?,
                errors: r.get(3)?,
            })
        },
    )
    .map_err(|e| anyhow!("{e}"))
}

fn group_by(pool: &Pool, app_type: &str, column: &str, limit: i64) -> Result<Vec<NamedUsage>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let sql = format!(
        "SELECT {column} AS k, COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
         FROM usage_log WHERE app_type = ?1 AND {column} IS NOT NULL AND {column} != ''
         GROUP BY k ORDER BY (SUM(input_tokens)+SUM(output_tokens)) DESC LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![app_type], |r| {
        Ok(NamedUsage {
            key: r.get(0)?,
            summary: UsageSummary {
                requests: r.get(1)?,
                input_tokens: r.get(2)?,
                output_tokens: r.get(3)?,
                errors: 0, // 分组明细暂不统计错误数
            },
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn dashboard(pool: &Pool, app_type: &str, days: i64) -> Result<UsageDashboard> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let week_ago = chrono::Utc::now().timestamp() - days * 86400;
    let mut stmt = conn.prepare(
        "SELECT date(created_at,'unixepoch','localtime') AS d, COUNT(*),
                COALESCE(SUM(input_tokens)+SUM(output_tokens),0)
         FROM usage_log WHERE app_type = ?1 AND created_at >= ?2
         GROUP BY d ORDER BY d",
    )?;
    let by_day = stmt
        .query_map(rusqlite::params![app_type, week_ago], |r| {
            Ok(DayUsage {
                date: r.get(0)?,
                requests: r.get(1)?,
                tokens: r.get(2)?,
            })
        })?
        .flatten()
        .collect();

    Ok(UsageDashboard {
        total: app_total(pool, app_type)?,
        by_provider: group_by(pool, app_type, "provider_id", 10)?,
        by_model: group_by(pool, app_type, "model", 8)?,
        by_day,
    })
}
