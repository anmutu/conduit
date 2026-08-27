//! Skills 统一管理:本地 vault(~/.keyway/skills)作为唯一源,
//! 同步复制到各 CLI 的 skills 目录(Claude ~/.claude/skills、Codex ~/.codex/skills)。
//!
//! 格式事实标准:每个 skill 一个目录,内含 SKILL.md(YAML frontmatter:name/description)。
//! vault 内额外放 .keyway.json 记录同步目标 {apps:[...]}。
//! 同步只增删 vault 中存在的 id(按目录名精确匹配),CLI 自带/用户手工的 skill 不动。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, serde::Serialize)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 同步目标(cli 标识,如 claude/codex)
    pub apps: Vec<String>,
    /// 目录内除 SKILL.md/.keyway.json 外是否还有附属文件
    pub has_files: bool,
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME 未设置")
}

pub fn vault_dir() -> Result<PathBuf> {
    Ok(home()?.join(".keyway").join("skills"))
}

/// 各 CLI 的 skills 目录
pub fn cli_skills_dir(app: &str) -> Result<PathBuf> {
    match app {
        "claude" => Ok(home()?.join(".claude").join("skills")),
        "codex" => Ok(home()?.join(".codex").join("skills")),
        _ => Err(anyhow!("未知 CLI: {app}")),
    }
}

pub const TARGET_APPS: &[&str] = &["claude", "codex"];

// ---------- frontmatter 解析(轻量:只取 name/description 首个字段) ----------

