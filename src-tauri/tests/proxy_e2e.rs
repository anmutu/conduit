//! 代理端到端集成测试:
//! mock 上游 ← 代理 ← HTTP 客户端,验证「按 current provider 转发」的核心闭环。
//! 不依赖 keychain(provider 不配 Key)、不依赖真实网络。

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
        models: vec![],
        is_current: false,
        is_healthy: true,
        sort_index: 0,
        created_at: 0,
        has_key: false,
    }
}

async fn spawn_mock_upstream(tag: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/messages",
        post(move || async move {
            axum::Json(serde_json::json!({ "via": tag }))
        }),
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
    tokio::spawn(proxy::server::run_listener(AppState::new(pool.clone()), listener));

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
    assert_eq!(resp["via"], "mock-b", "切换 is_current 后应立即命中 B(免重启)");

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
