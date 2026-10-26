//! Tauri 命令层（IPC）。
//!
//! 约定：
//! - 元数据 / 小结果走 JSON（serde）；
//! - 温度矩阵走 base64 字符串（Tauri 1 无二进制 Response，base64 比 JSON 数组更小更快）；
//! - 所有命令返回 Result<T, String>，错误信息面向用户可读。

use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Manager, State};

use crate::batch::{self, BatchSummary};
use crate::db::{self, LedgerInput, LedgerRecord};
use crate::diagnosis::{self, Verdict};
use crate::error::AppResult;
use crate::export::{self, RoiOverlay};
use crate::roi::{self, RoiShape};
use crate::settings::Thresholds;
use crate::thermal::{self, ImageMeta, LoadedImage};

/// 应用全局状态：当前加载的辐射图片会话 + 缺陷阈值 + 台账数据库
pub struct AppState {
    pub image: Mutex<Option<LoadedImage>>,
    pub thresholds: Mutex<Thresholds>,
    /// thresholds.json 落盘路径（setup 时注入）
    pub thresholds_path: Mutex<Option<std::path::PathBuf>>,
    /// SQLite 连接（懒加载初始化）
    pub db: Mutex<Option<Connection>>,
    /// ledger.db 落盘路径（setup 时注入）
    pub db_path: Mutex<Option<std::path::PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            image: Mutex::new(None),
            thresholds: Mutex::new(Thresholds::default()),
            thresholds_path: Mutex::new(None),
            db: Mutex::new(None),
            db_path: Mutex::new(None),
        }
    }
}

impl AppState {
    /// 获取（或初始化）台账数据库连接
    pub fn with_db<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.db.lock().unwrap();
        if guard.is_none() {
            let path = self
                .db_path
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| "数据库路径未初始化".to_string())?;
            *guard = Some(db::open(&path)?);
        }
        f(guard.as_ref().unwrap())
    }
}

/// F1：加载辐射原图（FLIR / 大疆 R-JPEG）
#[tauri::command]
pub fn load_image(
    path: String,
    distance: Option<f64>,
    state: State<'_, AppState>,
) -> AppResult<ImageMeta> {
    let img = thermal::load_rad_image(&path, distance)?;
    let meta = img.meta.clone();
    *state.image.lock().unwrap() = Some(img);
    Ok(meta)
}

/// F2：获取温度矩阵（base64 编码的 f32 小端字节流，长度 w×h×4）
#[tauri::command]
pub fn get_temps(state: State<'_, AppState>) -> AppResult<String> {
    let guard = state.image.lock().unwrap();
    let img = guard.as_ref().ok_or_else(|| "尚未加载图片".to_string())?;
    let mut bytes = Vec::with_capacity(img.temps.len() * 4);
    for t in &img.temps {
        bytes.extend_from_slice(&t.to_le_bytes());
    }
    Ok(to_base64(&bytes))
}

/// 当前图片尺寸（前端分配 Float32Array 用）
#[tauri::command]
pub fn get_dimensions(state: State<'_, AppState>) -> AppResult<(usize, usize)> {
    let guard = state.image.lock().unwrap();
    let img = guard.as_ref().ok_or_else(|| "尚未加载图片".to_string())?;
    Ok((img.width, img.height))
}

/// F7：修改发射率，重算温度矩阵，返回新元数据
#[tauri::command]
pub fn set_emissivity(value: f64, state: State<'_, AppState>) -> AppResult<ImageMeta> {
    let mut guard = state.image.lock().unwrap();
    let img = guard.as_mut().ok_or_else(|| "尚未加载图片".to_string())?;
    img.recompute_with_emissivity(value)?;
    Ok(img.meta.clone())
}

