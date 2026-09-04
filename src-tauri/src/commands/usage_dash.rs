//! 用量仪表盘命令。

use tauri::{Manager, State};

use crate::db::usage_dao::UsageDashboard;
use crate::state::AppState;
use crate::types::AppType;

#[tauri::command]
pub fn get_usage_dashboard(
    state: State<'_, AppState>,
    app_type: AppType,
    days: Option<i64>,
) -> Result<UsageDashboard, String> {
    let days = days.unwrap_or(7).clamp(1, 90);
    crate::db::usage_dao::dashboard(&state.db, app_type.as_str(), days).map_err(|e| e.to_string())
}

/// 最近请求日志(请求浏览器)
#[tauri::command]
pub fn get_recent_usage(
    state: State<'_, AppState>,
    app_type: AppType,
    limit: Option<i64>,
) -> Result<Vec<crate::db::usage_dao::UsageEntry>, String> {
    crate::db::usage_dao::recent(&state.db, app_type.as_str(), limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

/// 清空该分组的请求日志
#[tauri::command]
pub fn clear_usage(state: State<'_, AppState>, app_type: AppType) -> Result<usize, String> {
    crate::db::usage_dao::clear(&state.db, app_type.as_str()).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct CsvResult {
    pub path: String,
    pub count: usize,
}

/// 导出该分组最近 5000 条请求为 CSV(写入 app 数据目录,返回路径)。
#[tauri::command]
pub fn export_usage_csv(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_type: AppType,
    target: Option<String>,
) -> Result<CsvResult, String> {
    let entries = crate::db::usage_dao::recent(&state.db, app_type.as_str(), 5000)
        .map_err(|e| e.to_string())?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位数据目录: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = match target {
        // 前端保存对话框选定的路径
        Some(t) if !t.trim().is_empty() => std::path::PathBuf::from(t),
        _ => dir.join(format!("keyway-usage-{ts}.csv")),
    };

    let esc = |s: &str| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut csv = String::from(
        "time,provider,model,input_tokens,output_tokens,status,duration_ms,rule,error\n",
    );
    for e in &entries {
        let time = chrono::DateTime::from_timestamp(e.created_at, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            time,
            esc(&e.provider_id),
            esc(e.model.as_deref().unwrap_or("")),
            e.input_tokens,
            e.output_tokens,
            e.status,
            e.duration_ms,
            esc(e.rule_pattern.as_deref().unwrap_or("")),
            esc(e.error_note.as_deref().unwrap_or("")),
        ));
    }
    // BOM 头:让 Excel 正确识别 UTF-8
    std::fs::write(&path, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    Ok(CsvResult {
        path: path.to_string_lossy().into_owned(),
        count: entries.len(),
    })
}

/// 供应商×模型 用量明细导出(days 天范围)。
#[tauri::command]
pub fn export_usage_models_csv(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_type: AppType,
    days: Option<i64>,
    target: Option<String>,
) -> Result<CsvResult, String> {
    let days = days.unwrap_or(7).clamp(1, 365);
    let rows = crate::db::usage_dao::group_provider_model(&state.db, app_type.as_str(), days)
        .map_err(|e| e.to_string())?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位数据目录: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = match target {
        Some(t) if !t.trim().is_empty() => std::path::PathBuf::from(t),
        _ => dir.join(format!("keyway-models-{ts}.csv")),
    };
    let esc = |s: &str| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut csv = String::from("provider,model,requests,errors,input_tokens,output_tokens\n");
    for r in &rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            esc(&r.provider_id),
            esc(&r.model),
            r.requests,
            r.errors,
            r.input_tokens,
            r.output_tokens,
        ));
    }
    std::fs::write(&path, format!("\u{feff}{csv}")).map_err(|e| e.to_string())?;
    Ok(CsvResult {
        path: path.to_string_lossy().into_owned(),
        count: rows.len(),
    })
}

// ---------- 模型单价表(成本估算,批 C):KV `price:{model}` = "input,output"(美元/百万 tokens) ----------

fn price_key(model: &str) -> String {
    format!("price:{}", model.trim())
}

/// 全部已设单价:(model, input $/M, output $/M),按模型名排序
#[tauri::command]
pub fn get_model_prices(
    state: State<'_, AppState>,
) -> Result<Vec<(String, f64, f64)>, String> {
    let mut out: Vec<(String, f64, f64)> = crate::db::kv::all(&state.db)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(k, v)| {
            let model = k.strip_prefix("price:")?.to_string();
            let (i, o) = v.split_once(',')?;
            let input = i.trim().parse::<f64>().ok()?;
            let output = o.trim().parse::<f64>().ok()?;
            Some((model, input, output))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// 设置/更新某模型单价;输入与输出同时为 0 时删除该条
#[tauri::command]
pub fn set_model_price(
    state: State<'_, AppState>,
    model: String,
    input: f64,
    output: f64,
) -> Result<(), String> {
    let model = model.trim().to_string();
    if model.is_empty() || model.len() > 128 {
        return Err("模型名不能为空且不得超过 128 字符".into());
    }
    if !(0.0..=1e9).contains(&input) || !(0.0..=1e9).contains(&output) || input.is_nan() || output.is_nan() {
        return Err("单价必须是不小于 0 的数字".into());
    }
    let key = price_key(&model);
    if input == 0.0 && output == 0.0 {
        crate::db::kv::del(&state.db, &key).map_err(|e| e.to_string())?;
    } else {
        crate::db::kv::set(&state.db, &key, &format!("{input},{output}"))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 删除某模型单价
#[tauri::command]
pub fn remove_model_price(
    state: State<'_, AppState>,
    model: String,
) -> Result<(), String> {
    crate::db::kv::del(&state.db, &price_key(model.trim()))
        .map_err(|e| e.to_string())
}
