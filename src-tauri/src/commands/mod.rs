//! Tauri 命令层(薄):仅做参数接收与调用编排,业务在 `services/`。
//!
//! 用同步命令(`fn` 而非 `async fn`):Tauri 2 会在独立线程执行同步命令,
//! 不会阻塞 UI;而 rusqlite 是阻塞调用,放线程池比放 async runtime 更合适。

pub mod backup;
pub mod import;
pub mod keychain;
pub mod locale;
pub mod profile;
pub mod provider;
pub mod proxy;
pub mod route;
pub mod settings;
pub mod takeover;
pub mod tray;
pub mod update;
pub mod usage;
pub mod usage_dash;
