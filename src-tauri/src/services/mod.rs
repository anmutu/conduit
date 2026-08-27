//! 业务服务层。
//!
//! 命令层(`commands/`)调用这里,再向下到 `db/` 与系统 keychain。
//! 保持命令层"薄":只做参数校验和调用编排。

pub mod backup;
pub mod import;
pub mod keychain;
pub mod keys;
pub mod mcp;
pub mod profile;
pub mod provider;
pub mod skills;
pub mod takeover;
