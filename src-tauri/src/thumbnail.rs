//! 红外原图缩略图（离线地图点位详情弹窗按需加载）。
//!
//! 走 Tauri IPC 返回 data URL，而不是启用 asset protocol：用户选择的目录是
//! 动态的，无需运行时授权 scope，也不触碰 allowlist / Cargo features / CSP。

use std::io::Cursor;
use std::path::Path;

use image::ImageFormat;

use crate::error::AppResult;
use crate::utils;

/// 单个文件上限（64MB），避免误传超大文件拖垮 WebView
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// 缩略图长边默认值
pub const DEFAULT_MAX_EDGE: u32 = 320;

/// 把本地 JPG 等比缩到长边不超过 `max_edge`，返回 JPEG 字节。
fn encode_thumbnail(path: &Path, max_edge: u32) -> AppResult<Vec<u8>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "jpg" | "jpeg") {
        return Err(format!("仅支持 JPG 图片，当前扩展名为 .{}", ext));
    }
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("图片不可读（{}）：{}", file_name(path), e))?;
    if !meta.is_file() {
        return Err(format!("不是有效图片文件：{}", file_name(path)));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "图片过大（{} MB），不生成缩略图",
            meta.len() / 1024 / 1024
        ));
    }

    let edge = max_edge.clamp(64, 1024);
    let img = image::open(path)
        .map_err(|e| format!("缩略图解码失败（{}）：{}", file_name(path), e))?;
    let rgb = img.thumbnail(edge, edge).to_rgb8();
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut cursor, ImageFormat::Jpeg)
        .map_err(|e| format!("缩略图编码失败（{}）：{}", file_name(path), e))?;
    Ok(cursor.into_inner())
}

/// 返回可直接赋给 `<img src>` 的 data URL；解码失败时由前端降级为占位图。
pub fn make_thumbnail(path: &str, max_edge: u32) -> AppResult<String> {
    let bytes = encode_thumbnail(Path::new(path), max_edge)?;
    Ok(format!("data:image/jpeg;base64,{}", utils::to_base64(&bytes)))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ir_thumb_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("创建测试临时目录");
        dir
    }

    fn write_test_image(path: &Path, w: u32, h: u32) {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 80])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn thumbnail_scales_long_edge_and_encodes_jpeg() {
        let dir = scratch("ok");
        let p = dir.join("a.jpg");
        write_test_image(&p, 640, 320);

        let bytes = encode_thumbnail(&p, 320).expect("应生成缩略图");
        let decoded = image::load_from_memory(&bytes).expect("应是合法 JPEG");
        assert_eq!(decoded.width(), 320);
        assert_eq!(decoded.height(), 160);

        let url = make_thumbnail(p.to_str().unwrap(), 320).expect("应返回 data URL");
        let prefix = "data:image/jpeg;base64,";
        assert!(url.starts_with(prefix));
        assert!(url[prefix.len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn thumbnail_rejects_wrong_extension_missing_and_corrupt_file() {
        let dir = scratch("bad");
        let png = dir.join("a.png");
        write_test_image(&png, 20, 20);
        assert!(encode_thumbnail(&png, 320)
            .unwrap_err()
            .contains("仅支持 JPG"));

        let missing = dir.join("missing.jpg");
        assert!(encode_thumbnail(&missing, 320)
            .unwrap_err()
            .contains("图片不可读"));

        let corrupt = dir.join("bad.jpg");
        std::fs::write(&corrupt, b"not a jpeg").unwrap();
        assert!(encode_thumbnail(&corrupt, 320)
            .unwrap_err()
            .contains("解码失败"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 真实御 3T 辐射原图含多段 APP3 热数据，主 JPEG 仍必须能被解码成缩略图
    #[test]
    fn thumbnail_of_real_dji_rjpeg_is_decodable() {
        let mut candidates = Vec::new();
        if let Ok(p) = std::env::var("IR_SAMPLE_JPG") {
            candidates.push(PathBuf::from(p));
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        candidates.push(manifest.join("../DJI_20260804234634_0002_T.JPG"));
        candidates.push(manifest.join("../../DJI_20260804234634_0002_T.JPG"));
        let sample = candidates
            .into_iter()
            .find(|p| p.exists())
            .expect("未找到御3T样本（设置 IR_SAMPLE_JPG 指向真实辐射原图）");
        let url = make_thumbnail(sample.to_str().unwrap(), 320).expect("真实辐射原图应能生成缩略图");
        assert!(url.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn thumbnail_edge_is_clamped() {
        let dir = scratch("clamp");
        let p = dir.join("a.jpg");
        write_test_image(&p, 800, 400);
        let bytes = encode_thumbnail(&p, 100_000).expect("超大参数应被钳制");
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(decoded.width() <= 1024 && decoded.height() <= 1024);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
