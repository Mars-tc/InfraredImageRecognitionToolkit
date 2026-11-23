//! 批量文件夹遍历（F14）：递归扫描辐射图 + 逐张解析 + 进度上报。
//!
//! 文档风险 #5：解析放 spawn_blocking 线程执行，避免卡死 Webview。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::diagnosis;
use crate::error::AppResult;
use crate::settings::Thresholds;
use crate::thermal::{load_rad_image_from_bytes, position_from_meta, read_position_from_bytes};

/// 批量结果条目
#[derive(Debug, Serialize, Clone)]
pub struct BatchItem {
    pub path: String,
    pub file_name: String,
    pub max: f64,
    pub min: f64,
    pub delta_t: f64,
    /// 缺陷等级 0~3
    pub level: u8,
    pub error: Option<String>,
    /// 拍摄位置（离线地图打点用；无 GPS 或坐标无效时为 null）
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub datetime: Option<String>,
}

/// 批量汇总
#[derive(Debug, Serialize)]
pub struct BatchSummary {
    /// 扫描到的候选 JPG 总数（含被忽略项）
    pub total: usize,
    /// 成功解析的红外原图数量
    pub ok: usize,
    /// 因无法解析出辐射数据而被忽略的非红外照片数量
    pub skipped: usize,
    pub items: Vec<BatchItem>,
}

/// 扫描文件夹下的辐射图（jpg/jpeg），recursive 时递归子目录
pub fn scan_folder(dir: &Path, recursive: bool) -> AppResult<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Err(format!("不是有效文件夹：{}", dir.display()));
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|e| format!("读取目录失败：{e}"))?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if recursive {
                    stack.push(p);
                }
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "jpg" || ext == "jpeg" {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 进度事件步长：约 1% 一次（最多约 200 次），小目录至少每张一次
pub fn progress_stride(total: usize) -> usize {
    (total / 200).max(1)
}

/// 解析单张并按阈值定级（失败时返回带 error 的条目）
///
/// 文件只读取一次：同一份字节用于热数据解析与定位信息兜底，避免批量场景重复磁盘 IO。
pub fn parse_one(path: &Path, thresholds: &Thresholds) -> BatchItem {
    let mut item = BatchItem {
        path: path.to_string_lossy().to_string(),
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        max: 0.0,
        min: 0.0,
        delta_t: 0.0,
        level: 0,
        error: None,
        lat: None,
        lon: None,
        datetime: None,
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            item.error = Some(format!("读取文件失败：{e}"));
            return item;
        }
    };
    match load_rad_image_from_bytes(&item.path, &bytes, None) {
        Ok(img) => {
            // 定位信息优先复用已解析的 meta（不再重复扫描 EXIF / XMP）；
            // meta 坐标无效时回退字节解析，保留「EXIF 占位坐标 + XMP 有效坐标」场景
            let pos = position_from_meta(&img.meta)
                .or_else(|| read_position_from_bytes(&bytes));
            if let Some(pos) = pos {
                item.lat = Some(pos.lat);
                item.lon = Some(pos.lon);
                item.datetime = pos.datetime;
            }
            // 极值复用 load 阶段已算好的范围，避免再遍历一遍温度矩阵
            let max = img.meta.temp_max;
            let min = img.meta.temp_min;
            if !max.is_finite() {
                item.error = Some("图像内无有效温度数据".to_string());
                return item;
            }
            let stats = crate::roi::RoiStats {
                max,
                min,
                avg: (max + min) / 2.0,
                delta_t: max - min,
                count: img.temps.len(),
            };
            let verdict = diagnosis::verdict(&stats, thresholds);
            item.max = max;
            item.min = min;
            item.delta_t = max - min;
            item.level = verdict.level;
            item
        }
        Err(e) => {
            // 定位信息独立于温度解析：普通预览图 / 损坏文件解析失败时仍可打点
            if let Some(pos) = read_position_from_bytes(&bytes) {
                item.lat = Some(pos.lat);
                item.lon = Some(pos.lon);
                item.datetime = pos.datetime;
            }
            item.error = Some(e);
            item
        }
    }
}

