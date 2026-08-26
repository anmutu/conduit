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

use crate::core::proxy::meter::{MeteredStream, UsageCtx, UsageMeter};
use crate::core::proxy::HOP_BY_HOP;
use crate::db::provider_dao;
use crate::services::keychain;
use crate::state::AppState;
use crate::types::AppType;

/// 启动代理服务(阻塞,应在独立 tokio task 中运行)。
pub async fn run(state: AppState, addr: &str) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("代理监听于 http://{addr}");
    run_listener(state, listener).await
}

/// 在既有 listener 上启动代理(测试用随机端口)。
pub async fn run_listener(state: AppState, listener: tokio::net::TcpListener) -> Result<()> {
    let app = Router::new().fallback(proxy_handler).with_state(state);
    axum::serve(listener, app).await?;
    Ok(())
}

/// 从 URL 路径推断 AppType。
fn infer_app_type(path: &str) -> Option<AppType> {
        for app in AppType::all() {
            for prefix in app.path_prefixes() {
                if path.starts_with(prefix) {
                    return Some(*app);
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

    // 1. 候选链:故障转移开启时 = 当前 + 备用(有 Key),否则仅当前
    let failover_on = crate::db::kv::get(&state.db, &format!("failover:{}", app.as_str()))
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false);
    let candidates: Vec<crate::types::Provider> = match if failover_on {
        provider_dao::failover_candidates(&state.db, app)
    } else {
        provider_dao::get_current(&state.db, app).map(|p| p.into_iter().collect())
    } {
        Ok(list) if !list.is_empty() => list,
        Ok(_) => {
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

    // 2. 取真实 Key(循环内按候选注入)

    // 3. 读请求体(先读一次,重试复用;Bytes 克隆廉价)
    let bytes = match axum::body::to_bytes(body, 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            warn!("读取请求体失败: {e}");
            return text_resp(StatusCode::BAD_REQUEST, "读取请求体失败");
        }
    };

    // 计量 model(一次提取;usage 归属实际成功的供应商)
    let model = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string));

    // 客户端基础 header(剔除 hop-by-hop 与凭证类;凭证按供应商注入)
    let mut base_headers = HeaderMap::new();
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
        base_headers.append(k.clone(), v.clone());
    }
    let protocol = app.protocol();
    if protocol == crate::types::Protocol::Anthropic {
        base_headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
    }

    // 4. 候选链逐个转发:连接失败 / 5xx / 429 视为故障,自动尝试下一个
    let mut fallbacks: Vec<(String, String)> = Vec::new();
    for (i, provider) in candidates.iter().enumerate() {
        let api_key = provider
            .keychain_id
            .as_deref()
            .and_then(|kid| keychain::load_provider_key(kid).ok().flatten());

        // 端点选择:优先该协议端点(v2),退回 base_url(旧数据)
        let base = provider
            .endpoint(protocol)
            .unwrap_or(provider.base_url.as_str())
            .trim_end_matches('/');
        let mut upstream = format!("{base}{path}");
        for prefix in ["/v1", "/v1beta"] {
            let doubled = format!("{prefix}{prefix}/");
            if upstream.contains(&doubled) {
                upstream = upstream.replace(&doubled, &format!("{prefix}/"));
                break;
            }
        }
        let mut query_pairs: Vec<String> = Vec::new();
        if let Some(q) = &query {
            query_pairs.push(q.clone());
        }
        if protocol == crate::types::Protocol::Gemini {
            if let Some(key) = &api_key {
                query_pairs.push(format!("key={key}"));
            }
        }
        if !query_pairs.is_empty() {
            upstream.push('?');
            upstream.push_str(&query_pairs.join("&"));
        }

        // 凭证注入
        let mut req_headers = base_headers.clone();
        if let Some(key) = &api_key {
            if protocol != crate::types::Protocol::Gemini {
                if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                    req_headers.insert(HeaderName::from_static("authorization"), val);
                }
                if let Ok(val) = HeaderValue::from_str(key) {
                    req_headers.insert(HeaderName::from_static("x-api-key"), val);
                }
            }
        }

        let send_result = state
            .http
            .request(method.clone(), &upstream)
            .headers(req_headers)
            .body(bytes.clone())
            .send()
            .await;

        let resp = match send_result {
            Ok(r) => r,
            Err(e) => {
                warn!(upstream = %upstream, "上游连接失败: {e}");
                if i + 1 < candidates.len() {
                    fallbacks.push((provider.name.clone(), candidates[i + 1].name.clone()));
                    continue;
                }
                notify_fallbacks(&state, &fallbacks);
                return text_resp(StatusCode::BAD_GATEWAY, &format!("上游请求失败: {e}"));
            }
        };

        let status = resp.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        if retryable && i + 1 < candidates.len() {
            warn!(upstream = %upstream, %status, "上游故障,切换候选");
            fallbacks.push((provider.name.clone(), candidates[i + 1].name.clone()));
            continue; // 响应体丢弃,试下一个
        }

        // 成功(或不可重试/最后一个):流式回传
        notify_fallbacks(&state, &fallbacks);
        let mut out_headers = HeaderMap::new();
        for (k, v) in resp.headers().iter() {
            if HOP_BY_HOP.contains(&k.as_str()) {
                continue;
            }
            out_headers.append(k.clone(), v.clone());
        }
        // 错误响应附 actionable 提示(x-conduit-hint),CLI 侧可直接看到原因定位
        if let Some(hint) = status_hint(status.as_u16()) {
            if let Ok(val) = HeaderValue::from_str(hint) {
                out_headers.insert(HeaderName::from_static("x-conduit-hint"), val);
            }
        }
        let stream = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        // 用量计量:归属实际服务的供应商
        let meter = if matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini) {
            UsageMeter::new(UsageCtx {
                pool: state.db.clone(),
                app_type: app.as_str().to_string(),
                provider_id: provider.id.clone(),
                model: model.clone(),
                status: status.as_u16(),
            })
        } else {
            UsageMeter::disabled()
        };
        let mut out = Response::new(Body::from_stream(MeteredStream::new(stream, meter)));
        *out.status_mut() = status;
        *out.headers_mut() = out_headers;
        return out;
    }

    // 候选为空(理论不可达)
    text_resp(StatusCode::BAD_GATEWAY, "无可用供应商")
}

/// 常见错误的 actionable 提示(附在响应头 x-conduit-hint)。
fn status_hint(code: u16) -> Option<&'static str> {
    match code {
        401 | 403 => Some("Conduit: 鉴权失败 — API Key 无效或过期,请在 Conduit 中编辑该供应商重新填写 Key"),
        404 => Some("Conduit: 端点不存在 — 请检查该供应商的接口地址(如 anthropic 端点应填根地址、openai 端点带 /v1)"),
        429 => Some("Conduit: 限流/额度不足 — 可在 Conduit 中切换其他供应商或开启故障转移"),
        _ => None,
    }
}

/// 批量通知前端:发生了自动回退
fn notify_fallbacks(state: &AppState, fallbacks: &[(String, String)]) {
    if fallbacks.is_empty() {
        return;
    }
    if let Some(app_handle) = &state.app {
        use tauri::Emitter;
        let chain: Vec<String> = fallbacks
            .iter()
            .map(|(a, b)| format!("{a} → {b}"))
            .collect();
        let _ = app_handle.emit(
            "provider-fallback",
            serde_json::json!({ "chain": chain.join(", ") }),
        );
    }
}

fn text_resp(code: StatusCode, msg: &str) -> Response<Body> {
    (code, msg.to_string()).into_response()
}