fn frontmatter_field(content: &str, field: &str) -> Option<String> {
    let mut in_fm = false;
    for line in content.lines() {
        let l = line.trim();
        if l == "---" {
            if in_fm {
                break;
            }
            in_fm = true;
            continue;
        }
        if !in_fm {
            continue;
        }
        if let Some(rest) = l.strip_prefix(&format!("{field}:")) {
            let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

// ---------- 目录复制 ----------

fn copy_dir_rec(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_rec(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

// ---------- vault 操作(带 base 参数便于测试) ----------

fn read_meta(skill_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(skill_dir.join(".keyway.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("apps").and_then(|a| a.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn write_meta(skill_dir: &Path, apps: &[String]) -> Result<()> {
    std::fs::write(
        skill_dir.join(".keyway.json"),
        serde_json::json!({ "apps": apps }).to_string(),
    )?;
    Ok(())
}

/// 列出 vault 内全部 skills
pub fn list() -> Result<Vec<SkillEntry>> {
    list_at(&vault_dir()?)
}

fn list_at(vault: &Path) -> Result<Vec<SkillEntry>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(vault) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let dir = entry.path();
        let content = std::fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
        let apps = read_meta(&dir);
        let has_files = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok()).any(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n != "SKILL.md" && n != ".keyway.json"
                })
            })
            .unwrap_or(false);
        out.push(SkillEntry {
            name: frontmatter_field(&content, "name").unwrap_or_else(|| id.clone()),
            description: frontmatter_field(&content, "description").unwrap_or_default(),
            id,
            apps,
            has_files,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// 新建/更新 vault 内 skill(content 为 SKILL.md 全文)
pub fn save(id: &str, content: &str, apps: &[String]) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("ID 仅支持字母、数字、-、_"));
    }
    let dir = vault_dir()?.join(id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("SKILL.md"), content)?;
    write_meta(&dir, apps)?;
    Ok(())
}

/// 从某 CLI 的 skills 目录导入一个 skill 到 vault(含附属文件)
pub fn import_from(app: &str, id: &str) -> Result<()> {
    let src = cli_skills_dir(app)?.join(id);
    if !src.is_dir() {
        return Err(anyhow!("源 skill 不存在: {app}/{id}"));
    }
    let vault = vault_dir()?;
    let dst = vault.join(id);
    if dst.exists() {
        std::fs::remove_dir_all(&dst)?;
    }
    std::fs::create_dir_all(&vault)?;
    copy_dir_rec(&src, &dst)?;
    // 记录来源 app 为默认目标
    let mut apps = read_meta(&dst);
    if !apps.iter().any(|a| a == app) {
        apps.push(app.to_string());
    }
    write_meta(&dst, &apps)?;
    Ok(())
}

/// 列出某 CLI skills 目录下的全部 skill id(供导入选择)
pub fn scan_cli(app: &str) -> Result<Vec<String>> {
    let dir = cli_skills_dir(app)?;
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.filter_map(|e| e.ok()) {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.path().join("SKILL.md").exists()
            {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 删除:vault + 所有目标目录中的副本
pub fn delete(id: &str) -> Result<()> {
    let dir = vault_dir()?.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    for app in TARGET_APPS {
        let t = cli_skills_dir(app)?.join(id);
        if t.exists() {
            std::fs::remove_dir_all(&t)?;
        }
    }
    Ok(())
}

/// 同步:vault 中启用的 skill 复制到目标;未启用的目标副本删除(仅限 vault 内 id)。
pub fn sync_all() -> Result<Vec<String>> {
    let vault = vault_dir()?;
    let mut targets: Vec<(&str, PathBuf)> = Vec::new();
    for app in TARGET_APPS {
        targets.push((*app, cli_skills_dir(app)?));
    }
    sync_at(&vault, &targets)
}

fn sync_at(vault: &Path, targets: &[(&str, PathBuf)]) -> Result<Vec<String>> {
    let mut report = Vec::new();
    let entries = list_at(vault)?;
    let vault_ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();

    for (app, target) in targets {
        let _ = std::fs::create_dir_all(target);
        // 已知副本里,不在 vault 的不动;在 vault 但未启用的删除
        let mut n = 0;
        for e in &entries {
            let dst = target.join(&e.id);
            let wanted = e.apps.iter().any(|a| a == app);
            if wanted {
                if dst.exists() {
                    std::fs::remove_dir_all(&dst)?;
                }
                copy_dir_rec(&vault.join(&e.id), &dst)?;
                n += 1;
            } else if dst.exists() && vault_ids.contains(&e.id.as_str()) {
                std::fs::remove_dir_all(&dst)?;
            }
        }
        report.push(format!("{app}: {n} 个 skill 已同步"));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("skills_t_{tag}_{}", uuid::Uuid::new_v4()));
        let vault = base.join("vault");
        let claude = base.join("claude").join("skills");
        let codex = base.join("codex").join("skills");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&codex).unwrap();
        (vault, claude, codex)
    }

    // 直接测 _at 版逻辑:临时目录模拟 vault 与目标
    #[test]
    fn sync_copies_and_prunes() {
        let (vault, claude, codex) = setup("sync");
        // vault 两个 skill:demo 双端;only_c 仅 claude
        let mk = |id: &str, apps: &[&str]| {
            let d = vault.join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join("SKILL.md"),
                format!("---\nname: \"{id}\"\ndescription: \"d\"\n---\nbody"),
            )
            .unwrap();
            write_meta(&d, &apps.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        };
        mk("demo", &["claude", "codex"]);
        mk("only_c", &["claude"]);
        // 用户手工 skill(不在 vault)
        std::fs::create_dir_all(claude.join("user_own")).unwrap();
        std::fs::write(claude.join("user_own").join("SKILL.md"), "x").unwrap();
        // 旧副本(vault 中存在但已改为不启用 codex)
        std::fs::create_dir_all(codex.join("only_c")).unwrap();
        std::fs::write(codex.join("only_c").join("SKILL.md"), "old").unwrap();

        let report = sync_at(
            &vault,
            &[("claude", claude.clone()), ("codex", codex.clone())],
        )
        .unwrap();
        assert!(report[0].contains("2"), "{report:?}");
        assert!(report[1].contains("1"), "{report:?}");
        assert!(claude.join("demo/SKILL.md").exists());
        assert!(codex.join("demo/SKILL.md").exists());
        assert!(claude.join("only_c/SKILL.md").exists());
        assert!(!codex.join("only_c").exists(), "未启用的目标副本应删除");
        assert!(
            claude.join("user_own/SKILL.md").exists(),
            "用户手工 skill 不动"
        );

        // 列表解析 frontmatter
        let list = list_at(&vault).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "demo");
        let _ = std::fs::remove_dir_all(vault.parent().unwrap());
    }

    #[test]
    fn frontmatter_parses() {
        let c = "---\nname: \"hello world\"\ndescription: 'a skill'\n---\nbody";
        assert_eq!(frontmatter_field(c, "name").unwrap(), "hello world");
        assert_eq!(frontmatter_field(c, "description").unwrap(), "a skill");
        assert!(frontmatter_field("no fm", "name").is_none());
    }
}

/// 读取 vault 内 skill 的 SKILL.md 全文(编辑用)
pub fn read_content(id: &str) -> Result<String> {
    std::fs::read_to_string(vault_dir()?.join(id).join("SKILL.md"))
        .context("skill 不存在或无 SKILL.md")
}

/// 仅改同步目标(不动内容)
pub fn set_apps(id: &str, apps: &[String]) -> Result<()> {
    write_meta(&vault_dir()?.join(id), apps)
}

// ---------- 备份(v3)支持 ----------

use serde::Serialize;

#[derive(Serialize)]
pub struct BackupFileEntry {
    pub path: String,
    pub content: String,
}

fn walk_files(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            let name = e.file_name().to_string_lossy().to_string();
            walk_files(&e.path(), &format!("{prefix}{name}/"), out)?;
        } else {
            let name = e.file_name().to_string_lossy().to_string();
            out.push((format!("{prefix}{name}"), std::fs::read(e.path())?));
        }
    }
    Ok(())
}

/// 导出 vault 全部内容(相对路径 → base64)
pub fn export_vault() -> Result<Vec<crate::services::backup::BackupSkill>> {
    let vault = vault_dir()?;
    let entries = list_at(&vault)?;
    let mut out = Vec::new();
    for e in entries {
        let mut files = Vec::new();
        walk_files(&vault.join(&e.id), "", &mut files)?;
        out.push(crate::services::backup::BackupSkill {
            id: e.id,
            files: files
                .into_iter()
                .map(|(path, content)| crate::services::backup::BackupSkillFile {
                    path,
                    content: {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD.encode(content)
                    },
                })
                .collect(),
        });
    }
    Ok(out)
}

/// 从备份恢复一个 skill 到 vault(同 id 已存在则跳过,返回是否写入)
pub fn import_backup_skill(
    id: &str,
    files: &[crate::services::backup::BackupSkillFile],
) -> Result<bool> {
    let dir = vault_dir()?.join(id);
    if dir.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&dir)?;
    for f in files {
        use base64::Engine;
        let content = base64::engine::general_purpose::STANDARD
            .decode(&f.content)
            .with_context(|| format!("备份文件损坏: {}", f.path))?;
        let target = dir.join(&f.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, content)?;
    }
    Ok(true)
}
