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

/// 提取 opencode 配置:第一个带 baseURL 的 provider(options.apiKey 可能为空)
fn scan_opencode_at(path: &std::path::Path) -> Option<(String, Option<String>)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let providers = v.get("provider")?.as_object()?;
    for (_, p) in providers {
        let opts = p.get("options")?;
        let base = opts.get("baseURL")?.as_str()?.trim().to_string();
        if base.is_empty() || base.starts_with(takeover::PROXY_URL) {
            continue;
        }
        let key = opts
            .get("apiKey")
            .and_then(|k| k.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.starts_with("{env:"));
        return Some((base, key));
    }
    None
}

/// 提取 openclaw 配置(JSON5):第一个带 baseUrl 的 provider
fn scan_openclaw_at(path: &std::path::Path) -> Option<(String, Option<String>)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = json5::from_str(&raw).ok()?;
    let providers = v.get("models")?.get("providers")?.as_object()?;
    for (id, p) in providers {
        let base = p.get("baseUrl")?.as_str()?.trim().to_string();
        if base.is_empty() || base.starts_with(takeover::PROXY_URL) {
            continue;
        }
        // "${ENV_VAR}" 形式的占位 Key 无法导入,跳过
        let key = p
            .get("apiKey")
            .and_then(|k| k.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.starts_with("${"));
        let _ = id;
        return Some((base, key));
    }
    None
}

fn scan_opencode() -> Option<(String, Option<String>)> {
    scan_opencode_at(&home_join(&[".config", "opencode", "opencode.json"])?)
}

fn scan_openclaw() -> Option<(String, Option<String>)> {
    scan_openclaw_at(&home_join(&[".openclaw", "openclaw.json"])?)
}

/// 扫描并导入;返回本次新建的供应商列表。
pub fn import_existing(pool: &Pool) -> Result<Vec<ImportedProvider>> {
    let mut imported = Vec::new();

    #[allow(clippy::type_complexity)]
    let scanners: Vec<(&str, AppType, fn() -> Option<(String, Option<String>)>)> = vec![
        ("导入的 Claude 配置", AppType::Claude, scan_claude),
        ("导入的 Codex 配置", AppType::Codex, scan_codex),
        ("导入的 Gemini 配置", AppType::Gemini, scan_gemini),
        ("导入的 OpenCode 配置", AppType::OpenCode, scan_opencode),
        ("导入的 OpenClaw 配置", AppType::OpenClaw, scan_openclaw),
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
        let mut endpoints = std::collections::HashMap::new();
        endpoints.insert(app_type.protocol().as_str().to_string(), base_url.clone());
        let provider = Provider {
            id,
            app_type,
            name: name.to_string(),
            base_url,
            endpoints,
            keychain_id: keychain_ref,
            models: vec![],
            is_current: false,
            is_healthy: true,
            sort_index: 0,
            created_at: chrono::Utc::now().timestamp(),
            has_key: key.is_some(),
            meta_has_key: None,
            last_test: None, // insert 会按 has_key 写入 meta
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpfile(tag: &str, content: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "conduit_import_test_{tag}_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(tag);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn scan_opencode_picks_first_base_url_and_key() {
        let p = tmpfile(
            "opencode.json",
            r#"{"provider":{"a":{"options":{"baseURL":"http://127.0.0.1:9527/v1","apiKey":"x"}},"b":{"options":{"baseURL":"https://real.example/v1","apiKey":"sk-1"}}}}"#,
        );
        let (base, key) = scan_opencode_at(&p).unwrap();
        assert_eq!(base, "https://real.example/v1");
        assert_eq!(key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn scan_opencode_skips_env_placeholder_key() {
        let p = tmpfile(
            "opencode2.json",
            r#"{"provider":{"a":{"options":{"baseURL":"https://x.example/v1","apiKey":"{env:MY_KEY}"}}}}"#,
        );
        let (base, key) = scan_opencode_at(&p).unwrap();
        assert_eq!(base, "https://x.example/v1");
        assert!(key.is_none());
    }

    #[test]
    fn scan_openclaw_json5_and_env_key() {
        let p = tmpfile(
            "openclaw.json",
            r#"{
  // comment
  "models": { "providers": { "kimi": {
      "baseUrl": "https://api.moonshot.ai/v1",
      "apiKey": "${KIMI_KEY}",
      "models": [],
  } } },
}"#,
        );
        let (base, key) = scan_openclaw_at(&p).unwrap();
        assert_eq!(base, "https://api.moonshot.ai/v1");
        assert!(key.is_none()); // ${} 占位不导入
    }
}
