//! 进程级共享状态。
//!
//! `AppState` 同时被 Tauri 命令层和 axum 代理共享(都实现 `Clone`):
//! - `db`: r2d2 连接池,内部已是 `Arc`,克隆廉价
//! - `http`: reqwest 连接池客户端,复用 TCP 连接
//!
//! 代理热路径不持有全局锁:`get_current` 直接走 DB 池,并发请求各取各的连接。

use crate::db::Pool;

#[derive(Clone)]
pub struct AppState {
    pub db: Pool,
    pub http: reqwest::Client,
    /// Tauri 应用句柄(emit 事件用);测试环境为 None
    pub app: Option<tauri::AppHandle>,
}

impl AppState {
    pub fn new(db: Pool) -> Self {
        Self::with_handle(db, None)
    }

    pub fn with_handle(db: Pool, app: Option<tauri::AppHandle>) -> Self {
        let http = reqwest::Client::builder()
            // LLM 流式响应可能很久,放宽超时;代理不主动断流
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("reqwest client 构建失败");
        Self { db, http, app }
    }
}
