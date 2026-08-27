//! 供应商组合 Profile:一键保存/恢复各分组的当前供应商(current_map 快照)。
//!
//! 典型场景:「工作」组合 vs「个人」组合,切换不用逐分组点。

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::db::{kv, provider_dao, Pool};

/// profile 名 → (app_type → provider_id)
type Profiles = HashMap<String, HashMap<String, String>>;

fn load_profiles(pool: &Pool) -> Result<Profiles> {
    Ok(kv::get(pool, "profiles")?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

fn save_profiles(pool: &Pool, p: &Profiles) -> Result<()> {
    let json = serde_json::to_string(p)?;
    kv::set(pool, "profiles", &json)
}

/// 保存当前 current_map 为命名 profile(同名覆盖)。
pub fn save(pool: &Pool, name: &str) -> Result<Vec<String>> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("Profile 名不能为空"));
    }
    let mut profiles = load_profiles(pool)?;
    let mut apps = Vec::new();
    for app in crate::types::AppType::all() {
        if let Some(p) = provider_dao::get_current(pool, *app)? {
            profiles
                .entry(name.to_string())
                .or_default()
                .insert(app.as_str().to_string(), p.id);
            apps.push(app.as_str().to_string());
        }
    }
    if apps.is_empty() {
        return Err(anyhow!("当前没有任何分组的当前供应商,无可保存"));
    }
    save_profiles(pool, &profiles)?;
    Ok(apps)
}

/// 应用 profile:恢复各分组的当前供应商(缺失的分组跳过)。
pub fn apply(pool: &Pool, name: &str) -> Result<usize> {
    let profiles = load_profiles(pool)?;
    let map = profiles
        .get(name)
        .ok_or_else(|| anyhow!("Profile 不存在: {name}"))?;
    let mut applied = 0;
    for (app, pid) in map {
        let Some(app) = crate::types::AppType::from_str(app) else {
            continue;
        };
        if provider_dao::get_by_id(pool, pid)?.is_some() {
            provider_dao::set_current(pool, pid, app)?;
            applied += 1;
        }
    }
    Ok(applied)
}

pub fn delete(pool: &Pool, name: &str) -> Result<()> {
    let mut profiles = load_profiles(pool)?;
    profiles
        .remove(name)
        .ok_or_else(|| anyhow!("Profile 不存在: {name}"))?;
    save_profiles(pool, &profiles)
}

pub fn list(pool: &Pool) -> Result<Vec<String>> {
    let profiles = load_profiles(pool)?;
    Ok(profiles.keys().cloned().collect())
}
