//! MCP 服务器同步:把统一管理的 MCP 定义写入各 CLI 的配置文件。
//!
//! - Claude:`~/.claude.json` 顶层 `mcpServers`(支持 stdio 与 http/sse)
//! - Codex:`~/.codex/config.toml` 的 `[mcp_servers.<id>]`(仅 stdio;toml_edit 保注释保格式)
//!
//! 同步策略:键名 = 服务器 id。仅增改/删除**我们管理的键**,其余用户手工配置的
//! MCP 条目原样保留(除非与 id 重名,视为 ours 覆盖)。
//! 停用的服务器从目标配置中移除(移除时按 id 精确匹配)。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::db::{mcp_dao, mcp_dao::McpServer, Pool};

fn home_path(rel: &[&str]) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME 未设置")?;
    let mut p = PathBuf::from(home);
    for seg in rel {
        p.push(seg);
    }
    Ok(p)
}

pub fn claude_config_path() -> Result<PathBuf> {
    home_path(&[".claude.json"])
}

pub fn codex_config_path() -> Result<PathBuf> {
    home_path(&[".codex", "config.toml"])
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("mcp.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// MCP config JSON → Claude mcpServers 条目
fn claude_entry(config: &serde_json::Value) -> Option<serde_json::Value> {
    let obj = config.as_object()?;
    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        let mut e = serde_json::Map::new();
        let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("http");
        e.insert("type".into(), serde_json::json!(typ));
        e.insert("url".into(), serde_json::json!(url));
        return Some(serde_json::Value::Object(e));
    }
    let mut e = serde_json::Map::new();
    e.insert(
        "type".into(),
        serde_json::json!(obj.get("type").and_then(|v| v.as_str()).unwrap_or("stdio")),
    );
    e.insert(
        "command".into(),
        serde_json::json!(obj.get("command").and_then(|v| v.as_str()).unwrap_or("")),
    );
    if let Some(args) = obj.get("args").and_then(|v| v.as_array()) {
        e.insert("args".into(), serde_json::Value::Array(args.clone()));
    }
    if let Some(env) = obj.get("env").and_then(|v| v.as_object()) {
        e.insert("env".into(), serde_json::Value::Object(env.clone()));
    }
    Some(serde_json::Value::Object(e))
}

/// MCP config JSON → Codex [mcp_servers.<id>] TOML 表;远程型返回 None(Codex 不支持)
fn codex_entry(doc: &mut toml_edit::DocumentMut, id: &str, config: &serde_json::Value) -> bool {
    let Some(obj) = config.as_object() else {
        return false;
    };
    if !obj.contains_key("command") {
        return false;
    }
    let mut t = toml_edit::Table::new();
    if let Some(c) = obj.get("command").and_then(|v| v.as_str()) {
        t["command"] = toml_edit::value(c);
    }
    if let Some(args) = obj.get("args").and_then(|v| v.as_array()) {
        let mut a = toml_edit::Array::new();
        for arg in args.iter().filter_map(|v| v.as_str()) {
            a.push(arg);
        }
        t["args"] = toml_edit::value(a);
    }
    if let Some(env) = obj.get("env").and_then(|v| v.as_object()) {
        let mut et = toml_edit::Table::new();
        for (k, v) in env {
            if let Some(s) = v.as_str() {
                et[k] = toml_edit::value(s);
            }
        }
        t["env"] = toml_edit::Item::Table(et);
    }
    doc["mcp_servers"][id] = toml_edit::Item::Table(t);
    true
}

/// 同步全部服务器到所有目标 CLI。返回各目标的写入结果描述。
///
/// 只增删**我们管理**的键(上次同步写过的 id 记录在 settings KV),
/// 用户手工配置的 MCP 条目原样保留。
pub fn sync_all(pool: &Pool) -> Result<Vec<String>> {
    let servers = mcp_dao::list(pool)?;
    let mut report = Vec::new();

    for (app, sync_at, path) in [
        (
            "claude",
            sync_claude_at as fn(&Path, &[McpServer], &[String]) -> Result<usize>,
            claude_config_path()?,
        ),
        ("codex", sync_codex_at, codex_config_path()?),
    ] {
        let kv_key = format!("mcp.managed.{app}");
        let prev: Vec<String> = crate::db::kv::get(pool, &kv_key)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // 本次要写的 id;移除 = 上次写过 - 本次想写(被删除/停用/不再面向该 app)
        let wanted: Vec<String> = servers
            .iter()
            .filter(|s| wants(s, app))
            .map(|s| s.id.clone())
            .collect();
        let remove: Vec<String> = prev
            .iter()
            .filter(|id| !wanted.contains(id))
            .cloned()
            .collect();

        if !path.exists() {
            report.push(format!("{app}: 未找到 {},跳过", path.display()));
            continue;
        }
        match sync_at(&path, &servers, &remove) {
            Ok(n) => report.push(format!("{app}: {n} 个服务器已同步")),
            Err(e) => report.push(format!("{app}: 同步失败({e})")),
        }
        crate::db::kv::set(pool, &kv_key, &serde_json::to_string(&wanted)?)
            .map_err(|e| anyhow!("{e}"))?;
    }
    Ok(report)
}

fn wants(s: &McpServer, app: &str) -> bool {
    s.enabled && s.apps.iter().any(|a| a == app)
}

/// 写 Claude ~/.claude.json:mcpServers 键 = 服务器 id;remove 为不再管理的 id。
fn sync_claude_at(path: &Path, servers: &[McpServer], remove: &[String]) -> Result<usize> {
    let original = std::fs::read_to_string(path).context("读取 ~/.claude.json 失败")?;
    let mut root: serde_json::Value = if original.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&original).map_err(|e| anyhow!("~/.claude.json 解析失败: {e}"))?
    };
    if !root.is_object() {
        return Err(anyhow!("~/.claude.json 顶层不是对象,拒绝改写"));
    }
    let obj = root.as_object_mut().unwrap();
    let mcp = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    if !mcp.is_object() {
        *mcp = serde_json::json!({});
    }
    let mcp_obj = mcp.as_object_mut().unwrap();

    let mut n = 0;
    for s in servers.iter().filter(|s| wants(s, "claude")) {
        if let Some(entry) = claude_entry(&s.config) {
            mcp_obj.insert(s.id.clone(), entry);
            n += 1;
        }
    }
    for id in remove {
        mcp_obj.remove(id);
    }
    write_atomic(path, &serde_json::to_string_pretty(&root)?)?;
    Ok(n)
}

