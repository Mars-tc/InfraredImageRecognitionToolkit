//! 批量文件夹遍历（F14）：递归扫描辐射图 + 逐张解析 + 进度上报。
//!
//! 文档风险 #5：解析放 spawn_blocking 线程执行，避免卡死 Webview。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::diagnosis;
use crate::error::AppResult;
use crate::settings::Thresholds;
use crate::thermal::load_rad_image;

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
}

/// 批量汇总
#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub total: usize,
    pub ok: usize,
    pub failed: usize,
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

/// 解析单张并按阈值定级（失败时返回带 error 的条目）
pub fn parse_one(path: &Path, thresholds: &Thresholds) -> BatchItem {
    let item = BatchItem {
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
    };
    match load_rad_image(&path.to_string_lossy(), None) {
        Ok(img) => {
            let mut max = f64::MIN;
            let mut min = f64::MAX;
            for t in &img.temps {
                if t.is_finite() {
                    max = max.max(*t as f64);
                    min = min.min(*t as f64);
                }
            }
            if !max.is_finite() {
                return BatchItem {
                    error: Some("图像内无有效温度数据".to_string()),
                    ..item
                };
            }
            let stats = crate::roi::RoiStats {
                max,
                min,
                avg: (max + min) / 2.0,
                delta_t: max - min,
                count: img.temps.len(),
            };
            let verdict = diagnosis::verdict(&stats, thresholds);
            BatchItem {
                max,
                min,
                delta_t: max - min,
                level: verdict.level,
                ..item
            }
        }
        Err(e) => BatchItem {
            error: Some(e),
            ..item
        },
    }
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
}
