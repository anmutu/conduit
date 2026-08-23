//! 首启导入:扫描各 CLI 现有 live 配置,自动创建对应供应商。
//!
//! 规则:该 app 尚无任何 provider 且现有配置里有「非官方、非代理回环」的 base_url
//! 才导入;API Key 一并写入 keychain。

use anyhow::Result;
use serde::Serialize;

use crate::db::{provider_dao, Pool};
use crate::services::{keychain, takeover};
use crate::types::{AppType, Provider};

#[derive(Debug, Serialize)]
pub struct ImportedProvider {
    pub app: String,
    pub name: String,
    pub has_key: bool,
}

fn home_join(rel: &[&str]) -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| rel.iter().fold(h, |p, c| p.join(c)))
}

/// 提取 claude 配置:(base_url, api_key)
fn scan_claude() -> Option<(String, Option<String>)> {
    let path = home_join(&[".claude", "settings.json"])?;
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let env = v.get("env")?;
    let base = env.get("ANTHROPIC_BASE_URL")?.as_str()?.trim().to_string();
    if base.is_empty() || base == takeover::PROXY_URL {
        return None;
    }
    let key = env
        .get("ANTHROPIC_AUTH_KEY")
        .or_else(|| env.get("ANTHROPIC_API_KEY"))
        .and_then(|k| k.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Some((base, key))
}

/// 提取 codex 配置:顶层 model_provider 指向的 provider 的 base_url + auth.json 的 key
fn scan_codex() -> Option<(String, Option<String>)> {
    let path = home_join(&[".codex", "config.toml"])?;
    let raw = std::fs::read_to_string(path).ok()?;
    let doc: toml_edit::DocumentMut = raw.parse().ok()?;
    let active = doc.get("model_provider")?.as_str()?.to_string();
    let base = doc
        .get("model_providers")?
        .get(&active)?
        .get("base_url")?
        .as_str()?
        .trim()
        .to_string();
    if base.is_empty() || base == takeover::PROXY_URL {
        return None;
    }
    let key = home_join(&[".codex", "auth.json"])
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("OPENAI_API_KEY")
                .and_then(|k| k.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    Some((base, key))
}

/// 提取 gemini 配置
fn scan_gemini() -> Option<(String, Option<String>)> {
    let path = home_join(&[".gemini", ".env"])?;
    let raw = std::fs::read_to_string(path).ok()?;
    let mut base = None;
    let mut key = None;
    for line in raw.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("GOOGLE_GEMINI_BASE_URL=") {
            let v = v.trim();
            if !v.is_empty() && v != takeover::PROXY_URL {
                base = Some(v.to_string());
            }
        } else if let Some(v) = t.strip_prefix("GEMINI_API_KEY=") {
            let v = v.trim();
            if !v.is_empty() {
                key = Some(v.to_string());
            }
        }
    }
    base.map(|b| (b, key))
}

/// 扫描并导入;返回本次新建的供应商列表。
pub fn import_existing(pool: &Pool) -> Result<Vec<ImportedProvider>> {
    let mut imported = Vec::new();

    let scanners: Vec<(&str, AppType, fn() -> Option<(String, Option<String>)>)> = vec![
        ("导入的 Claude 配置", AppType::Claude, scan_claude),
        ("导入的 Codex 配置", AppType::Codex, scan_codex),
        ("导入的 Gemini 配置", AppType::Gemini, scan_gemini),
    ];

    for (name, app_type, scan) in scanners {
        // 已有供应商则跳过,避免重复导入
        if !provider_dao::list_by_app(pool, app_type)?.is_empty() {
            continue;
        }
        let Some((base_url, key)) = scan() else {
            continue;
        };
        let id = uuid::Uuid::new_v4().to_string();
        if let Some(k) = &key {
            keychain::store_provider_key(&id, k)?;
        }
        let keychain_ref = Some(id.clone());
        let provider = Provider {
            id,
            app_type,
            name: name.to_string(),
            base_url,
            keychain_id: keychain_ref,
            models: vec![],
            is_current: false,
            is_healthy: true,
            sort_index: 0,
            created_at: chrono::Utc::now().timestamp(),
            has_key: key.is_some(),
        };
        provider_dao::insert(pool, &provider)?;
        tracing::info!("导入 {}: {}", app_type.as_str(), provider.base_url);
        imported.push(ImportedProvider {
            app: app_type.as_str().to_string(),
            name: name.to_string(),
            has_key: key.is_some(),
        });
    }
    Ok(imported)
}
