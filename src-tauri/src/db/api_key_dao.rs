//! API Key DAO。
//!
//! v4 起 API Key 直接存 SQLCipher 加密库(整库加密,等价静态加密),
//! keychain 仅保留数据库主密钥一项 —— 运行期零 keychain 访问,不再触发系统授权弹窗。

use anyhow::{anyhow, Result};
use rusqlite::params;

use crate::db::{Pool, PooledConn};

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

pub fn get(pool: &Pool, provider_id: &str) -> Result<Option<String>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare("SELECT key FROM api_keys WHERE provider_id = ?1")?;
    let mut rows = stmt.query_map(params![provider_id], |r| r.get::<_, String>(0))?;
    Ok(rows.next().and_then(|r| r.ok()))
}

pub fn set(pool: &Pool, provider_id: &str, key: &str) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "INSERT INTO api_keys (provider_id, key) VALUES (?1, ?2)
         ON CONFLICT(provider_id) DO UPDATE SET key = excluded.key",
        params![provider_id, key],
    )?;
    Ok(())
}

pub fn delete(pool: &Pool, provider_id: &str) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "DELETE FROM api_keys WHERE provider_id = ?1",
        params![provider_id],
    )?;
    Ok(())
}
