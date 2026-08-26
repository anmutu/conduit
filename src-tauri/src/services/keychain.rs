//! 系统凭证管理。
//!
//! 两类密钥都进 keychain,不落盘明文:
//! 1. **DB 主密钥**:`get_or_create_master_key` 生成一次,供 SQLCipher 使用。
//! 2. **各供应商 API Key**:按 `keychain_id` 存取。
//!
//! keyring v4 在 macOS 用 Keychain、Windows 用 DPAPI、Linux 用 Secret Service。
//! 注意:keyring v4 的 `Entry::new` 是 fallible(返回 `Result`),需用 `?` 解包。

use anyhow::{anyhow, Result};
use keyring::Entry;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

const SERVICE: &str = "com.conduit.desktop";
const MASTER_KEY_USER: &str = "db-master-key";

/// 进程内 key 缓存:keychain_id → Key(读不到为 None,负缓存)。
///
/// macOS 对未签名二进制的每次 keychain 读取都可能弹授权框;
/// 缓存后每个 key 仅首次访问触达 keychain,进程生命周期内不再打扰用户。
static KEY_CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache_get(keychain_id: &str) -> Option<Option<String>> {
    KEY_CACHE
        .lock()
        .ok()
        .and_then(|c| c.get(keychain_id).cloned())
}

fn cache_put(keychain_id: &str, value: Option<String>) {
    if let Ok(mut c) = KEY_CACHE.lock() {
        c.insert(keychain_id.to_string(), value);
    }
}

/// 获取或首次生成 SQLCipher 主密钥(32 字节,十六进制)。
pub fn get_or_create_master_key() -> Result<String> {
    let entry = Entry::new(SERVICE, MASTER_KEY_USER)?;
    match entry.get_password() {
        Ok(existing) => Ok(existing),
        Err(keyring::Error::NoEntry) => {
            let key = generate_master_key();
            entry
                .set_password(&key)
                .map_err(|e| anyhow!("写入主密钥到 keychain 失败: {e}"))?;
            Ok(key)
        }
        Err(e) => Err(anyhow!("读取主密钥失败: {e}")),
    }
}

/// 生成 32 字节随机主密钥的十六进制(64 字符)。
///
/// 用两个 UUID v4(密码学随机源)拼接,等价于 256 bit。
fn generate_master_key() -> String {
    let mut s = String::with_capacity(64);
    s.push_str(&uuid::Uuid::new_v4().simple().to_string());
    s.push_str(&uuid::Uuid::new_v4().simple().to_string());
    s
}

/// 存储某供应商的 API Key(同时写入缓存)。
pub fn store_provider_key(keychain_id: &str, key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    entry
        .set_password(key)
        .map_err(|e| anyhow!("写入 API Key 失败: {e}"))?;
    cache_put(keychain_id, Some(key.to_string()));
    Ok(())
}

/// 读取某供应商的 API Key;未配置返回 `None`。优先走进程内缓存。
pub fn load_provider_key(keychain_id: &str) -> Result<Option<String>> {
    if let Some(cached) = cache_get(keychain_id) {
        return Ok(cached);
    }
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    match entry.get_password() {
        Ok(k) => {
            cache_put(keychain_id, Some(k.clone()));
            Ok(Some(k))
        }
        Err(keyring::Error::NoEntry) => {
            cache_put(keychain_id, None);
            Ok(None)
        }
        Err(e) => Err(anyhow!("读取 API Key 失败: {e}")),
    }
}

/// 检查某供应商是否已配置 Key(不暴露 Key 内容)。优先走缓存。
pub fn has_provider_key(keychain_id: &str) -> bool {
    load_provider_key(keychain_id).unwrap_or(None).is_some()
}

/// 删除某供应商的 API Key(同时清缓存)。
pub fn delete_provider_key(keychain_id: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    match entry.delete_credential() {
        Ok(()) => {
            cache_put(keychain_id, None);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            cache_put(keychain_id, None);
            Ok(())
        }
        Err(e) => Err(anyhow!("删除 API Key 失败: {e}")),
    }
}

/// 自检:验证 keychain 可读写(前端首启向导用)。
pub fn health_check() -> Result<()> {
    let probe = Entry::new(SERVICE, "health-probe")?;
    probe
        .set_password("ok")
        .map_err(|e| anyhow!("keychain 写入探测失败: {e}"))?;
    let v = probe
        .get_password()
        .map_err(|e| anyhow!("keychain 读取探测失败: {e}"))?;
    if v != "ok" {
        return Err(anyhow!("keychain 读写不一致"));
    }
    let _ = probe.delete_credential();
    Ok(())
}