/// 修改测量距离，重算温度矩阵，返回新元数据
#[tauri::command]
pub fn set_distance(value: f64, state: State<'_, AppState>) -> AppResult<ImageMeta> {
    if !(0.1..=10000.0).contains(&value) {
        return Err("距离应在 0.1 ~ 10000 米之间".to_string());
    }
    let mut guard = state.image.lock().unwrap();
    let img = guard.as_mut().ok_or_else(|| "尚未加载图片".to_string())?;
    img.recompute_with_distance(value)?;
    Ok(img.meta.clone())
}

/// F5/F10：ROI 统计 + F12 阈值定级
#[derive(Serialize)]
pub struct RoiStatsReply {
    #[serde(flatten)]
    pub stats: roi::RoiStats,
    pub verdict: Verdict,
}

#[tauri::command]
pub fn roi_stats(shape: RoiShape, state: State<'_, AppState>) -> AppResult<RoiStatsReply> {
    let guard = state.image.lock().unwrap();
    let img = guard.as_ref().ok_or_else(|| "尚未加载图片".to_string())?;
    let stats = roi::roi_stats(&img.temps, img.width, img.height, &shape)?;
    let verdict = {
        let t = state.thresholds.lock().unwrap();
        diagnosis::verdict(&stats, &t)
    };
    Ok(RoiStatsReply { stats, verdict })
}

/// F12：读取缺陷阈值
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Thresholds {
    state.thresholds.lock().unwrap().clone()
}

/// F12：修改缺陷阈值（内存 + 落盘）
#[tauri::command]
pub fn set_settings(
    joint_temp: f64,
    delta_t: f64,
    phase_delta_t: f64,
    state: State<'_, AppState>,
) -> AppResult<Thresholds> {
    if !(0.0..=500.0).contains(&joint_temp) {
        return Err("接头温度阈值应在 0 ~ 500 ℃ 之间".to_string());
    }
    if !(0.0..=200.0).contains(&delta_t) || !(0.0..=200.0).contains(&phase_delta_t) {
        return Err("温差阈值应在 0 ~ 200 ℃ 之间".to_string());
    }
    let t = Thresholds {
        joint_temp,
        delta_t,
        phase_delta_t,
    };
    let path = state.thresholds_path.lock().unwrap().clone();
    if let Some(p) = path {
        crate::settings::save(&p, &t)?;
    }
    *state.thresholds.lock().unwrap() = t.clone();
    Ok(t)
}

/// F11：三相同部位 ΔT 比对（entries: [相别, 最高温]）
#[tauri::command]
pub fn phase_compare(
    entries: Vec<(String, f64)>,
    state: State<'_, AppState>,
) -> AppResult<Option<Verdict>> {
    let t = state.thresholds.lock().unwrap().clone();
    Ok(diagnosis::phase_compare(&entries, &t))
}

