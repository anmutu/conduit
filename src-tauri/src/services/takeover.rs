//! takeover 接管:把各 CLI 的 live 配置改写为指向本地代理,实现免重启热切换。
//!
//! 覆盖(M1):Claude / Codex / Gemini;OpenCode / OpenClaw 为多供应商共存结构,后续版本支持。
//!
//! 安全设计:
//! - 接管前把原始数据备份进加密 DB(settings 表),可随时一键还原
//! - 原子写(临时文件 + rename),中断不损坏配置
//! - 状态三态:active(我们标记过)/ effective(配置当前确实指向代理,
//!   可检测被 CLI 或其他工具覆盖的情况)

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::db::Pool;
use crate::types::AppType;

/// 本地代理地址(与 core::proxy::PROXY_ADDR 保持一致)
pub const PROXY_URL: &str = "http://127.0.0.1:9527";

#[derive(Debug, Serialize)]
pub struct TakeoverStatus {
    pub app: String,
    /// 是否支持接管(M1 仅 claude/codex/gemini)
    pub supported: bool,
    /// live 配置文件是否存在
    pub config_exists: bool,
    /// 已由 Keyway 接管(内部标记)
    pub active: bool,
    /// 配置当前确实指向代理(未被外部覆盖)
    pub effective: bool,
    /// 故障转移开关
    pub failover: bool,
}

// ---------- settings KV ----------

fn settings_get(pool: &Pool, key: &str) -> Result<Option<String>> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(rusqlite::params![key], |r| r.get::<_, String>(0))?;
    Ok(rows.next().and_then(|r| r.ok()))
}

fn settings_set(pool: &Pool, key: &str, value: &str) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

fn settings_del(pool: &Pool, key: &str) -> Result<()> {
    let conn = pool.get().map_err(|e| anyhow!("{e}"))?;
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        rusqlite::params![key],
    )?;
    Ok(())
}

fn active_key(app: AppType) -> String {
    format!("takeover_active:{}", app.as_str())
}
fn backup_key(app: AppType) -> String {
    format!("takeover_backup:{}", app.as_str())
}

// ---------- 通用:原子写 ----------

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("conduit-tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn home_path(rel: &[&str]) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("无法定位用户目录"))?;
    Ok(rel.iter().fold(home, |p, c| p.join(c)))
}

// ---------- Claude: ~/.claude/settings.json ----------

fn claude_settings_path() -> Result<PathBuf> {
    home_path(&[".claude", "settings.json"])
}

/// 备份内容:{"base_url": old-or-null}
pub fn apply_claude_at(path: &Path) -> Result<String> {
    let mut root: serde_json::Value = if path.exists() {
        let raw = std::fs::read_to_string(path)?;
        if raw.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&raw).context("claude settings.json 不是合法 JSON")?
        }
    } else {
        serde_json::json!({})
    };
    let old = root
        .get("env")
        .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let backup = serde_json::json!({ "base_url": old }).to_string();

    root.as_object_mut()
        .ok_or_else(|| anyhow!("settings.json 顶层必须是对象"))?
        .entry("env")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow!("env 必须是对象"))?
        .insert(
            "ANTHROPIC_BASE_URL".into(),
            serde_json::Value::String(PROXY_URL.into()),
        );
    write_atomic(path, &serde_json::to_string_pretty(&root)?)?;
    Ok(backup)
}

pub fn restore_claude_at(path: &Path, backup: &str) -> Result<()> {
    let b: serde_json::Value = serde_json::from_str(backup)?;
    let old = b.get("base_url").and_then(|v| v.as_str());
    let mut root: serde_json::Value = if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(path)?)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let env = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.json 顶层必须是对象"))?
        .entry("env")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow!("env 必须是对象"))?;
    match old {
        Some(url) => {
            env.insert(
                "ANTHROPIC_BASE_URL".into(),
                serde_json::Value::String(url.into()),
            );
        }
        None => {
            env.remove("ANTHROPIC_BASE_URL");
        }
    }
    write_atomic(path, &serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn claude_effective(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("env")
                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                .and_then(|u| u.as_str().map(|s| s == PROXY_URL))
        })
        .unwrap_or(false)
}

// ---------- Codex: ~/.codex/config.toml ----------

fn codex_config_path() -> Result<PathBuf> {
    home_path(&[".codex", "config.toml"])
}

