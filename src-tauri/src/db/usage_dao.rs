//! 用量计量 DAO。

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::db::Pool;

#[derive(Debug, Serialize, Clone, Copy, Default)]
pub struct UsageSummary {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

pub fn insert(
    pool: &Pool,
    app_type: &str,
    provider_id: &str,
    model: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO usage_log(app_type, provider_id, model, input_tokens, output_tokens, created_at)
         VALUES(?1,?2,?3,?4,?5,?6)",
        rusqlite::params![
            app_type,
            provider_id,
            model,
            input_tokens,
            output_tokens,
            chrono::Utc::now().timestamp(),
        ],
    )?;
    Ok(())
}

/// 按供应商聚合。key = provider_id
pub fn summarize_map(pool: &Pool, app_type: &str) -> Result<std::collections::HashMap<String, UsageSummary>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT provider_id,
                COUNT(*),
                COALESCE(SUM(input_tokens),0),
                COALESCE(SUM(output_tokens),0)
         FROM usage_log WHERE app_type = ?1 GROUP BY provider_id",
    )?;
    let rows = stmt.query_map(rusqlite::params![app_type], |r| {
        Ok((
            r.get::<_, String>(0)?,
            UsageSummary {
                requests: r.get(1)?,
                input_tokens: r.get(2)?,
                output_tokens: r.get(3)?,
            },
        ))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows.flatten() {
        map.insert(row.0, row.1);
    }
    Ok(map)
}
