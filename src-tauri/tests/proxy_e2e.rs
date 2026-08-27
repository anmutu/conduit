//! 代理端到端集成测试:
//! mock 上游 ← 代理 ← HTTP 客户端,验证「按 current provider 转发」的核心闭环。
//! 不依赖 keychain(provider 不配 Key)、不依赖真实网络。
//! Windows CI runner 上该测试二进制加载失败(STATUS_ENTRYPOINT_NOT_FOUND,
//! 环境性 DLL 问题),Windows 下整体跳过;核心逻辑另有单元测试覆盖。
#![cfg(not(target_os = "windows"))]

use axum::{routing::post, Router};
use conduit_lib::core::proxy;
use conduit_lib::db::{self, provider_dao};
use conduit_lib::state::AppState;
use conduit_lib::types::{AppType, Provider};

fn test_provider(id: &str, name: &str, base_url: String) -> Provider {
    Provider {
        id: id.into(),
        app_type: AppType::Claude,
        name: name.into(),
        base_url,
        keychain_id: None,
        endpoints: std::collections::HashMap::new(),
        models: vec![],
        is_current: false,
        is_healthy: true,
        sort_index: 0,
        created_at: 0,
        has_key: false,
        meta_has_key: None,
    }
}

async fn spawn_mock_upstream(tag: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/messages",
        post(move || async move { axum::Json(serde_json::json!({ "via": tag })) }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://{addr}")
}

#[tokio::test]
async fn proxy_forwards_to_current_provider_and_switches_without_restart() {
    // 1. 两个 mock 上游(切换前后)
    let upstream_a = spawn_mock_upstream("mock-a").await;
    let upstream_b = spawn_mock_upstream("mock-b").await;

    // 2. 临时加密 DB + 两个 provider(A 当前)
    let key = "ab".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_e2e_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("test.db"), &key).unwrap();

    provider_dao::insert(&pool, &test_provider("pa", "ProviderA", upstream_a)).unwrap();
    provider_dao::insert(&pool, &test_provider("pb", "ProviderB", upstream_b)).unwrap();
    provider_dao::set_current(&pool, "pa", AppType::Claude).unwrap();

    // 3. 随机端口起代理
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        AppState::new(pool.clone()),
        listener,
    ));

    let client = reqwest::Client::new();
    let url = format!("http://{proxy_addr}/v1/messages");

    // 4. 初始请求 → 命中 A
    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({"ping": 1}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["via"], "mock-a", "初始应命中当前供应商 A");

    // 5. 只改 DB 指针(= 热切换),同连接再请求 → 命中 B,无需重启代理
    provider_dao::set_current(&pool, "pb", AppType::Claude).unwrap();
    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({"ping": 2}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        resp["via"], "mock-b",
        "切换 is_current 后应立即命中 B(免重启)"
    );

    // 6. 未知路径 → 404(路由分流兜底)
    let resp = client
        .post(format!("http://{proxy_addr}/unknown/path"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let _ = std::fs::remove_dir_all(&db_dir);
}

#[tokio::test]
async fn proxy_returns_502_when_no_current_provider() {
    let key = "cd".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_e2e2_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("test.db"), &key).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(AppState::new(pool), listener));

    let resp = reqwest::Client::new()
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 502, "无当前供应商应返回 502 与可读错误");
    let _ = std::fs::remove_dir_all(&db_dir);
}

#[tokio::test]
async fn failover_switches_to_backup_on_5xx() {
    // 上游 A 恒 500,B 正常
    let a = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/v1/messages",
            post(|| async { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        format!("http://{addr}")
    };
    let b = spawn_mock_upstream("mock-b").await;

    let key = "ef".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_fo_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("t.db"), &key).unwrap();
    provider_dao::insert(&pool, &test_provider("fa", "A", a)).unwrap();
    let mut pb = test_provider("fb", "B", b);
    pb.keychain_id = Some("fb".into()); // 候选链要求备用配置过 Key 引用
    provider_dao::insert(&pool, &pb).unwrap();
    provider_dao::set_current(&pool, "fa", AppType::Claude).unwrap();
    db::kv::set(&pool, "failover:claude", "1").unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        conduit_lib::state::AppState::new(pool),
        listener,
    ));

    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({"x":1}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["via"], "mock-b", "主供应商 5xx 应自动回退到备用");
    let _ = std::fs::remove_dir_all(&db_dir);
}

#[tokio::test]
async fn no_failover_returns_upstream_error_as_is() {
    let a = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/v1/messages",
            post(|| async { (axum::http::StatusCode::BAD_GATEWAY, "upstream down") }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        format!("http://{addr}")
    };
    let key = "fe".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_fo2_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("t.db"), &key).unwrap();
    provider_dao::insert(&pool, &test_provider("na", "A", a)).unwrap();
    provider_dao::set_current(&pool, "na", AppType::Claude).unwrap();
    // failover 未开启:错误原样透传
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        conduit_lib::state::AppState::new(pool),
        listener,
    ));

    let resp = reqwest::Client::new()
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 502, "未开故障转移时上游错误原样透传");
    let _ = std::fs::remove_dir_all(&db_dir);
}

