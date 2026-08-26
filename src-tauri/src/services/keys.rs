//! Key 读写统一入口。
//!
//! v4:主存储 = SQLCipher 加密库(api_keys 表);keychain 里的旧 Key 只做**一次性迁移**
//! (读到即落库,此后不再触碰)。运行期全部走 DB,零 keychain 弹窗。

use anyhow::Result;

use crate::db::{api_key_dao, Pool};
use crate::types::Provider;

/// 读取供应商 Key:DB → (旧数据)keychain 迁移 → None。
pub fn load(pool: &Pool, provider: &Provider) -> Result<Option<String>> {
    if let Some(k) = api_key_dao::get(pool, &provider.id)? {
        return Ok(Some(k));
    }
    // 旧数据:从 keychain 读一次并迁移进 DB(此后不再访问 keychain)
    if let Some(kid) = &provider.keychain_id {
        if let Some(k) = crate::services::keychain::load_provider_key(kid)? {
            let _ = api_key_dao::set(pool, &provider.id, &k);
            tracing::info!(provider = %provider.id, "keychain Key 已迁移至加密库");
            return Ok(Some(k));
        }
    }
    Ok(None)
}

/// 异步版(代理转发用):keychain 迁移读加 8s 超时,避免授权弹窗无人响应时挂住请求。
pub async fn load_async(pool: &Pool, provider: &Provider) -> Option<String> {
    if let Ok(Some(k)) = api_key_dao::get(pool, &provider.id) {
        return Some(k);
    }
    let kid = provider.keychain_id.clone()?;
    let pool2 = pool.clone();
    let pid = provider.id.clone();
    let migrated = tokio::task::spawn_blocking(move || {
        crate::services::keychain::load_provider_key(&kid)
            .ok()
            .flatten()
            .map(|k| {
                let _ = api_key_dao::set(&pool2, &pid, &k);
                k
            })
    })
    .await;
    match migrated {
        Ok(Some(k)) => {
            tracing::info!(provider = %provider.id, "keychain Key 已迁移至加密库(异步)");
            Some(k)
        }
        Ok(None) => None,
        Err(_) => None,
    }
}

/// 保存 Key(仅写加密库)。
pub fn store(pool: &Pool, provider_id: &str, key: &str) -> Result<()> {
    api_key_dao::set(pool, provider_id, key)
}

/// 删除 Key(加密库必删;keychain 旧条目尽力清理,失败不影响)。
pub fn delete(pool: &Pool, provider: &Provider) -> Result<()> {
    api_key_dao::delete(pool, &provider.id)?;
    if let Some(kid) = &provider.keychain_id {
        let _ = crate::services::keychain::delete_provider_key(kid);
    }
    Ok(())
}