/// F14：批量解析文件夹（后台线程 + 进度事件 batch-progress）
#[tauri::command]
pub async fn batch_parse(
    dir: String,
    recursive: bool,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<BatchSummary> {
    let thresholds = state.thresholds.lock().unwrap().clone();
    let dir_path = std::path::PathBuf::from(&dir);
    // 扫描放阻塞线程，避免大文件夹卡住 async 运行时
    let files = tauri::async_runtime::spawn_blocking(move || {
        batch::scan_folder(&dir_path, recursive)
    })
    .await
    .map_err(|e| format!("任务执行失败：{e}"))??;

    let total = files.len();
    if total == 0 {
        return Err("该文件夹下没有找到 JPG 辐射图".to_string());
    }
    let mut items = Vec::with_capacity(total);
    let mut ok = 0usize;
    let mut failed = 0usize;
    for (i, path) in files.iter().enumerate() {
        let item = batch::parse_one(path, &thresholds);
        if item.error.is_some() {
            failed += 1;
        } else {
            ok += 1;
        }
        let done = i + 1;
        let _ = app.emit_all(
            "batch-progress",
            serde_json::json!({ "done": done, "total": total, "file": item.file_name, "error": item.error }),
        );
        items.push(item);
    }
    Ok(BatchSummary {
        total,
        ok,
        failed,
        items,
    })
}

/// F8：导出当前图片温度矩阵 CSV
#[tauri::command]
pub fn export_csv(path: String, state: State<'_, AppState>) -> AppResult<usize> {
    let guard = state.image.lock().unwrap();
    let img = guard.as_ref().ok_or_else(|| "尚未加载图片".to_string())?;
    export::export_csv(&img.temps, img.width, img.height, std::path::Path::new(&path))
}

/// F14：批量解析结果导出 CSV
#[tauri::command]
pub fn export_batch_csv(
    rows: Vec<export::BatchCsvRow>,
    path: String,
    state: State<'_, AppState>,
) -> AppResult<usize> {
    let _ = state;
    export::export_batch_csv(&rows, std::path::Path::new(&path))
}

/// F8：导出当前图片带标注 PNG（伪彩 + ROI 矩形）
#[tauri::command]
pub fn export_annotated(
    path: String,
    rois: Vec<RoiShape>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let guard = state.image.lock().unwrap();
    let img = guard.as_ref().ok_or_else(|| "尚未加载图片".to_string())?;
    // 只取矩形；多边形/线段以包围盒近似
    let overlays: Vec<RoiOverlay> = rois
        .iter()
        .filter_map(|shape| match shape {
            RoiShape::Rect { x, y, w, h } => Some(RoiOverlay {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                color: [0, 166, 80],
            }),
            RoiShape::Poly { points } | RoiShape::Line { points } => {
                if points.is_empty() {
                    return None;
                }
                let min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
                let min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
                let max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
                let max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
                Some(RoiOverlay {
                    x: min_x,
                    y: min_y,
                    w: max_x - min_x,
                    h: max_y - min_y,
                    color: [0, 166, 80],
                })
            }
        })
        .collect();
    export::export_annotated_png(
        &img.temps,
        img.width,
        img.height,
        &overlays,
        std::path::Path::new(&path),
    )
}

/// F15：新增台账记录
#[tauri::command]
pub fn ledger_add(input: LedgerInput, state: State<'_, AppState>) -> AppResult<i64> {
    state.with_db(|conn| db::insert(conn, &input))
}

/// F15：查询台账
#[tauri::command]
pub fn ledger_query(
    keyword: String,
    level: Option<u8>,
    limit: u32,
    offset: u32,
    state: State<'_, AppState>,
) -> AppResult<Vec<LedgerRecord>> {
    state.with_db(|conn| db::query(conn, &keyword, level, limit, offset))
}

/// F15：台账等级统计
#[tauri::command]
pub fn ledger_stats(state: State<'_, AppState>) -> AppResult<[i64; 4]> {
    state.with_db(|conn| db::count_by_level(conn))
}

/// F15：更新台账记录
#[tauri::command]
pub fn ledger_update(id: i64, input: LedgerInput, state: State<'_, AppState>) -> AppResult<usize> {
    state.with_db(|conn| db::update(conn, id, &input))
}

/// F15：删除台账记录
#[tauri::command]
pub fn ledger_delete(id: i64, state: State<'_, AppState>) -> AppResult<usize> {
    state.with_db(|conn| db::delete(conn, id))
}

/// F16：导出 Word 缺陷报告
#[tauri::command]
pub fn export_report(
    ids: Vec<i64>,
    path: String,
    inspector: String,
    date: String,
    org: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let records = state.with_db(|conn| db::get_by_ids(conn, &ids))?;
    crate::report::export_report(&records, std::path::Path::new(&path), &inspector, &date, &org)?;
    Ok(path)
}

/// 标准 base64 编码（无第三方依赖）
fn to_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encoding() {
        assert_eq!(to_base64(b""), "");
        assert_eq!(to_base64(b"f"), "Zg==");
        assert_eq!(to_base64(b"fo"), "Zm8=");
        assert_eq!(to_base64(b"foo"), "Zm9v");
        assert_eq!(to_base64(b"foobar"), "Zm9vYmFy");
    }
}
