//! 版本检查:读 GitHub Releases latest(未签名阶段自动更新不可用,先做手动检查)。

use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub latest: String,
    pub url: String,
    pub has_update: bool,
}

/// 查询最新 Release;latest 形如 "v0.2.0",与本地版本(不含 v)逐段比较。
#[tauri::command]
pub async fn check_update(state: tauri::State<'_, AppState>) -> Result<UpdateInfo, String> {
    let resp = state
        .http
        .get("https://api.github.com/repos/anmutu/keyway/releases/latest")
        .header("User-Agent", "keyway-updater")
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| format!("检查更新失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("检查更新失败: HTTP {}", resp.status()));
    }
    let rel: Release = resp.json().await.map_err(|e| format!("解析失败: {e}"))?;
    let current = env!("CARGO_PKG_VERSION");
    let latest_num = rel.tag_name.trim_start_matches('v');
    let has_update = newer(latest_num, current);
    Ok(UpdateInfo {
        latest: rel.tag_name,
        url: rel.html_url,
        has_update,
    })
}

/// 逐段比较语义化版本:a 是否比 b 新
fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<i64> {
        s.split(['.', '-'])
            .map(|x| x.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    for i in 0..va.len().max(vb.len()) {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}
