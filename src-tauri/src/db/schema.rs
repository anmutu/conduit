//! 表结构与迁移。
//!
//! M0 只需要两张表:`providers`(供应商配置)和 `settings`(KV 杂项)。
//! 注意 `providers` 表刻意**不存 API Key 明文**,只有 `keychain_id` 引用。

use anyhow::Result;
use rusqlite::Connection;

/// 当前 schema 版本,用于未来迁移校验。
#[allow(dead_code)]
pub const SCHEMA_VERSION: u32 = 1;

pub fn create_tables(conn: &Connection) -> Result<()> {
    // schema_version 记录,便于后续迁移判断
    conn.execute_batch(
        r#"
        PRAGMA user_version = 1;

        CREATE TABLE IF NOT EXISTS providers (
            id          TEXT PRIMARY KEY,
            app_type    TEXT NOT NULL,
            name        TEXT NOT NULL,
            base_url    TEXT NOT NULL,
            -- keychain 引用标识;真实 Key 不入库,只存这里作为取用凭证
            keychain_id TEXT,
            models      TEXT NOT NULL DEFAULT '[]',
            is_current  INTEGER NOT NULL DEFAULT 0,
            is_healthy  INTEGER NOT NULL DEFAULT 1,
            sort_index  INTEGER NOT NULL DEFAULT 0,
            created_at  INTEGER NOT NULL DEFAULT 0,
            meta        TEXT NOT NULL DEFAULT '{}'
        );
        CREATE INDEX IF NOT EXISTS idx_providers_app     ON providers(app_type);
        CREATE INDEX IF NOT EXISTS idx_providers_current ON providers(app_type, is_current);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}