/// 顺序解析候选 JPG，跳过无法解析出辐射数据的非红外照片。
/// `on_progress(done, &item)` 在每个候选处理完后调用（被忽略项也会回调，便于进度条走满）。
/// 返回（成功条目, 被忽略数量）。
pub fn parse_radiometric<F: FnMut(usize, &BatchItem)>(
    files: &[PathBuf],
    thresholds: &Thresholds,
    mut on_progress: F,
) -> (Vec<BatchItem>, usize) {
    let mut items = Vec::new();
    let mut skipped = 0usize;
    for (i, path) in files.iter().enumerate() {
        let item = parse_one(path, thresholds);
        on_progress(i + 1, &item);
        if item.error.is_none() {
            items.push(item);
        } else {
            skipped += 1;
        }
    }
    (items, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每轮独占的临时目录：cargo test 默认并行，并发跑两次或上次残留
    /// 都不能让固定目录名污染断言。
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ir_test_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建测试目录");
        dir
    }

    #[test]
    fn scan_finds_jpg_only() {
        let dir = scratch("batch_scan");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.jpg"), b"x").unwrap();
        std::fs::write(dir.join("b.JPG"), b"x").unwrap();
        std::fs::write(dir.join("c.png"), b"x").unwrap();
        std::fs::write(sub.join("d.jpeg"), b"x").unwrap();
        let flat = scan_folder(&dir, false).unwrap();
        assert_eq!(flat.len(), 2);
        let deep = scan_folder(&dir, true).unwrap();
        assert_eq!(deep.len(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_one_reports_error_for_non_rad() {
        let dir = scratch("batch_parse");
        let p = dir.join("fake.jpg");
        std::fs::write(&p, b"not a jpeg").unwrap();
        let item = parse_one(&p, &Thresholds::default());
        assert!(item.error.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_one_keeps_gps_when_temperature_parse_fails() {
        let dir = scratch("batch_gps");
        let p = dir.join("xmp_only.jpg");
        let mut bytes = b"\xff\xd8not-a-rjpeg".to_vec();
        bytes.extend_from_slice(
            br#"<x:xmpmeta drone-dji:GpsLatitude="+31.25" drone-dji:GpsLongitude="+121.5" xmp:CreateDate="2026-08-04T23:46:34">"#,
        );
        std::fs::write(&p, &bytes).unwrap();

        let item = parse_one(&p, &Thresholds::default());
        assert!(item.error.is_some(), "非辐射图应标记为解析失败");
        assert_eq!(item.lat, Some(31.25));
        assert_eq!(item.lon, Some(121.5));
        assert_eq!(item.datetime.as_deref(), Some("2026-08-04T23:46:34"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_radiometric_skips_non_rad() {
        let dir = scratch("batch_filter");
        std::fs::write(dir.join("fake.jpg"), b"not a jpeg").unwrap();
        let files = scan_folder(&dir, false).unwrap();
        let (items, skipped) = parse_radiometric(&files, &Thresholds::default(), |_, _| {});
        assert!(items.is_empty(), "非红外照片不应进入结果");
        assert_eq!(skipped, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_one_without_gps_leaves_null_coords() {
        let dir = scratch("batch_no_gps");
        let p = dir.join("fake.jpg");
        std::fs::write(&p, b"not a jpeg").unwrap();
        let item = parse_one(&p, &Thresholds::default());
        assert!(item.error.is_some());
        assert_eq!(item.lat, None);
        assert_eq!(item.lon, None);
        assert_eq!(item.datetime, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn progress_stride_caps_event_count() {
        assert_eq!(progress_stride(0), 1);
        assert_eq!(progress_stride(10), 1);
        assert_eq!(progress_stride(199), 1);
        assert_eq!(progress_stride(200), 1);
        assert_eq!(progress_stride(5000), 25);
    }
}
