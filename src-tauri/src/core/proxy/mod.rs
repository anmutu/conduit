pub mod anthropic_upstream;
pub mod convert;
pub mod gemini_client_convert;
pub mod gemini_convert;
pub mod meter;
pub mod responses_convert;
pub mod server;

/// 代理默认监听地址。
pub const PROXY_ADDR: &str = "127.0.0.1:9527";

/// hop-by-hop 头,转发时需剔除(见 RFC 7230 §6.1)。
pub const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length", // reqwest 依 body 重算
];
