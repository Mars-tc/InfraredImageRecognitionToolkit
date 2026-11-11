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

/// 温度标注点（由前端组装，坐标为图像像素坐标）
#[derive(Debug, serde::Deserialize, Clone)]
pub struct TempMark {
    pub x: f64,
    pub y: f64,
    /// ASCII 标签；点阵字模不支持的字符会被跳过
    pub label: String,
    pub color: [u8; 3],
}

/// F8：导出带标注红外图（伪彩 + ROI 矩形框 + 温度标注）
pub fn export_annotated_png_with_marks(
    temps: &[f32],
    width: usize,
    height: usize,
    rois: &[RoiOverlay],
    marks: &[TempMark],
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
    // 叠加温度标注：准星 + 深色底文字标签
    for mark in marks {
        let cx = mark.x.round() as i64;
        let cy = mark.y.round() as i64;
        draw_cross(&mut buf, cx, cy, mark.color);
        draw_label(&mut buf, cx, cy, &mark.label, mark.color);
    }
    buf.save(path).map_err(|e| format!("保存 PNG 失败：{e}"))?;
    Ok(())
}

/// 兼容包装：无温度标注时保持原有调用签名
pub fn export_annotated_png(
    temps: &[f32],
    width: usize,
    height: usize,
    rois: &[RoiOverlay],
    path: &Path,
) -> AppResult<()> {
    export_annotated_png_with_marks(temps, width, height, rois, &[], path)
}

/// 内置 5×7 ASCII 点阵字模：每行低 5 位从左到右对应 5 个像素列。
/// 逐像素绘制，不引入字体库或字体文件，满足离线内网约束。
static FONT: &[(char, [u8; 7])] = &[
    (' ', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
    ('-', [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
    ('+', [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00]),
    (':', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
    ('/', [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10]),
    ('0', [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
    ('1', [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('2', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
    ('3', [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E]),
    ('4', [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
    ('5', [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
    ('6', [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
    ('7', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    ('8', [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
    ('9', [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
    ('A', [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('B', [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
    ('C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
    ('D', [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E]),
    ('E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
    ('F', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
    ('G', [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F]),
    ('H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('J', [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C]),
    ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
    ('M', [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
    ('N', [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11]),
    ('O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
    ('Q', [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
    ('R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
    ('S', [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
    ('T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
    ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11]),
    ('X', [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
    ('Y', [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04]),
    ('Z', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
];

const GLYPH_W: i64 = 5;
const GLYPH_H: i64 = 7;
const SCALE: i64 = 2;

fn glyph(ch: char) -> Option<[u8; 7]> {
    FONT.iter().find(|(c, _)| *c == ch).map(|(_, g)| *g)
}

fn put_checked(buf: &mut image::RgbImage, x: i64, y: i64, color: [u8; 3]) {
    if x >= 0 && y >= 0 && x < buf.width() as i64 && y < buf.height() as i64 {
        buf.put_pixel(x as u32, y as u32, image::Rgb(color));
    }
}

/// 准星：圆环 + 十字，中心留空不遮挡热点像素
fn draw_cross(buf: &mut image::RgbImage, cx: i64, cy: i64, color: [u8; 3]) {
    const R: i64 = 6;
    for dy in -R..=R {
        for dx in -R..=R {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if (d - R as f64).abs() <= 0.7 {
                put_checked(buf, cx + dx, cy + dy, color);
            }
        }
    }
    for d in (-R - 4)..=(R + 4) {
        if d.abs() > 2 {
            put_checked(buf, cx + d, cy, color);
            put_checked(buf, cx, cy + d, color);
        }
    }
}

/// 文字标签：深色底 + 左侧色条 + 点阵字；整体钳制在图像边界内
fn draw_label(buf: &mut image::RgbImage, x: i64, y: i64, label: &str, color: [u8; 3]) {
    let chars: Vec<char> = label.chars().map(|c| c.to_ascii_uppercase()).collect();
    if chars.is_empty() {
        return;
    }
    let (w, h) = (buf.width() as i64, buf.height() as i64);
    let cell = (GLYPH_W + 1) * SCALE;
    let box_w = chars.len() as i64 * cell + 6;
    let box_h = GLYPH_H * SCALE + 6;
    let x0 = (x + 10).clamp(0, (w - box_w).max(0));
    let y0 = (y - box_h - 6).clamp(0, (h - box_h).max(0));

    for dy in 0..box_h {
        for dx in 0..box_w {
            put_checked(buf, x0 + dx, y0 + dy, [24, 30, 40]);
        }
        for dx in 0..2 {
            put_checked(buf, x0 + dx, y0 + dy, color);
        }
    }

    for (i, ch) in chars.iter().enumerate() {
        let Some(g) = glyph(*ch) else { continue };
        for (row, bits) in g.iter().enumerate() {
            for col in 0..GLYPH_W {
                if *bits & (1u8 << (4 - col)) != 0 {
                    for sy in 0..SCALE {
                        for sx in 0..SCALE {
                            put_checked(
                                buf,
                                x0 + 3 + i as i64 * cell + col * SCALE + sx,
                                y0 + 3 + row as i64 * SCALE + sy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
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

    #[test]
    fn png_export_with_temp_marks() {
        let temps: Vec<f32> = (0..640 * 64).map(|i| (i % 640) as f32).collect();
        let path = scratch("png");
        let marks = vec![TempMark {
            x: 100.0,
            y: 30.0,
            label: "IMG MAX 75.2C".to_string(),
            color: [255, 64, 32],
        }];
        export_annotated_png_with_marks(&temps, 640, 64, &[], &marks, &path).unwrap();
        let rgb = image::open(&path).unwrap().to_rgb8();
        // 准星十字臂（中心留空，不遮挡热点像素）
        assert_eq!(rgb.get_pixel(104, 30).0, [255, 64, 32]);
        assert_ne!(rgb.get_pixel(100, 30).0, [255, 64, 32]);
        // 文字标签深色底
        assert_eq!(rgb.get_pixel(112, 8).0, [24, 30, 40]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_marks_matches_legacy_export() {
        let temps: Vec<f32> = (0..64 * 32).map(|i| i as f32).collect();
        let a = scratch("png");
        let b = scratch("png");
        let rois = vec![RoiOverlay {
            x: 2.0,
            y: 2.0,
            w: 10.0,
            h: 8.0,
            color: [0, 166, 80],
        }];
        export_annotated_png(&temps, 64, 32, &rois, &a).unwrap();
        export_annotated_png_with_marks(&temps, 64, 32, &rois, &[], &b).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn marks_outside_bounds_are_clamped() {
        let temps: Vec<f32> = vec![20.0; 16 * 12];
        let path = scratch("png");
        let marks = vec![
            TempMark { x: -5.0, y: -5.0, label: "IMG MIN 20.0C".into(), color: [0, 128, 255] },
            TempMark { x: 1000.0, y: 1000.0, label: "IMG MAX 20.0C".into(), color: [255, 64, 32] },
        ];
        export_annotated_png_with_marks(&temps, 16, 12, &[], &marks, &path).unwrap();
        assert_eq!(image::open(&path).unwrap().width(), 16);
        let _ = std::fs::remove_file(&path);
    }
}