/// 写 Codex ~/.codex/config.toml 的 [mcp_servers.*];toml_edit 保注释保格式。
fn sync_codex_at(path: &Path, servers: &[McpServer], remove: &[String]) -> Result<usize> {
    let original = std::fs::read_to_string(path).context("读取 config.toml 失败")?;
    let mut doc: toml_edit::DocumentMut = original
        .parse()
        .map_err(|e| anyhow!("config.toml 解析失败: {e}"))?;

    let mut n = 0;
    for s in servers.iter().filter(|s| wants(s, "codex")) {
        if codex_entry(&mut doc, &s.id, &s.config) {
            n += 1;
        }
    }
    // 不再管理的 id:仅移除我们写过的键
    if let Some(mcp) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) {
        for id in remove {
            mcp.remove(id);
        }
    }
    write_atomic(path, &doc.to_string())?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srv(id: &str, apps: &[&str], enabled: bool, config: serde_json::Value) -> McpServer {
        McpServer {
            id: id.into(),
            name: id.into(),
            config,
            apps: apps.iter().map(|s| s.to_string()).collect(),
            enabled,
            created_at: 0,
        }
    }

    #[test]
    fn claude_sync_merges_and_prunes() {
        let dir = std::env::temp_dir().join(format!("mcp_t_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("claude.json");
        std::fs::write(
            &p,
            r#"{"firstStartTime":"2026-01-01","mcpServers":{"old":{"command":"x"},"keep":{"command":"y"}}}"#,
        )
        .unwrap();

        let servers = vec![
            srv(
                "fs",
                &["claude"],
                true,
                serde_json::json!({"command":"npx","args":["-y","fs-mcp"],"env":{"A":"1"}}),
            ),
            srv(
                "remote",
                &["claude"],
                true,
                serde_json::json!({"type":"http","url":"https://m.example/sse"}),
            ),
        ];
        // 上次同步写过 old(在 remove 列表);keep 是用户手工条目,不在管理范围
        let n = sync_claude_at(&p, &servers, &["old".into()]).unwrap();
        assert_eq!(n, 2);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["firstStartTime"], "2026-01-01", "无关键保留");
        assert_eq!(v["mcpServers"]["fs"]["command"], "npx");
        assert_eq!(v["mcpServers"]["fs"]["args"][1], "fs-mcp");
        assert_eq!(v["mcpServers"]["fs"]["env"]["A"], "1");
        assert_eq!(v["mcpServers"]["remote"]["type"], "http");
        assert_eq!(v["mcpServers"]["remote"]["url"], "https://m.example/sse");
        assert!(v["mcpServers"].get("keep").is_some(), "用户手工条目保留");
        assert!(v["mcpServers"].get("old").is_none(), "不在库中的 id 被清");

        // 停用后(sync_all 会把 fs 放进 remove):条目消失,手工条目仍在
        let disabled = vec![srv(
            "fs",
            &["claude"],
            false,
            serde_json::json!({"command":"npx"}),
        )];
        sync_claude_at(&p, &disabled, &["fs".into(), "remote".into()]).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert!(v["mcpServers"].get("fs").is_none());
        assert!(v["mcpServers"].get("keep").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_sync_toml_preserves_comments() {
        let dir = std::env::temp_dir().join(format!("mcp_t_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(
            &p,
            "# my config\nmodel = \"gpt-5\"\n\n[mcp_servers.user]\ncommand = \"keep\"\n",
        )
        .unwrap();

        let servers = vec![srv(
            "fs",
            &["codex"],
            true,
            serde_json::json!({"command":"npx","args":["-y","fs-mcp"],"env":{"A":"1"}}),
        )];
        let n = sync_codex_at(&p, &servers, &[]).unwrap();
        assert_eq!(n, 1);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.starts_with("# my config"), "注释保留: {s}");
        assert!(s.contains("model = \"gpt-5\""));
        assert!(s.contains("[mcp_servers.fs]"), "{s}");
        assert!(s.contains("command = \"npx\""), "{s}");
        assert!(s.contains("\"-y\""), "{s}");
        assert!(s.contains("[mcp_servers.user]"), "手工条目保留: {s}");

        // 远程型:不写入 codex
        let remote = vec![srv(
            "r",
            &["codex"],
            true,
            serde_json::json!({"type":"http","url":"https://x"}),
        )];
        assert_eq!(sync_codex_at(&p, &remote, &[]).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
