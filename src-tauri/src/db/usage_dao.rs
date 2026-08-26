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

pub fn insert(
    pool: &Pool,
    app_type: &str,
    provider_id: &str,
    model: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
    status: u16,
) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO usage_log(app_type, provider_id, model, input_tokens, output_tokens, status, created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            app_type,
            provider_id,
            model,
            input_tokens,
            output_tokens,
            status as i64,
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
    .map_err(|e| anyhow!("{e}").into())
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

pub fn dashboard(pool: &Pool, app_type: &str) -> Result<UsageDashboard> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let week_ago = chrono::Utc::now().timestamp() - 7 * 86400;
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
