//! 配置备份/恢复。
//!
//! 导出:全部供应商(名称/端点/模型)→ app 数据目录下的 JSON 文件。
//! 出于安全考虑 **不导出 API Key**(keychain 条目与本机绑定,跨机无意义)。
//! 导入:同名跳过,异名新建(端点/模型还原),Key 需重新粘贴一次。

use anyhow::{anyhow, Result};

use crate::db::{provider_dao, Pool};
use crate::types::Provider;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupFile {
    pub version: u32,
    pub exported_at: i64,
    pub providers: Vec<BackupProvider>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupProvider {
    pub name: String,
    pub base_url: String,
    pub endpoints: std::collections::HashMap<String, String>,
    pub models: Vec<String>,
}

/// 导出到指定路径,返回导入/导出统计。
pub fn export(pool: &Pool, path: &std::path::Path) -> Result<usize> {
    let list = provider_dao::list_all(pool)?;
    let file = BackupFile {
        version: 1,
        exported_at: chrono::Utc::now().timestamp(),
        providers: list
            .iter()
            .map(|p| BackupProvider {
                name: p.name.clone(),
                base_url: p.base_url.clone(),
                endpoints: p.endpoints.clone(),
                models: p.models.clone(),
            })
            .collect(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    std::fs::write(path, json)?;
    Ok(list.len())
}

/// 从指定路径导入:同名跳过,异名新建。返回 (新建数, 跳过数)。
pub fn import(pool: &Pool, path: &std::path::Path) -> Result<(usize, usize)> {
    let raw = std::fs::read_to_string(path).map_err(|e| anyhow!("读取备份失败: {e}"))?;
    let file: BackupFile =
        serde_json::from_str(&raw).map_err(|e| anyhow!("备份文件格式错误: {e}"))?;
    let mut created = 0;
    let mut skipped = 0;
    for bp in file.providers {
        if provider_dao::find_by_name(pool, &bp.name)?.is_some() {
            skipped += 1;
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let p = Provider {
            id,
            app_type: crate::types::AppType::Claude, // 逻辑实体跨分组,首分组字段仅为兼容
            name: bp.name,
            base_url: bp.base_url,
            endpoints: bp.endpoints,
            keychain_id: None,
            models: bp.models,
            is_current: false,
            is_healthy: true,
            sort_index: 0,
            created_at: chrono::Utc::now().timestamp(),
            has_key: false,
            meta_has_key: Some(false),
        };
        provider_dao::insert(pool, &p)?;
        created += 1;
    }
    Ok((created, skipped))
}

/// 默认备份路径:app 数据目录/conduit-backup-YYYYMMDD-HHMMSS.json
pub fn default_backup_path(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    app_data_dir.join(format!("conduit-backup-{ts}.json"))
}
