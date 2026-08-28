//! 共享类型定义
//!
//! 贯穿前后端的领域类型。注意安全设计:`Provider` 结构**不包含 API Key 字段**,
//! Key 只存放在系统 keychain,数据库里只保留 keychain 的引用标识(`keychain_id`)。
//! 这是 Keyway 相对竞品的核心安全差异化:DB 泄露不会导致凭证泄露。

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
    /// 扩充分组(默认隐藏,可在设置中开启;代理层与 OpenCode 同级透传)
    Qwen,
    Iflow,
    Crush,
    Droid,
}

/// 全部分组(顺序即代理路由与托盘的遍历顺序;前 5 个为默认显示)
pub const ALL_APP_TYPES: [AppType; 9] = [
    AppType::Claude,
    AppType::Codex,
    AppType::Gemini,
    AppType::OpenCode,
    AppType::OpenClaw,
    AppType::Qwen,
    AppType::Iflow,
    AppType::Crush,
    AppType::Droid,
];

impl AppType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppType::Claude => "claude",
            AppType::Codex => "codex",
            AppType::Gemini => "gemini",
            AppType::OpenCode => "opencode",
            AppType::OpenClaw => "openclaw",
            AppType::Qwen => "qwen",
            AppType::Iflow => "iflow",
            AppType::Crush => "crush",
            AppType::Droid => "droid",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(AppType::Claude),
            "codex" => Some(AppType::Codex),
            "gemini" => Some(AppType::Gemini),
            "opencode" => Some(AppType::OpenCode),
            "openclaw" => Some(AppType::OpenClaw),
            "qwen" => Some(AppType::Qwen),
            "iflow" => Some(AppType::Iflow),
            "crush" => Some(AppType::Crush),
            "droid" => Some(AppType::Droid),
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
            // OpenCode / OpenClaw / 扩充分组复用既有端点(M1 先透传)
            AppType::OpenCode => &["/v1/messages"],
            AppType::OpenClaw => &["/v1/messages"],
            AppType::Qwen => &[
                "/v1/chat/completions",
                "/v1/responses",
                "/chat/completions",
                "/responses",
            ],
            AppType::Iflow => &["/v1/messages"],
            AppType::Crush => &["/v1/messages"],
            AppType::Droid => &["/v1/messages"],
        }
    }

    pub fn all() -> &'static [AppType] {
        &ALL_APP_TYPES
    }

    /// 该分组使用的上游协议(决定取供应商的哪个端点)
    pub fn protocol(&self) -> Protocol {
        match self {
            AppType::Claude
            | AppType::OpenCode
            | AppType::OpenClaw
            | AppType::Iflow
            | AppType::Crush
            | AppType::Droid => Protocol::Anthropic,
            AppType::Codex | AppType::Qwen => Protocol::Openai,
            AppType::Gemini => Protocol::Gemini,
        }
    }
}

/// 上游协议:供应商按协议暴露不同端点,代理按请求路径推断协议后取对应端点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Anthropic,
    Openai,
    Gemini,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Anthropic => "anthropic",
            Protocol::Openai => "openai",
            Protocol::Gemini => "gemini",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "anthropic" => Some(Protocol::Anthropic),
            "openai" => Some(Protocol::Openai),
            "gemini" => Some(Protocol::Gemini),
            _ => None,
        }
    }

    pub fn all() -> &'static [Protocol] {
        &[Protocol::Anthropic, Protocol::Openai, Protocol::Gemini]
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
    /// 各协议端点 {anthropic|openai|gemini: base_url}。
    /// 迁移前旧数据只有 base_url 单值;v2 起以此为准,base_url 保留为展示兼容
    #[serde(default)]
    pub endpoints: std::collections::HashMap<String, String>,
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
    /// meta 持久化的 has_key 标记:Some 表示已落库(启动时不再查 keychain,避免授权弹窗)
    #[serde(skip)]
    pub meta_has_key: Option<bool>,
    /// 最近一次测速结果(meta 持久化,重启后卡片仍显示)
    #[serde(default)]
    pub last_test: Option<LastTest>,
}

/// 供应商最近一次测速结果(持久化在 meta)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LastTest {
    pub ok: bool,
    pub latency_ms: u64,
    /// unix 秒
    #[serde(default)]
    pub ts: i64,
}

impl Provider {
    /// 取指定协议的端点地址(无则 None)
    pub fn endpoint(&self, protocol: Protocol) -> Option<&str> {
        self.endpoints.get(protocol.as_str()).map(|s| s.as_str())
    }
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
