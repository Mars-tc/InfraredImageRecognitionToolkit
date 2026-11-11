//! 后端单元测试入口（不链接 tauri）。
//!
//! 主 crate 的 `cargo test` 需要 webkit2gtk-4.0 / gtk-3 / glib-2.0 的开发库，
//! 精简开发机与内网 CI 往往没有；这里沿用 `bin/dump_temps.rs` 的 `#[path]` 做法，
//! 直接把 src-tauri/src 下的模块挂到本 crate 根上，模块内原有的 `#[cfg(test)]`
//! 用例会照常编译运行。测试的是同一份源码文件，不是复制品。
#![allow(dead_code, unused_imports)]

// settings.rs::config_path 的签名需要 &tauri::Config。测试目标不链接 tauri，
// 用自引用给一个最小替身，避免为了单个函数引入整套 GTK 依赖。
extern crate self as tauri;

pub struct Config;

pub mod api {
    pub mod path {
        pub fn app_config_dir(_: &crate::Config) -> Option<std::path::PathBuf> {
            None
        }
    }
}

#[path = "../../src/error.rs"]
pub mod error;
#[path = "../../src/utils.rs"]
pub mod utils;
#[path = "../../src/settings.rs"]
pub mod settings;
#[path = "../../src/temperature.rs"]
pub mod temperature;
#[path = "../../src/thermal.rs"]
pub mod thermal;
#[path = "../../src/dji.rs"]
pub mod dji;
#[path = "../../src/roi.rs"]
pub mod roi;
#[path = "../../src/diagnosis.rs"]
pub mod diagnosis;
#[path = "../../src/export.rs"]
pub mod export;
#[path = "../../src/db.rs"]
pub mod db;
#[path = "../../src/report.rs"]
pub mod report;
#[path = "../../src/batch.rs"]
pub mod batch;
#[path = "../../src/thumbnail.rs"]
pub mod thumbnail;

#[cfg(test)]
mod pipeline_tests {
    use std::path::PathBuf;

    /// 每轮测试独占的临时目录，避免 cargo test 并行 / 多次运行互相踩踏。
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ir_pipeline_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    /// 真实御 3T 辐射原图全链路：
    /// R-JPEG 解析 → 温度矩阵 → CSV / 带 ROI 标注 PNG / Word 报告产物可用。
    #[test]
    fn real_sample_parses_and_exports() {
        let sample = std::env::var("IR_SAMPLE_JPG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../DJI_20260804234634_0002_T.JPG")
            });
        assert!(sample.exists(), "缺少真实样本：{}", sample.display());

        let img = crate::thermal::load_rad_image(&sample.to_string_lossy(), None)
            .expect("真实辐射原图应能解析");
        assert_eq!((img.width, img.height), (640, 512));
        assert_eq!(img.temps.len(), img.width * img.height);
        assert!(img.temps.iter().all(|t| t.is_finite()), "温度矩阵含非有限值");
        let (min, max) = crate::temperature::temp_range(&img.temps);
        assert!(min > 10.0 && max < 60.0, "温度范围异常：{min} ~ {max}");
        assert!((img.meta.emissivity - 0.95).abs() < 0.01);

        let dir = scratch("sample");

        // F8：温度矩阵 CSV
        let csv = dir.join("temps.csv");
        let n = crate::export::export_csv(&img.temps, img.width, img.height, &csv)
            .expect("导出 CSV");
        assert_eq!(n, img.temps.len());
        let head = std::fs::read(&csv).expect("读取 CSV");
        assert_eq!(&head[..3], b"\xEF\xBB\xBF", "CSV 应带 UTF-8 BOM");
        assert!(head.len() > img.width, "CSV 内容过短");

        // F8：带 ROI 标注 PNG（只画小框，避免全图重绘放大耗时）
        let png = dir.join("annotated.png");
        let rois = [crate::export::RoiOverlay {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
            color: [0, 255, 0],
        }];
        crate::export::export_annotated_png(&img.temps, img.width, img.height, &rois, &png)
            .expect("导出标注图");
        let decoded = image::open(&png).expect("标注图应可解码");
        assert_eq!((decoded.width(), decoded.height()), (640, 512));

        // F16：Word 缺陷报告（用真实解析出的最高温构造一条台账记录）
        let record = sample_record(max, &sample);
        let docx = dir.join("report.docx");
        crate::report::export_report(
            &[record],
            &docx,
            "测试员",
            "2026-11-04",
            "测试班组",
        )
        .expect("导出报告");
        let bytes = std::fs::read(&docx).expect("读取报告");
        assert_eq!(&bytes[..2], b"PK", "docx 应为 zip 容器");
        assert!(bytes.len() > 1000, "报告内容过短");

        std::fs::remove_dir_all(&dir).ok();
    }

    fn sample_record(max: f64, sample: &std::path::Path) -> crate::db::LedgerRecord {
        crate::db::LedgerRecord {
            id: 1,
            tower_no: "10kV-测试线-001".into(),
            equipment: "电缆接头".into(),
            phase: "A".into(),
            image_path: sample.to_string_lossy().to_string(),
            max_temp: max,
            delta_t: 3.0,
            level: 0,
            note: "各项指标正常".into(),
            created_at: "2026-11-04".into(),
        }
    }
}