/// 协议转换:Claude(Anthropic 协议)→ 只有 OpenAI 端点的供应商。
/// 非流式请求体转换 + OpenAI JSON 响应转回 Anthropic message。
#[tokio::test]
async fn convert_anthropic_client_to_openai_upstream() {
    // mock OpenAI 上游:/v1/chat/completions
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|body: String| async move {
            // 校验请求已转成 OpenAI 形态
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["messages"][0]["role"], "system");
            assert_eq!(v["messages"][1]["content"], "hi");
            assert_eq!(v["tools"][0]["function"]["name"], "ls");
            axum::Json(serde_json::json!({
                "id": "chatcmpl-1", "model": "test-model",
                "choices": [{ "finish_reason": "tool_calls", "message": {
                    "content": "let me check",
                    "tool_calls": [{ "id": "call_1", "type": "function",
                        "function": { "name": "ls", "arguments": "{\"path\":\"/tmp\"}" } }]
                }}],
                "usage": { "prompt_tokens": 11, "completion_tokens": 7 }
            }))
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await });

    let key = "ab".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_e2e_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("test.db"), &key).unwrap();

    // 供应商只配 openai 端点,无 anthropic 端点 → 触发转换
    let mut p = test_provider("po", "OpenAIOnly", format!("http://{addr}"));
    p.endpoints
        .insert("openai".into(), format!("http://{addr}/v1"));
    provider_dao::insert(&pool, &p).unwrap();
    provider_dao::set_current(&pool, "po", AppType::Claude).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        AppState::new(pool.clone()),
        listener,
    ));

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({
            "model": "test-model", "max_tokens": 100, "stream": false,
            "system": "be brief",
            "messages": [{ "role": "user", "content": "hi" }],
            "tools": [{ "name": "ls", "description": "list", "input_schema": {"type":"object"} }]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["type"], "message", "应返回 Anthropic message 形态");
    assert_eq!(resp["stop_reason"], "tool_use");
    assert_eq!(resp["content"][0]["type"], "text");
    assert_eq!(resp["content"][1]["type"], "tool_use");
    assert_eq!(resp["content"][1]["input"]["path"], "/tmp");
    assert_eq!(resp["usage"]["input_tokens"], 11);
    assert_eq!(resp["usage"]["output_tokens"], 7);
}

/// 协议转换流式:OpenAI SSE → Anthropic SSE(message_start/delta/stop 全链)
#[tokio::test]
async fn convert_streaming_openai_sse_to_anthropic_sse() {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async move {
            let body = concat!(
                "data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n",
                "data: [DONE]\n\n",
            );
            ([(axum::http::header::CONTENT_TYPE, "text/event-stream")], body).into_response()
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await });

    let key = "ab".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_e2e_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("test.db"), &key).unwrap();
    let mut p = test_provider("po2", "OpenAIOnly2", format!("http://{addr}"));
    p.endpoints
        .insert("openai".into(), format!("http://{addr}/v1"));
    provider_dao::insert(&pool, &p).unwrap();
    provider_dao::set_current(&pool, "po2", AppType::Claude).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        AppState::new(pool.clone()),
        listener,
    ));

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({
            "model": "m", "max_tokens": 10, "stream": true,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("text/event-stream"), "content-type: {ct}");
    let body = resp.text().await.unwrap();
    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains("text_delta"), "{body}");
    assert!(body.contains("Hel") && body.contains("lo"), "{body}");
    assert!(body.contains("\"stop_reason\":\"end_turn\""), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
}

/// 协议转换:Claude(Anthropic 协议)→ 只有 Gemini 端点的供应商(非流式)。
#[tokio::test]
async fn convert_anthropic_client_to_gemini_upstream() {
    // mock Gemini 上游:/v1beta/models/{model}:generateContent
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1beta/models/{modelverb}",
        post(|body: String| async move {
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["contents"][0]["parts"][0]["text"], "hi");
            assert_eq!(v["systemInstruction"]["parts"][0]["text"], "brief");
            axum::Json(serde_json::json!({
                "modelVersion": "gemini-test",
                "candidates": [{ "finishReason": "STOP", "content": { "parts": [
                    { "text": "hello from gemini" }
                ]}}],
                "usageMetadata": { "promptTokenCount": 3, "candidatesTokenCount": 4 }
            }))
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await });

    let key = "ab".repeat(32);
    let db_dir = std::env::temp_dir().join(format!("conduit_e2e_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&db_dir).unwrap();
    let pool = db::init_pool(db_dir.join("test.db"), &key).unwrap();
    let mut p = test_provider("pg", "GeminiOnly", format!("http://{addr}"));
    p.endpoints
        .insert("gemini".into(), format!("http://{addr}/v1beta"));
    provider_dao::insert(&pool, &p).unwrap();
    provider_dao::set_current(&pool, "pg", AppType::Claude).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(proxy::server::run_listener(
        AppState::new(pool.clone()),
        listener,
    ));

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("http://{proxy_addr}/v1/messages"))
        .json(&serde_json::json!({
            "model": "gemini-test", "max_tokens": 50, "stream": false,
            "system": "brief",
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["type"], "message", "应返回 Anthropic message 形态");
    assert_eq!(resp["content"][0]["text"], "hello from gemini");
    assert_eq!(resp["stop_reason"], "end_turn");
    assert_eq!(resp["usage"]["input_tokens"], 3);
    assert_eq!(resp["usage"]["output_tokens"], 4);
}
