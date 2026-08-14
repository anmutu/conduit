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

const SERVICE: &str = "com.conduit.desktop";
const MASTER_KEY_USER: &str = "db-master-key";

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

/// 存储某供应商的 API Key。
pub fn store_provider_key(keychain_id: &str, key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    entry
        .set_password(key)
        .map_err(|e| anyhow!("写入 API Key 失败: {e}"))
}

/// 读取某供应商的 API Key;未配置返回 `None`。
pub fn load_provider_key(keychain_id: &str) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    match entry.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("读取 API Key 失败: {e}")),
    }
}

/// 检查某供应商是否已配置 Key(不暴露 Key 内容)。
pub fn has_provider_key(keychain_id: &str) -> bool {
    let Ok(entry) = Entry::new(SERVICE, &format!("provider-{keychain_id}")) else {
        return false;
    };
    entry.get_password().is_ok()
}

/// 删除某供应商的 API Key。
pub fn delete_provider_key(keychain_id: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, &format!("provider-{keychain_id}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
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
