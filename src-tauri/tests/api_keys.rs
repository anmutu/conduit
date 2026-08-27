//! api_keys 加密库存取集成测试。

use conduit_lib::db::{self, api_key_dao, provider_dao};
use conduit_lib::types::{AppType, Provider};

fn make_provider() -> Provider {
    Provider {
        id: "p1".into(),
        app_type: AppType::Codex,
        name: "T".into(),
        base_url: "http://x".into(),
        endpoints: std::collections::HashMap::new(),
        keychain_id: None,
        models: vec![],
        is_current: false,
        is_healthy: true,
        sort_index: 0,
        created_at: 0,
        has_key: false,
        meta_has_key: None,
        last_test: None,
    }
}

#[test]
fn api_key_roundtrip_and_delete() {
    let dir = std::env::temp_dir().join(format!("conduit_ak_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::init_pool(dir.join("t.db"), &"ab".repeat(32)).unwrap();

    let p = make_provider();
    provider_dao::insert(&pool, &p).unwrap();

    // 未设置 → None
    assert!(api_key_dao::get(&pool, "p1").unwrap().is_none());
    // 写入 → 读回
    api_key_dao::set(&pool, "p1", "sk-test").unwrap();
    assert_eq!(
        api_key_dao::get(&pool, "p1").unwrap().as_deref(),
        Some("sk-test")
    );
    // 覆盖更新
    api_key_dao::set(&pool, "p1", "sk-new").unwrap();
    assert_eq!(
        api_key_dao::get(&pool, "p1").unwrap().as_deref(),
        Some("sk-new")
    );
    // 删除
    api_key_dao::delete(&pool, "p1").unwrap();
    assert!(api_key_dao::get(&pool, "p1").unwrap().is_none());

    let _ = std::fs::remove_dir_all(&dir);
}
