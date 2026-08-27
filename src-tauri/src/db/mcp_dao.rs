//! MCP 服务器 DAO(统一管理,一处定义同步到各 CLI)。

use anyhow::{anyhow, Result};
use rusqlite::params;

use crate::db::{Pool, PooledConn};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    /// JSON:stdio {"command","args","env"} 或远程 {"type":"http","url"}
    pub config: serde_json::Value,
    /// 生效的 app 列表(如 ["claude","codex"])
    pub apps: Vec<String>,
    pub enabled: bool,
    pub created_at: i64,
}

fn get_conn(pool: &Pool) -> Result<PooledConn> {
    pool.get().map_err(|e| anyhow!("获取数据库连接失败: {e}"))
}

fn row_to_server(r: &rusqlite::Row) -> rusqlite::Result<McpServer> {
    let config_raw: String = r.get(2)?;
    let apps_raw: String = r.get(3)?;
    Ok(McpServer {
        id: r.get(0)?,
        name: r.get(1)?,
        config: serde_json::from_str(&config_raw).unwrap_or(serde_json::Value::Null),
        apps: serde_json::from_str(&apps_raw).unwrap_or_default(),
        enabled: r.get::<_, i64>(4)? != 0,
        created_at: r.get(5)?,
    })
}

pub fn list(pool: &Pool) -> Result<Vec<McpServer>> {
    let conn = get_conn(pool)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, config, apps, enabled, created_at FROM mcp_servers ORDER BY created_at, name",
    )?;
    let rows = stmt.query_map([], row_to_server)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn upsert(pool: &Pool, s: &McpServer) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "INSERT INTO mcp_servers(id, name, config, apps, enabled, created_at)
         VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name, config = excluded.config,
           apps = excluded.apps, enabled = excluded.enabled",
        params![
            s.id,
            s.name,
            s.config.to_string(),
            serde_json::to_string(&s.apps)?,
            s.enabled as i64,
            s.created_at
        ],
    )?;
    Ok(())
}

pub fn delete(pool: &Pool, id: &str) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_enabled(pool: &Pool, id: &str, enabled: bool) -> Result<()> {
    let conn = get_conn(pool)?;
    conn.execute(
        "UPDATE mcp_servers SET enabled = ?2 WHERE id = ?1",
        params![id, enabled as i64],
    )?;
    Ok(())
}
