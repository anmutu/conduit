//! 代理状态命令。

use serde::Serialize;

use crate::core::proxy::PROXY_ADDR;

#[derive(Debug, Serialize)]
pub struct ProxyStatus {
    /// 代理监听地址
    pub addr: String,
    /// 是否已随应用启动(M0 代理常驻,恒为 true)
    pub running: bool,
    /// 支持的 CLI 应用
    pub supported_apps: Vec<String>,
}

#[tauri::command]
pub fn proxy_status() -> ProxyStatus {
    ProxyStatus {
        addr: PROXY_ADDR.to_string(),
        running: true,
        supported_apps: crate::types::AppType::all()
            .iter()
            .map(|a| a.as_str().to_string())
            .collect(),
    }
}
