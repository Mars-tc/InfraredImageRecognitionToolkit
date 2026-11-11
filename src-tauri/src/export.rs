//! 导出（F8）：温度矩阵 CSV + 带标注红外 PNG。

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::AppResult;

/// 铁红调色板（与前端 palettes.js 铁红一致）：黑-紫-红-橙-黄-白
const IRON_STOPS: [(f32, [u8; 3]); 6] = [
    (0.0, [0, 0, 4]),
    (0.25, [87, 16, 110]),
    (0.5, [188, 55, 84]),
    (0.7, [249, 142, 8]),
    (0.88, [252, 219, 9]),
    (1.0, [255, 255, 255]),
];

/// 温度 → 铁红 RGB（t 为归一化 0~1）
pub fn iron_color(t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    for w in IRON_STOPS.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return [
                (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * k) as u8,
                (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * k) as u8,
                (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * k) as u8,
            ];
        }
    }
    IRON_STOPS[IRON_STOPS.len() - 1].1
}

/// F8：导出温度矩阵 CSV（每行一个像素行，UTF-8 BOM 便于 Excel 识别）
pub fn export_csv(
    temps: &[f32],
    width: usize,
    height: usize,
    path: &Path,
) -> AppResult<usize> {
    if temps.len() != width * height {
        return Err("温度矩阵尺寸不匹配".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("创建文件失败：{e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"\xEF\xBB\xBF").map_err(io_err)?;
    writeln!(w, "行,列1..列{width}（温度 ℃）").map_err(io_err)?;
    for row in 0..height {
        let mut line = String::with_capacity(width * 8);
        for col in 0..width {
            if col > 0 {
                line.push(',');
            }
            line.push_str(&format!("{:.2}", temps[row * width + col]));
        }
        writeln!(w, "{row},{}", line).map_err(io_err)?;
    }
    w.flush().map_err(io_err)?;
    Ok(height * width)
}

/// 标注叠加层：矩形框（图像像素坐标）
pub struct RoiOverlay {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub color: [u8; 3],
}

/// 批量结果 CSV 行（F14）
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct BatchCsvRow {
    pub file_name: String,
    pub max: f64,
    pub min: f64,
    pub delta_t: f64,
    /// 缺陷等级名（其他/一般/重大/紧急/解析失败）
    pub level: String,
    pub path: String,
}

/// F14：批量解析结果导出 CSV（UTF-8 BOM，便于 Excel 直开）
pub fn export_batch_csv(rows: &[BatchCsvRow], path: &Path) -> AppResult<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("创建文件失败：{e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"\xEF\xBB\xBF").map_err(io_err)?;
    writeln!(w, "文件名,最高温(℃),最低温(℃),温差ΔT(℃),缺陷等级,路径").map_err(io_err)?;
    for r in rows {
        writeln!(
            w,
            "\"{}\",{:.2},{:.2},{:.2},{},\"{}\"",
            r.file_name, r.max, r.min, r.delta_t, r.level, r.path
        )
        .map_err(io_err)?;
    }
    w.flush().map_err(io_err)?;
    Ok(rows.len())
}

/// F8：导出带标注红外图（伪彩 + ROI 矩形框）
pub fn export_annotated_png(
    temps: &[f32],
    width: usize,
    height: usize,
    rois: &[RoiOverlay],
    path: &Path,
) -> AppResult<()> {
    if temps.len() != width * height {
        return Err("温度矩阵尺寸不匹配".to_string());
    }
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for t in temps {
        if t.is_finite() {
            min = min.min(*t);
            max = max.max(*t);
        }
    }
    let range = (max - min).max(1e-6);

    let mut buf = image::RgbImage::new(width as u32, height as u32);
    for (i, t) in temps.iter().enumerate() {
        let c = iron_color((t - min) / range);
        let x = (i % width) as u32;
        let y = (i / width) as u32;
        buf.put_pixel(x, y, image::Rgb(c));
    }
    // 叠加 ROI 矩形（2px 线宽）
    for roi in rois {
        let x0 = roi.x.max(0.0).floor() as i64;
        let y0 = roi.y.max(0.0).floor() as i64;
        let x1 = (roi.x + roi.w).min(width as f64).ceil() as i64 - 1;
        let y1 = (roi.y + roi.h).min(height as f64).ceil() as i64 - 1;
        draw_rect(&mut buf, x0, y0, x1, y1, roi.color, 2);
    }
    buf.save(path).map_err(|e| format!("保存 PNG 失败：{e}"))?;
    Ok(())
}

fn draw_rect(
    buf: &mut image::RgbImage,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    color: [u8; 3],
    thickness: i64,
) {
    let (w, h) = (buf.width() as i64, buf.height() as i64);
    let put = |buf: &mut image::RgbImage, x: i64, y: i64| {
        if x >= 0 && y >= 0 && x < w && y < h {
            buf.put_pixel(x as u32, y as u32, image::Rgb(color));
        }
    };
    for t in 0..thickness {
        for x in x0..=x1 {
            put(buf, x, y0 + t);
            put(buf, x, y1 - t);
        }
        for y in y0..=y1 {
            put(buf, x0 + t, y);
            put(buf, x1 - t, y);
        }
    }
}

fn io_err(e: std::io::Error) -> String {
    format!("写入失败：{e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iron_color_endpoints() {
        assert_eq!(iron_color(0.0), [0, 0, 4]);
        assert_eq!(iron_color(1.0), [255, 255, 255]);
        // 中段落在红-橙之间
        let c = iron_color(0.6);
        assert!(c[0] > 200);
    }

    /// 每轮独占的临时文件：唯一后缀必须放在扩展名之前，
    /// 否则 image crate 无法按扩展名推断 PNG 格式。
    fn scratch(ext: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ir_test_export_{}_{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            ext
        ))
    }

    #[test]
    fn csv_export_and_count() {
        let temps: Vec<f32> = (0..9).map(|i| i as f32).collect();
        let path = scratch("csv");
        let n = export_csv(&temps, 3, 3, &path).unwrap();
        assert_eq!(n, 9);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("0.00,1.00,2.00"));
        assert_eq!(&std::fs::read(&path).unwrap()[..3], b"\xEF\xBB\xBF");
        // 尺寸不匹配必须明确报错，避免写出半截矩阵
        assert!(export_csv(&temps, 4, 3, &path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn png_export_with_roi() {
        let temps: Vec<f32> = (0..640 * 64).map(|i| (i % 640) as f32).collect();
        let path = scratch("png");
        let rois = vec![RoiOverlay {
            x: 10.0,
            y: 10.0,
            w: 50.0,
            h: 30.0,
            color: [0, 255, 0],
        }];
        export_annotated_png(&temps, 640, 64, &rois, &path).unwrap();
        let img = image::open(&path).unwrap();
        assert_eq!(img.width(), 640);
        // ROI 边框像素应为绿色
        if let image::DynamicImage::ImageRgb8(rgb) = &img {
            let px = rgb.get_pixel(10, 10);
            assert_eq!(px.0, [0, 255, 0]);
        }
        let _ = std::fs::remove_file(&path);
    }
}
