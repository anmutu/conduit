//! 本地代理 HTTP 服务。
//!
//! 工作流程(每个请求):
//! 1. 按 URL 前缀推断 `AppType`
//! 2. 查该 AppType 的当前供应商(从 DB,带 base_url + keychain_id)
//! 3. 取真实 API Key(SQLCipher 加密库;旧 keychain 数据自动迁移)
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
use tracing::{debug, info, warn};

use crate::core::proxy::meter::{MeteredStream, UsageCtx, UsageMeter};
use crate::core::proxy::HOP_BY_HOP;
use crate::db::provider_dao;
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
    let health_state = state.clone();
    let app = Router::new()
        .route(
            "/healthz",
            axum::routing::get(move || {
                let state = health_state.clone();
                async move {
                    // 各分组当前供应商名(无则 null),便于脚本一眼确认路由去向
                    let mut current = serde_json::Map::new();
                    for app in [AppType::Claude, AppType::Codex, AppType::Gemini] {
                        let name = crate::db::provider_dao::get_current(&state.db, app)
                            .ok()
                            .flatten()
                            .map(|p| serde_json::Value::String(p.name));
                        current
                            .insert(app.as_str().into(), name.unwrap_or(serde_json::Value::Null));
                    }
                    axum::Json(serde_json::json!({
                        "status": "ok",
                        "app": "keyway",
                        "version": env!("CARGO_PKG_VERSION"),
                        "current": current,
                    }))
                }
            }),
        )
        .fallback(proxy_handler)
        .with_state(state);
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
    let mut candidates: Vec<crate::types::Provider> = match if failover_on {
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
    let req_json = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
    // Gemini 客户端的模型名在路径(/v1beta/models/{m}:verb),不在请求体
    let gemini_path_model = path
        .split("/models/")
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .map(str::to_string)
        .filter(|m| !m.is_empty());
    let model = req_json
        .as_ref()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string))
        .or(gemini_path_model);
    // 客户端是否请求流式(协议转换时决定回程处理方式;Gemini 由路径动词决定)
    let wants_stream = req_json
        .as_ref()
        .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
        .unwrap_or(false)
        || path.contains("streamGenerateContent");

    // 长上下文分流:请求体体量估算 token(bytes/4 粗估)超阈值 → 指定供应商提前。
    // 优先级低于显式路由规则(后插入者占据首位),仍是候选链一员,5xx 可回退。
    let mut rule_pattern: Option<String> = None;
    if let Some((lc_pid, threshold)) = crate::db::route_dao::get_longctx(&state.db, app.as_str())
        .ok()
        .flatten()
    {
        // threshold 单位 token;请求体除消息外还有工具定义等,粗估整体即可
        let est_tokens = (bytes.len() / 4) as i64;
        if est_tokens >= threshold {
            if let Some(lc) = crate::db::provider_dao::get_by_id(&state.db, &lc_pid)
                .ok()
                .flatten()
            {
                if let Some(pos) = candidates.iter().position(|p| p.id == lc_pid) {
                    let p = candidates.remove(pos);
                    candidates.insert(0, p);
                } else {
                    candidates.insert(0, lc);
                    candidates.truncate(3);
                }
                if rule_pattern.is_none() {
                    rule_pattern = Some("长上下文".into());
                }
                info!(est_tokens, threshold, provider = %lc_pid, "长上下文分流命中");
            }
        }
    }

    // 深度思考分流:请求体显式开启 thinking,或模型名为 reasoning/thinking 档
    // (OpenRouter 风格 *-thinking / *-reasoning)→ 走指定推理供应商。
    if let Some(th_pid) = crate::db::route_dao::get_think(&state.db, app.as_str())
        .ok()
        .flatten()
    {
        let wants_think = req_json.as_ref().is_some_and(|v| {
            v.get("thinking")
                .and_then(|t| t.get("type"))
                .and_then(|x| x.as_str())
                == Some("enabled")
        }) || model.as_deref().is_some_and(|m| {
            let m = m.to_lowercase();
            m.contains("reasoning") || m.contains("thinking")
        });
        if wants_think {
            if let Some(th) = crate::db::provider_dao::get_by_id(&state.db, &th_pid)
                .ok()
                .flatten()
            {
                if let Some(pos) = candidates.iter().position(|p| p.id == th_pid) {
                    let p = candidates.remove(pos);
                    candidates.insert(0, p);
                } else {
                    candidates.insert(0, th);
                    candidates.truncate(3);
                }
                if rule_pattern.is_none() {
                    rule_pattern = Some("深度思考".into());
                }
                info!(provider = %th_pid, model = ?model, "深度思考分流命中");
            }
        }
    }

    // 后台轻量分流:Claude Code 后台小任务等 Haiku 档请求走指定便宜渠道。
    // 判据是模型名档位(haiku/mini/flash/nano),优先级低于显式路由规则。
    if let Some(bg_pid) = crate::db::route_dao::get_background(&state.db, app.as_str())
        .ok()
        .flatten()
    {
        let is_lite = model.as_deref().is_some_and(|m| {
            let m = m.to_lowercase();
            ["haiku", "-mini", "mini-", "flash", "nano"]
                .iter()
                .any(|k| m.contains(k))
        });
        if is_lite {
            if let Some(bg) = crate::db::provider_dao::get_by_id(&state.db, &bg_pid)
                .ok()
                .flatten()
            {
                if let Some(pos) = candidates.iter().position(|p| p.id == bg_pid) {
                    let p = candidates.remove(pos);
                    candidates.insert(0, p);
                } else {
                    candidates.insert(0, bg);
                    candidates.truncate(3);
                }
                if rule_pattern.is_none() {
                    rule_pattern = Some("后台轻量".into());
                }
                info!(provider = %bg_pid, model = ?model, "后台轻量分流命中");
            }
        }
    }

    // 模型路由规则:命中则把规则供应商提到候选链最前(优先于当前供应商;
    // 后续仍保留故障转移候选,规则供应商 5xx 时可继续回退)
    if let Some((rule_pid, rule_pat, rule_fallback)) =
        crate::db::route_dao::match_provider(&state.db, app.as_str(), model.as_deref())
            .ok()
            .flatten()
    {
        if let Some(rule_provider) = crate::db::provider_dao::get_by_id(&state.db, &rule_pid)
            .ok()
            .flatten()
        {
            if let Some(pos) = candidates.iter().position(|p| p.id == rule_pid) {
                let p = candidates.remove(pos);
                candidates.insert(0, p);
            } else {
                candidates.insert(0, rule_provider);
                candidates.truncate(3);
            }
            rule_pattern = Some(rule_pat);
            info!(rule = %rule_pid, model = ?model, "模型路由规则命中");
            // 规则级降级:命中供应商失败时优先回退到规则指定的供应商(候选链第二位)
            if let Some(fb_pid) = rule_fallback.filter(|id| id != &rule_pid) {
                if let Some(fb) = crate::db::provider_dao::get_by_id(&state.db, &fb_pid)
                    .ok()
                    .flatten()
                {
                    if let Some(pos) = candidates.iter().position(|p| p.id == fb_pid) {
                        let p = candidates.remove(pos);
                        candidates.insert(1, p);
                    } else {
                        candidates.insert(1, fb);
                        candidates.truncate(3);
                    }
                }
            }
        }
    }

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

    // 3.5 故障冷却:窗口内刚失败过的供应商挪到链尾(仍有健康候选时);
    // 全员冷却则保持原序(fail-open,不能因为冷却把整条链判死)。
    {
        let now = chrono::Utc::now().timestamp();
        let mut cooled: Vec<crate::types::Provider> = Vec::new();
        candidates.retain(|p| {
            let until = crate::db::kv::get(&state.db, &format!("cooldown.{}", p.id))
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if until > now {
                cooled.push(p.clone());
                false
            } else {
                true
            }
        });
        if !candidates.is_empty() {
            candidates.append(&mut cooled);
        } else {
            candidates = cooled;
        }
    }

    // 4. 候选链逐个转发:连接失败 / 5xx / 429 视为故障,自动尝试下一个
    // 逐条 "A→B(原因)",供前端通知与 x-keyway-fallback 响应头
    let mut fallbacks: Vec<String> = Vec::new();
    for (i, provider) in candidates.iter().enumerate() {
        // Key 读取:加密库优先;旧数据走 keychain 一次性迁移(带超时,弹窗无人响应也不挂请求)
        let api_key = crate::services::keys::load_async(&state.db, provider).await;

        // 端点选择:优先该协议端点(v2),退回 base_url(旧数据)。
        // 协议转换:app 是 Anthropic 协议但供应商只有 OpenAI/Gemini 端点时,
        // 自动改走对应上游路径并做双向转换(对 CLI 透明)。
        #[derive(PartialEq, Clone, Copy)]
        enum Conv {
            None,
            Openai,
            Gemini,
            /// 反向:OpenAI 协议客户端(chat/completions)→ Anthropic 上游
            AnthropicUpstream,
            /// 反向:Gemini 协议客户端(generateContent)→ OpenAI 上游
            GeminiClientOpenai,
            /// Responses 协议客户端(Codex,/v1/responses)→ chat/completions 上游。
            /// 按供应商 KV responses.bridge.{pid} 显式开启。
            ResponsesBridge,
        }
        let mut conv = Conv::None;
        // /v1/responses 路径的请求体是 Responses API,不在反向转换范围
        let reversible_path = path.contains("chat/completions");
        // Responses 桥接:Codex 的 /v1/responses 请求 → chat/completions 上游。
        // 显式开关(官方 OpenAI 原生支持 /responses,不需要桥接)。
        let responses_bridge = protocol == crate::types::Protocol::Openai
            && path.contains("/responses")
            && crate::db::kv::get(&state.db, &format!("responses.bridge.{}", provider.id))
                .ok()
                .flatten()
                .as_deref()
                == Some("1");
        let base = if responses_bridge {
            match provider.endpoint(crate::types::Protocol::Openai) {
                Some(b) => b.trim_end_matches('/').to_string(),
                None => provider.base_url.trim_end_matches('/').to_string(),
            }
        } else {
            match provider.endpoint(protocol) {
                Some(b) => b.trim_end_matches('/').to_string(),
                None => {
                    if protocol == crate::types::Protocol::Anthropic {
                        if let Some(b) = provider.endpoint(crate::types::Protocol::Openai) {
                            conv = Conv::Openai;
                            b.trim_end_matches('/').to_string()
                        } else if let Some(b) = provider.endpoint(crate::types::Protocol::Gemini) {
                            conv = Conv::Gemini;
                            b.trim_end_matches('/').to_string()
                        } else {
                            provider.base_url.trim_end_matches('/').to_string()
                        }
                    } else if protocol == crate::types::Protocol::Openai && reversible_path {
                        if let Some(b) = provider.endpoint(crate::types::Protocol::Anthropic) {
                            conv = Conv::AnthropicUpstream;
                            b.trim_end_matches('/').to_string()
                        } else {
                            provider.base_url.trim_end_matches('/').to_string()
                        }
                    } else if protocol == crate::types::Protocol::Gemini {
                        if let Some(b) = provider.endpoint(crate::types::Protocol::Openai) {
                            conv = Conv::GeminiClientOpenai;
                            b.trim_end_matches('/').to_string()
                        } else {
                            provider.base_url.trim_end_matches('/').to_string()
                        }
                    } else {
                        provider.base_url.trim_end_matches('/').to_string()
                    }
                }
            }
        };
        if responses_bridge {
            conv = Conv::ResponsesBridge;
        }
        let path: String = match conv {
            Conv::AnthropicUpstream => {
                if base.ends_with("/v1") {
                    "/messages".into()
                } else {
                    "/v1/messages".into()
                }
            }
            Conv::Openai | Conv::GeminiClientOpenai | Conv::ResponsesBridge => {
                // 端点一般以 /v1 结尾;没有则补上
                if base.ends_with("/v1") {
                    "/chat/completions".into()
                } else {
                    "/v1/chat/completions".into()
                }
            }
            Conv::Gemini => {
                // Gemini 按模型生成路径;无模型名时无法转换,回退直连
                match &model {
                    Some(m) => {
                        let verb = if wants_stream {
                            "streamGenerateContent?alt=sse"
                        } else {
                            "generateContent"
                        };
                        if base.ends_with("/v1beta") {
                            format!("/models/{m}:{verb}")
                        } else {
                            format!("/v1beta/models/{m}:{verb}")
                        }
                    }
                    None => {
                        conv = Conv::None;
                        path.to_string()
                    }
                }
            }
            Conv::None => path.to_string(),
        };
        let mut upstream = format!("{base}{path}");
        for prefix in ["/v1", "/v1beta"] {
            let doubled = format!("{prefix}{prefix}/");
            if upstream.contains(&doubled) {
                upstream = upstream.replace(&doubled, &format!("{prefix}/"));
                break;
            }
        }
        let mut query_pairs: Vec<String> = Vec::new();
        // 转到 OpenAI 上游时不透传 Gemini 的 alt=sse 等原查询串
        if let Some(q) = &query {
            if conv != Conv::GeminiClientOpenai {
                query_pairs.push(q.clone());
            }
        }
        if protocol == crate::types::Protocol::Gemini && conv == Conv::None || conv == Conv::Gemini
        {
            if let Some(key) = &api_key {
                query_pairs.push(format!("key={key}"));
            }
        }
        if !query_pairs.is_empty() {
            upstream.push('?');
            upstream.push_str(&query_pairs.join("&"));
        }

        // 凭证注入(Gemini 走 query key;转换到 Gemini 上游同理,上面已处理;
        // Gemini 客户端转到 OpenAI 上游时仍用 Bearer)
        let mut req_headers = base_headers.clone();
        if let Some(key) = &api_key {
            if (protocol != crate::types::Protocol::Gemini || conv == Conv::GeminiClientOpenai)
                && conv != Conv::Gemini
            {
                if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                    req_headers.insert(HeaderName::from_static("authorization"), val);
                }
                if let Ok(val) = HeaderValue::from_str(key) {
                    req_headers.insert(HeaderName::from_static("x-api-key"), val);
                }
            }
        }

        // 反向转换:Anthropic 上游需要版本头(客户端是 OpenAI 协议,不会自带)
        if conv == Conv::AnthropicUpstream {
            req_headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static("2023-06-01"),
            );
            req_headers.remove("x-api-key");
            if let Some(key) = &api_key {
                if let Ok(val) = HeaderValue::from_str(key) {
                    req_headers.insert(HeaderName::from_static("x-api-key"), val);
                }
            }
        }

        // 协议转换:请求体双向映射(Anthropic↔OpenAI / Anthropic↔Gemini)
        let send_body = match conv {
            Conv::None => bytes.clone(),
            Conv::Openai => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => bytes::Bytes::from(super::convert::request(&v).to_string()),
                Err(_) => bytes.clone(),
            },
            Conv::Gemini => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => bytes::Bytes::from(super::gemini_convert::request(&v).to_string()),
                Err(_) => bytes.clone(),
            },
            Conv::AnthropicUpstream => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => bytes::Bytes::from(super::anthropic_upstream::request(&v).to_string()),
                Err(_) => bytes.clone(),
            },
            Conv::ResponsesBridge => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => bytes::Bytes::from(super::responses_convert::request(&v).to_string()),
                Err(_) => bytes.clone(),
            },
            Conv::GeminiClientOpenai => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => bytes::Bytes::from(
                    super::gemini_client_convert::request(&v, model.as_deref().unwrap_or(""))
                        .to_string(),
                ),
                Err(_) => bytes.clone(),
            },
        };

        let send_result = state
            .http
            .request(method.clone(), &upstream)
            .headers(req_headers)
            .body(send_body)
            .send()
            .await;

        let resp = match send_result {
            Ok(r) => r,
            Err(e) => {
                warn!(upstream = %upstream, "上游连接失败: {e}");
                if i + 1 < candidates.len() {
                    fallbacks.push(format!(
                        "{}->{}(conn)",
                        provider.name,
                        candidates[i + 1].name
                    ));
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
            set_cooldown(&state, &provider.id);
            fallbacks.push(format!(
                "{}->{}({})",
                provider.name,
                candidates[i + 1].name,
                status.as_u16()
            ));
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
        // 分流可观测:命中路由规则/长上下文时回写标记头
        if let Some(pat) = &rule_pattern {
            if let Ok(val) = HeaderValue::from_str(pat) {
                out_headers.insert(HeaderName::from_static("x-keyway-route"), val);
            }
        }
        // 降级可观测:发生过自动回退时回写链路与原因(参考 x-ccr-fallback-attempts)
        if !fallbacks.is_empty() {
            if let Ok(val) = HeaderValue::from_str(&fallbacks.join(",")) {
                out_headers.insert(HeaderName::from_static("x-keyway-fallback"), val);
            }
        }
        // 协议转换回程:流式包 ConvertStream,非流式整读后转换 JSON
        let upstream_is_sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("text/event-stream"))
            .unwrap_or(false);
        if conv != Conv::None && status.is_success() && (wants_stream || upstream_is_sse) {
            let raw = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::other(e.to_string()));
            let meter2 = meter_for(
                &state,
                app,
                provider,
                &model,
                status.as_u16(),
                &rule_pattern,
            );
            let mut out = match conv {
                Conv::Gemini => {
                    let converted = super::gemini_convert::GeminiConvertStream::new(raw);
                    Response::new(Body::from_stream(MeteredStream::new(converted, meter2)))
                }
                Conv::AnthropicUpstream => {
                    let converted = super::anthropic_upstream::AnthropicConvertStream::new(raw);
                    Response::new(Body::from_stream(MeteredStream::new(converted, meter2)))
                }
                Conv::GeminiClientOpenai => {
                    let converted =
                        super::gemini_client_convert::GeminiClientConvertStream::new(raw);
                    Response::new(Body::from_stream(MeteredStream::new(converted, meter2)))
                }
                Conv::ResponsesBridge => {
                    let converted =
                        super::responses_convert::ResponsesBridgeConvertStream::new(raw);
                    Response::new(Body::from_stream(MeteredStream::new(converted, meter2)))
                }
                _ => {
                    let converted = super::convert::ConvertStream::new(raw);
                    Response::new(Body::from_stream(MeteredStream::new(converted, meter2)))
                }
            };
            *out.status_mut() = status;
            out_headers.remove("content-length");
            out_headers.insert(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/event-stream"),
            );
            *out.headers_mut() = out_headers;
            return out;
        }
        if conv != Conv::None && status.is_success() {
            // 非流式:读全量 → JSON 转换
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    warn!("读取上游响应失败: {e}");
                    return text_resp(StatusCode::BAD_GATEWAY, "读取上游响应失败");
                }
            };
            let converted = serde_json::from_slice::<serde_json::Value>(&body_bytes)
                .ok()
                .map(|v| match conv {
                    Conv::Gemini => super::gemini_convert::response(&v),
                    Conv::AnthropicUpstream => super::anthropic_upstream::response(&v),
                    Conv::GeminiClientOpenai => super::gemini_client_convert::response(&v),
                    Conv::ResponsesBridge => super::responses_convert::response(&v),
                    _ => super::convert::response(&v),
                })
                .map(|v| v.to_string())
                .unwrap_or_else(|| String::from_utf8_lossy(&body_bytes).into_owned());
            let mut meter3 = meter_for(
                &state,
                app,
                provider,
                &model,
                status.as_u16(),
                &rule_pattern,
            );
            meter3.observe(converted.as_bytes());
            meter3.finish();
            out_headers.remove("content-length");
            out_headers.insert(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("application/json"),
            );
            let mut out = Response::new(Body::from(converted));
            *out.status_mut() = status;
            *out.headers_mut() = out_headers;
            return out;
        }

        let stream = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::other(e.to_string()));
        // 用量计量:归属实际服务的供应商
        let meter = if matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini) {
            UsageMeter::new(UsageCtx {
                pool: state.db.clone(),
                app_type: app.as_str().to_string(),
                provider_id: provider.id.clone(),
                model: model.clone(),
                status: status.as_u16(),
                rule_pattern: rule_pattern.clone(),
                app: state.app.clone(),
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

/// 构造用量计量器(归实际服务的供应商;协议转换路径复用)
fn meter_for(
    state: &AppState,
    app: AppType,
    provider: &crate::types::Provider,
    model: &Option<String>,
    status: u16,
    rule_pattern: &Option<String>,
) -> UsageMeter {
    if matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini) {
        UsageMeter::new(UsageCtx {
            pool: state.db.clone(),
            app_type: app.as_str().to_string(),
            provider_id: provider.id.clone(),
            model: model.clone(),
            status,
            rule_pattern: rule_pattern.clone(),
            app: state.app.clone(),
        })
    } else {
        UsageMeter::disabled()
    }
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

/// 失败冷却时长(秒):期间该供应商在候选链中被后移
const COOLDOWN_SECS: i64 = 120;

/// 记录供应商失败冷却起点(仅故障转移场景调用;单候选场景无意义)。
fn set_cooldown(state: &AppState, pid: &str) {
    let until = chrono::Utc::now().timestamp() + COOLDOWN_SECS;
    let _ = crate::db::kv::set(&state.db, &format!("cooldown.{pid}"), &until.to_string());
}

/// 批量通知前端:发生了自动回退
fn notify_fallbacks(state: &AppState, fallbacks: &[String]) {
    if fallbacks.is_empty() {
        return;
    }
    if let Some(app_handle) = &state.app {
        use tauri::Emitter;
        let _ = app_handle.emit(
            "provider-fallback",
            serde_json::json!({ "chain": fallbacks.join(", ") }),
        );
    }
}

fn text_resp(code: StatusCode, msg: &str) -> Response<Body> {
    (code, msg.to_string()).into_response()
}