/// 改写所有 [model_providers.*].base_url 指向代理;无 model_providers 时创建最小条目。
/// 备份 = 原始 toml 全文。toml_edit 保注释保格式。
pub fn apply_codex_at(path: &Path) -> Result<String> {
    let original = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let mut doc: toml_edit::DocumentMut = original
        .parse()
        .map_err(|e| anyhow!("config.toml 解析失败: {e}"))?;

    let providers = doc["model_providers"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("model_providers 必须是表"))?;

    if providers.is_empty() {
        // 无任何 provider:创建指向代理的最小条目并设为默认
        let mut t = toml_edit::Table::new();
        t["name"] = toml_edit::value("Keyway");
        t["base_url"] = toml_edit::value(PROXY_URL);
        t["wire_api"] = toml_edit::value("chat");
        providers["keyway"] = toml_edit::Item::Table(t);
        doc["model_provider"] = toml_edit::value("keyway");
    } else {
        for (_, item) in providers.iter_mut() {
            if let Some(t) = item.as_table_mut() {
                if t.contains_key("base_url") {
                    t["base_url"] = toml_edit::value(PROXY_URL);
                }
            }
        }
    }
    write_atomic(path, &doc.to_string())?;
    Ok(original)
}

pub fn restore_codex_at(path: &Path, backup: &str) -> Result<()> {
    if backup.is_empty() && !path.exists() {
        return Ok(());
    }
    write_atomic(path, backup)?;
    Ok(())
}

