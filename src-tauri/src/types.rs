//! 共享类型定义
//!
//! 贯穿前后端的领域类型。注意安全设计:`Provider` 结构**不包含 API Key 字段**,
//! Key 只存放在系统 keychain,数据库里只保留 keychain 的引用标识(`keychain_id`)。
//! 这是 Conduit 相对竞品的核心安全差异化:DB 泄露不会导致凭证泄露。

use serde::{Deserialize, Serialize};

/// 支持的 CLI 应用类型。
///
/// 每个 CLI 的请求由本地代理按 URL 前缀分流(见 `core/proxy/server.rs` 的路由表),
/// 供应商也按 `app_type` 分组管理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppType {
    Claude,
    Codex,
    Gemini,
    OpenCode,
    OpenClaw,
}

impl AppType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppType::Claude => "claude",
            AppType::Codex => "codex",
            AppType::Gemini => "gemini",
            AppType::OpenCode => "opencode",
            AppType::OpenClaw => "openclaw",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(AppType::Claude),
            "codex" => Some(AppType::Codex),
            "gemini" => Some(AppType::Gemini),
            "opencode" => Some(AppType::OpenCode),
            "openclaw" => Some(AppType::OpenClaw),
            _ => None,
        }
    }

    /// 该应用类型的请求路径前缀(用于代理路由分流)
    pub fn path_prefixes(&self) -> &'static [&'static str] {
        match self {
            // Anthropic Messages API
            AppType::Claude => &["/v1/messages"],
            // OpenAI 兼容(Codex CLI 走 chat completions / responses);
            // 兼容把代理地址配成不带 /v1 的变体
            AppType::Codex => &[
                "/v1/chat/completions",
                "/v1/responses",
                "/chat/completions",
                "/responses",
            ],
            // Google Gemini API
            AppType::Gemini => &["/v1beta/"],
            // OpenCode / OpenClaw 复用 Anthropic 端点(M1 先透传)
            AppType::OpenCode => &["/v1/messages"],
            AppType::OpenClaw => &["/v1/messages"],
        }
    }

    pub fn all() -> [AppType; 5] {
        [
            AppType::Claude,
            AppType::Codex,
            AppType::Gemini,
            AppType::OpenCode,
            AppType::OpenClaw,
        ]
    }
}

/// 供应商配置。
///
/// `has_key` 由后端在序列化时填充(查 keychain 是否存在对应记录),
/// 因此前端能看到"该供应商是否已配置 Key",但永远拿不到 Key 本身。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub app_type: AppType,
    pub name: String,
    pub base_url: String,
    /// keychain 引用标识,真实 Key 通过它从系统 keychain 取
    pub keychain_id: Option<String>,
    /// 可用模型列表
    pub models: Vec<String>,
    pub is_current: bool,
    pub is_healthy: bool,
    pub sort_index: i64,
    pub created_at: i64,
    /// 前端展示用:Key 是否已配置(运行时填充,不持久化)
    #[serde(default)]
    pub has_key: bool,
}

/// 创建供应商时的输入
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderInput {
    pub app_type: AppType,
    pub name: String,
    pub base_url: String,
    pub models: Vec<String>,
    /// 可选:创建时一并写入 keychain
    #[serde(default)]
    pub api_key: Option<String>,
}
