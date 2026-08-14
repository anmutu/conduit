//! 本地代理 HTTP 服务。
//!
//! 工作流程(每个请求):
//! 1. 按 URL 前缀推断 `AppType`
//! 2. 查该 AppType 的当前供应商(从 DB,带 base_url + keychain_id)
//! 3. 从 keychain 取真实 API Key(不在 DB、不落盘)
//! 4. 按 AppType 注入正确形式的凭证(Anthropic 用 `x-api-key`,OpenAI 用 `Authorization`,
//!    Gemini 用 URL `?key=`),并保留客户端原始 header
//! 5. 流式转发响应(支持 SSE)
//!
//! M0 只做透传 + 凭证注入;计量/故障转移/熔断留给 M1/M2。

use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    Router,
};
use futures::TryStreamExt;
use tracing::{debug, warn};

use crate::core::proxy::HOP_BY_HOP;
use crate::db::provider_dao;
use crate::services::keychain;
use crate::state::AppState;
use crate::types::AppType;

/// 启动代理服务(阻塞,应在独立 tokio task 中运行)。
pub async fn run(state: AppState, addr: &str) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("代理监听于 http://{addr}");
    let app = Router::new().fallback(proxy_handler).with_state(state);
    axum::serve(listener, app).await?;
    Ok(())
}

/// 从 URL 路径推断 AppType。
fn infer_app_type(path: &str) -> Option<AppType> {
    for app in AppType::all() {
        for prefix in app.path_prefixes() {
            if path.starts_with(prefix) {
                return Some(app);
            }
        }
    }
    None
}

/// 主转发处理器。
pub async fn proxy_handler(State(state): State<AppState>, req: Request<Body>) -> Response<Body> {
    // 拆分请求:method/uri/headers 与 body 分开处理,避免消费后丢失 header
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let path = parts.uri.path().to_string();
    let query = parts.uri.query().map(str::to_string);
    let client_headers = parts.headers;
    let app_type = infer_app_type(&path);

    debug!(%method, %path, ?app_type, "代理请求");

    let Some(app) = app_type else {
        return text_resp(StatusCode::NOT_FOUND, &format!("无法识别的路径: {path}"));
    };

    // 1. 取当前供应商
    let provider = match provider_dao::get_current(&state.db, app) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return text_resp(
                StatusCode::BAD_GATEWAY,
                &format!("未设置 {} 的当前供应商,请先在 Conduit 中切换", app.as_str()),
            )
        }
        Err(e) => {
            warn!("查询当前供应商失败: {e}");
            return text_resp(StatusCode::INTERNAL_SERVER_ERROR, "数据库查询失败");
        }
    };

    // 2. 取真实 Key
    let api_key = provider
        .keychain_id
        .as_deref()
        .and_then(|kid| keychain::load_provider_key(kid).ok().flatten());

    // 3. 组装上游 URL
    let base = provider.base_url.trim_end_matches('/');
    let mut upstream = format!("{base}{path}");
    let mut query_pairs: Vec<String> = Vec::new();
    if let Some(q) = &query {
        query_pairs.push(q.clone());
    }
    // Gemini 把凭证放 URL query
    if matches!(app, AppType::Gemini) {
        if let Some(key) = &api_key {
            query_pairs.push(format!("key={key}"));
        }
    }
    if !query_pairs.is_empty() {
        upstream.push('?');
        upstream.push_str(&query_pairs.join("&"));
    }

    // 4. 读请求体(LLM 请求体不大,64MB 上限足够覆盖图文混合)
    let bytes = match axum::body::to_bytes(body, 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            warn!("读取请求体失败: {e}");
            return text_resp(StatusCode::BAD_REQUEST, "读取请求体失败");
        }
    };

    // 5. 构造上游 header:复制客户端原始 header(剔除 hop-by-hop 与凭证类,
    //    凭证由我们统一注入,避免泄露客户端本地残留 key)
    let mut req_headers = HeaderMap::new();
    const OVERRIDE: &[&str] = &[
        "authorization",
        "x-api-key",
        "anthropic-version",
        "content-length",
    ];
    for (k, v) in client_headers.iter() {
        let name = k.as_str();
        if HOP_BY_HOP.contains(&name) || OVERRIDE.contains(&name) {
            continue;
        }
        req_headers.append(k.clone(), v.clone());
    }
    // 凭证注入(非 Gemini)
    if let Some(key) = &api_key {
        if !matches!(app, AppType::Gemini) {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                req_headers.insert(HeaderName::from_static("authorization"), val);
            }
            if let Ok(val) = HeaderValue::from_str(key) {
                req_headers.insert(HeaderName::from_static("x-api-key"), val);
            }
        }
    }
    if matches!(app, AppType::Claude | AppType::OpenCode | AppType::OpenClaw) {
        req_headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
    }

    // 6. 发起上游请求
    let upstream_req = state
        .http
        .request(method, &upstream)
        .headers(req_headers)
        .body(bytes);
    let resp = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(upstream = %upstream, "上游请求失败: {e}");
            return text_resp(StatusCode::BAD_GATEWAY, &format!("上游请求失败: {e}"));
        }
    };

    // 7. 流式回传
    let status = resp.status();
    let mut out_headers = HeaderMap::new();
    for (k, v) in resp.headers().iter() {
        if HOP_BY_HOP.contains(&k.as_str()) {
            continue;
        }
        out_headers.append(k.clone(), v.clone());
    }
    let stream = resp
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
    let mut out = Response::new(Body::from_stream(stream));
    *out.status_mut() = status;
    *out.headers_mut() = out_headers;
    out
}

fn text_resp(code: StatusCode, msg: &str) -> Response<Body> {
    (code, msg.to_string()).into_response()
}
