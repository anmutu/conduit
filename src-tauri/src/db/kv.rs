//! settings KV(公共)。
use crate::db::Pool;
use anyhow::{anyhow, Result};

pub fn get(pool: &Pool, key: &str) -> Result<Option<String>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(rusqlite::params![key], |r| r.get::<_, String>(0))?;
    Ok(rows.next().and_then(|r| r.ok()))
}

pub fn set(pool: &Pool, key: &str, value: &str) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn del(pool: &Pool, key: &str) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        rusqlite::params![key],
    )?;
    Ok(())
}