fn codex_effective(path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = raw.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    doc.get("model_providers")
        .and_then(|p| p.as_table())
        .map(|t| {
            t.iter().any(|(_, item)| {
                item.as_table()
                    .and_then(|tt| tt.get("base_url"))
                    .and_then(|v| v.as_str())
                    .map(|s| s == PROXY_URL)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

// ---------- Gemini: ~/.gemini/.env ----------

const GEMINI_VAR: &str = "GOOGLE_GEMINI_BASE_URL";

fn gemini_env_path() -> Result<PathBuf> {
    home_path(&[".gemini", ".env"])
}

pub fn apply_gemini_at(path: &Path) -> Result<String> {
    let original = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let target_line = format!("{GEMINI_VAR}={PROXY_URL}");
    let mut found = false;
    let mut lines: Vec<String> = original
        .lines()
        .map(|l| {
            if l.trim_start().starts_with(&format!("{GEMINI_VAR}="))
                || l.trim_start().starts_with(&format!("{GEMINI_VAR} ="))
            {
                found = true;
                target_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        if !lines.is_empty() && !original.ends_with('\n') {
            lines.push(String::new());
        }
        lines.push(target_line);
    }
    let mut out = lines.join("\n");
    if out.ends_with('\n') {
        out.pop();
    }
    write_atomic(path, &out)?;
    Ok(original)
}

pub fn restore_gemini_at(path: &Path, backup: &str) -> Result<()> {
    write_atomic(path, backup)?;
    Ok(())
}

fn gemini_effective(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|raw| {
            raw.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with(&format!("{GEMINI_VAR}=")) && t == format!("{GEMINI_VAR}={PROXY_URL}")
            })
        })
        .unwrap_or(false)
}

// ---------- 对外接口 ----------

pub fn status(pool: &Pool) -> Vec<TakeoverStatus> {
    AppType::all()
        .iter()
        .map(|&app| {
            let supported = matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini);
            let (exists, effective) = if supported {
                match live_path(app) {
                    Ok(p) => (
                        p.exists(),
                        match app {
                            AppType::Claude => claude_effective(&p),
                            AppType::Codex => codex_effective(&p),
                            AppType::Gemini => gemini_effective(&p),
                            _ => false,
                        },
                    ),
                    Err(_) => (false, false),
                }
            } else {
                (false, false)
            };
            let active = settings_get(pool, &active_key(app))
                .ok()
                .flatten()
                .map(|v| v == "1")
                .unwrap_or(false);
            let failover = settings_get(pool, &format!("failover:{}", app.as_str()))
                .ok()
                .flatten()
                .map(|v| v == "1")
                .unwrap_or(false);
            TakeoverStatus {
                app: app.as_str().to_string(),
                supported,
                config_exists: exists,
                active,
                effective,
                failover,
            }
        })
        .collect()
}

fn live_path(app: AppType) -> Result<PathBuf> {
    match app {
        AppType::Claude => claude_settings_path(),
        AppType::Codex => codex_config_path(),
        AppType::Gemini => gemini_env_path(),
        _ => Err(anyhow!("该应用暂不支持接管")),
    }
}

pub fn apply(pool: &Pool, app: AppType) -> Result<()> {
    if !matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini) {
        return Err(anyhow!("{} 暂不支持接管,下个版本支持", app.as_str()));
    }
    let path = live_path(app)?;
    let backup = match app {
        AppType::Claude => apply_claude_at(&path)?,
        AppType::Codex => apply_codex_at(&path)?,
        AppType::Gemini => apply_gemini_at(&path)?,
        _ => unreachable!(),
    };
    settings_set(pool, &backup_key(app), &backup)?;
    settings_set(pool, &active_key(app), "1")?;
    tracing::info!("已接管 {} → {PROXY_URL}({})", app.as_str(), path.display());
    Ok(())
}

pub fn restore(pool: &Pool, app: AppType) -> Result<()> {
    let backup = settings_get(pool, &backup_key(app))?
        .ok_or_else(|| anyhow!("没有 {} 的接管备份(从未接管过?)", app.as_str()))?;
    let path = live_path(app)?;
    match app {
        AppType::Claude => restore_claude_at(&path, &backup)?,
        AppType::Codex => restore_codex_at(&path, &backup)?,
        AppType::Gemini => restore_gemini_at(&path, &backup)?,
        _ => unreachable!(),
    }
    settings_del(pool, &active_key(app))?;
    settings_del(pool, &backup_key(app))?;
    tracing::info!("已还原 {} 的原始配置", app.as_str());
    Ok(())
}

// ---------- 单元测试(纯文件逻辑,不动真实 HOME) ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "conduit_takeover_test_{tag}_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn claude_apply_restore_roundtrip() {
        let p = tmp("claude").join("settings.json");
        std::fs::write(
            &p,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://origin.example"},"other":true}"#,
        )
        .unwrap();

        let backup = apply_claude_at(&p).unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains(PROXY_URL));
        assert!(raw.contains("other")); // 其他配置不动

        restore_claude_at(&p, &backup).unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("https://origin.example"));
        assert!(!raw.contains(PROXY_URL));
    }

    #[test]
    fn claude_apply_creates_missing_file() {
        let p = tmp("claude_new").join("settings.json");
        let backup = apply_claude_at(&p).unwrap();
        assert!(p.exists());
        assert!(claude_effective(&p));
        restore_claude_at(&p, &backup).unwrap();
        assert!(!claude_effective(&p));
    }

    #[test]
    fn codex_apply_keeps_comments_and_restores() {
        let p = tmp("codex").join("config.toml");
        let original = "# my comment\nmodel_provider = \"acme\"\n\n[model_providers.acme]\nname = \"Acme\"\nbase_url = \"https://acme.example\"\n";
        std::fs::write(&p, original).unwrap();

        let backup = apply_codex_at(&p).unwrap();
        assert_eq!(backup, original);
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("# my comment"), "注释必须保留");
        assert!(raw.contains(PROXY_URL));
        assert!(codex_effective(&p));

        restore_codex_at(&p, &backup).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
    }

    #[test]
    fn codex_apply_creates_minimal_provider() {
        let p = tmp("codex_new").join("config.toml");
        apply_codex_at(&p).unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("model_provider = \"keyway\""));
        assert!(raw.contains(PROXY_URL));
    }

    #[test]
    fn gemini_env_roundtrip() {
        let p = tmp("gemini").join(".env");
        std::fs::write(
            &p,
            "GEMINI_API_KEY=xxx\nGOOGLE_GEMINI_BASE_URL=https://g.example\n",
        )
        .unwrap();

        let backup = apply_gemini_at(&p).unwrap();
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains(&format!("{GEMINI_VAR}={PROXY_URL}")));
        assert!(raw.contains("GEMINI_API_KEY=xxx"));
        assert!(gemini_effective(&p));

        restore_gemini_at(&p, &backup).unwrap();
        assert!(std::fs::read_to_string(&p)
            .unwrap()
            .contains("https://g.example"));
    }

    #[test]
    fn gemini_env_appends_when_missing() {
        let p = tmp("gemini_new").join(".env");
        let backup = apply_gemini_at(&p).unwrap();
        assert_eq!(backup, "");
        assert!(gemini_effective(&p));
    }
}
