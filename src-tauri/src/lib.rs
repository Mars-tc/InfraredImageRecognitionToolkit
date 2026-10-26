//! 红外照片分析工具核心库
//!
//! 模块划分（遵循开发文档第 6 节目录结构）：
//! - thermal:     辐射 R-JPEG 解析（thermal crate）+ EXIF 元数据
//! - temperature: 普朗克换算温度矩阵、发射率/距离修正
//! - roi:         ROI 统计（后续里程碑）
//! - diagnosis:   缺陷定级与三相 ΔT 比对（后续里程碑）
//! - export:      CSV / 标注图导出（后续里程碑）
//! - db:          SQLite 巡检台账（后续里程碑）
//! - report:      docx 巡检报告（后续里程碑）
//! - batch:       批量文件夹遍历（后续里程碑）
//! - settings:    阈值配置持久化（后续里程碑）

pub mod batch;
pub mod commands;
pub mod db;
pub mod diagnosis;
pub mod dji;
pub mod error;
pub mod export;
pub mod report;
pub mod roi;
pub mod settings;
pub mod thermal;
pub mod temperature;
pub mod utils;
