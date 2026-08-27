//! 数据库层:rusqlite + SQLCipher(整库加密)+ r2d2 连接池。
//!
//! 设计要点(对标竞品痛点):
//! - **整库加密**:主密钥保存在系统 keychain(`services/keychain`),DB 文件本身用
//!   SQLCipher 加密,落盘即密文。
//! - **连接池 + WAL**:用 r2d2 池替代竞品的"单 Mutex 单连接",代理热路径不会因 DB
//!   争锁而阻塞;WAL 提升并发读写。
//! - **per-app 锁**:切换等需要串行的操作用更细粒度的锁(见 services),不全局串行。

pub mod api_key_dao;
pub mod kv;
pub mod provider_dao;
pub mod route_dao;
mod schema;
pub mod usage_dao;

use anyhow::{anyhow, Result};
use r2d2::CustomizeConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type PooledConn = r2d2::PooledConnection<SqliteConnectionManager>;

/// 每次从池中获取连接时执行:解锁 SQLCipher + 启用 WAL。
///
/// `on_acquire` 在连接已打开(空库)后、交还调用方前执行,
/// 此时执行 `PRAGMA key` 可正确解锁 SQLCipher。
#[derive(Debug)]
struct SqliteInit {
    /// 32 字节主密钥的十六进制(64 字符)
    cipher_key_hex: String,
}

impl CustomizeConnection<Connection, rusqlite::Error> for SqliteInit {
    fn on_acquire(&self, conn: &mut Connection) -> std::result::Result<(), rusqlite::Error> {
        // SQLCipher 十六进制密钥格式:`PRAGMA key = "x'<64 hex>'"`,
        // 直接对应 32 字节 AES-256 密钥,不受特殊字符影响。
        let pragma = format!("PRAGMA key = \"x'{}'\";", self.cipher_key_hex);
        conn.execute_batch(&pragma)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(())
    }
}

/// 初始化数据库连接池并建表。
///
/// `cipher_key_hex` 应来自 `services::keychain::get_or_create_master_key`。
pub fn init_pool<P: AsRef<Path>>(db_path: P, cipher_key_hex: &str) -> Result<Pool> {
    if let Some(parent) = db_path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    // v1→v2 合并迁移不可逆:执行前先备份一次(存在旧库且尚无备份时)
    {
        let p = db_path.as_ref();
        if p.exists() {
            let backup = p.with_extension("db.bak-v1");
            if !backup.exists() {
                // 仅在仍是 v1 时备份,避免每次启动都覆盖
                if let Ok(conn) = Connection::open(p) {
                    let _ = conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", cipher_key_hex));
                    let version: i64 = conn
                        .query_row("PRAGMA user_version", [], |r| r.get(0))
                        .unwrap_or(0);
                    if version < 2 {
                        let _ = std::fs::copy(p, &backup);
                    }
                }
            }
        }
    }
    let manager = SqliteConnectionManager::file(db_path);
    let pool = r2d2::Pool::builder()
        .max_size(8)
        .connection_customizer(Box::new(SqliteInit {
            cipher_key_hex: cipher_key_hex.to_string(),
        }))
        .build(manager)
        .map_err(|e| anyhow!("数据库连接池创建失败: {e}"))?;

    // 首次建表
    {
        let conn = pool.get().map_err(|e| anyhow!("获取连接失败: {e}"))?;
        schema::create_tables(&conn)?;
    }
    Ok(pool)
}
