//! 独立工具：将辐射原图温度矩阵导出为 JSON（前端 e2e mock 数据源）
//!
//! 刻意不引用 lib（避免链接 tauri/WebView2Loader，规避 Windows-GNU 测试二进制
//! 启动故障），通过 #[path] 直接复用解析模块。
//!
//! 用法: cargo run --bin dump_temps -- <rjpeg路径> [输出json路径]
#![allow(dead_code)]

#[path = "../error.rs"]
mod error;
#[path = "../dji.rs"]
mod dji;
#[path = "../temperature.rs"]
mod temperature;
#[path = "../thermal.rs"]
mod thermal;

use thermal::load_rad_image;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("用法: dump_temps <rjpeg路径> [输出json路径]");
    let out = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "temps.json".to_string());

    let img = load_rad_image(path, None).expect("解析失败");
    let value = serde_json::json!({
        "path": img.meta.path,
        "file_name": img.meta.file_name,
        "camera_model": img.meta.camera_model,
        "datetime": img.meta.datetime,
        "gps": img.meta.gps,
        "width": img.width,
        "height": img.height,
        "emissivity": img.meta.emissivity,
        "distance": img.meta.distance,
        "humidity": img.meta.humidity,
        "reflected_temp": img.meta.reflected_temp,
        "atmospheric_temp": img.meta.atmospheric_temp,
        "temp_min": img.meta.temp_min,
        "temp_max": img.meta.temp_max,
        "temps": img.temps,
    });
    std::fs::write(&out, serde_json::to_string(&value).expect("json 序列化失败"))
        .expect("写文件失败");
    println!("written {} ({}x{})", out, img.width, img.height);
}
