//! 大疆（DJI）R-JPEG 适配解析。
//!
//! 御 3T（M3T）/ Mavic 2 行业进阶版（M2EA）/ M30T / H20N 等机型的辐射原图
//! **不使用**标准 FLIR FFF 段，thermal crate 无法直接解析（文档风险#1）。
//! 实测结构（ExifTool `DJI.pm` ThermalParams2 + OpenDroneMap thermal_tools）：
//!
//! - 原始 16bit 小端热数据分散在若干 **APP3** 段中，按顺序拼接后恰为 w×h×2 字节
//!   （M3T/M2EA 为 640×512 = 655360 字节）；
//! - 标定参数在 **APP4** 段：段内偏移 0x20 起为 5 个 f32 小端
//!   （环境温/距离/发射率/相对湿度/反射温），此处按数值合理性启发式定位；
//! - 机型、GPS、时间在 XMP（drone-dji / tiff 命名空间）。
//!
//! 温度换算沿用 Thermimage 体系（与 thermal crate、flirimageextractor 一致）。
//! DJI 机型多数不在文件中存储普朗克常量，采用 FLIR Lepton 3.5 社区标准默认值
//! （与 OpenDroneMap、detecttechnologies/thermal_base 相同），可在设置中覆盖。

use serde::Serialize;
use thermal::temperature::ThermalSettings;

use crate::error::AppResult;
use crate::temperature::compute_temps;

/// FLIR Lepton 3.5 普朗克常量（DJI 640×512 机型的社区标准默认值）
pub const LEPTON_R1: f64 = 21106.77;
pub const LEPTON_B: f64 = 1501.0;
pub const LEPTON_F: f64 = 1.0;
pub const LEPTON_O: f64 = -7340.0;
pub const LEPTON_R2: f64 = 0.012545258;

/// FLIR 标准大气传输常数（Thermimage raw2temp.R）
const ATA1: f64 = 0.006569;
const ATA2: f64 = 0.01262;
const ATB1: f64 = -0.002276;
const ATB2: f64 = -0.00667;
const ATX: f64 = 1.9;

/// DJI 机型标定参数（APP4）
#[derive(Debug, Clone, Serialize)]
pub struct DjiParams {
    /// 发射率（0~1）
    pub emissivity: f64,
    /// 测量距离（米）
    pub object_distance: f64,
    /// 大气温度（℃）
    pub atmospheric_temperature: f64,
    /// 反射表观温度（℃）
    pub reflected_temperature: f64,
    /// 相对湿度（%）
    pub relative_humidity: f64,
    /// 红外窗口温度（℃）
    pub ir_window_temperature: f64,
    /// 红外窗口传输率（0~1）
    pub ir_window_transmission: f64,
    /// 参数是否真实取自 APP4（false 表示使用默认值）
    pub from_app4: bool,
}

impl Default for DjiParams {
    fn default() -> Self {
        DjiParams {
            emissivity: 0.95,
            object_distance: 1.0,
            atmospheric_temperature: 20.0,
            reflected_temperature: 20.0,
            relative_humidity: 50.0,
            ir_window_temperature: 20.0,
            ir_window_transmission: 1.0,
            from_app4: false,
        }
    }
}

/// 解析后的 DJI 辐射图片
pub struct DjiImage {
    /// 原始 DN 值矩阵（行主序，长度 w×h）
    pub raw: Vec<f64>,
    pub width: usize,
    pub height: usize,
    pub settings: ThermalSettings,
    /// settings 的 JSON 形式（发射率修正往返用）
    pub settings_json: serde_json::Value,
    pub params: DjiParams,
}

/// 解析大疆 R-JPEG 文件字节
pub fn parse_dji_rjpeg(bytes: &[u8]) -> AppResult<DjiImage> {
    let segments = walk_jpeg_segments(bytes)?;

    let raw_bytes: Vec<u8> = segments.app3.concat();
    let (width, height) = match_dimensions(raw_bytes.len())?;
    let count = width * height;
    let raw: Vec<f64> = raw_bytes
        .chunks_exact(2)
        .take(count)
        .map(|c| u16::from_le_bytes([c[0], c[1]]) as f64)
        .collect();
    if raw.len() != count {
        return Err("APP3 热数据不完整".to_string());
    }

    let params = parse_app4_params(segments.app4.as_deref());
    let settings = build_settings(&params)?;
    let settings_json =
        serde_json::to_value(&settings).map_err(|e| format!("标定参数序列化失败：{}", e))?;

    Ok(DjiImage {
        raw,
        width,
        height,
        settings,
        settings_json,
        params,
    })
}

/// 用 DJI 参数 + Lepton 常量构造 ThermalSettings（复用 thermal crate 的换算实现）。
/// 注意：ThermalSettings 的 emissivity 字段必须是 JSON 数字，其余字段为字符串数值。
pub fn build_settings(p: &DjiParams) -> AppResult<ThermalSettings> {
    use serde_json::Value;
    let mut obj = serde_json::Map::new();
    // 以下 4 个字段走 float_with_suffix（接受 exiftool 式 "25.0 C" 字符串），
    // 其余字段为普通 f64，必须给 JSON 数字
    for (k, v) in [
        ("RelativeHumidity", p.relative_humidity),
        ("ReflectedApparentTemperature", p.reflected_temperature),
        ("IRWindowTemperature", p.ir_window_temperature),
        ("AtmosphericTemperature", p.atmospheric_temperature),
    ] {
        obj.insert(
            k.to_string(),
            Value::String(crate::temperature::num_to_str_pub(v)),
        );
    }
    for (k, v) in [
        ("Emissivity", p.emissivity),
        ("IRWindowTransmission", p.ir_window_transmission),
        ("PlanckR1", LEPTON_R1),
        ("PlanckB", LEPTON_B),
        ("PlanckF", LEPTON_F),
        ("PlanckO", LEPTON_O),
        ("PlanckR2", LEPTON_R2),
        ("AtmosphericTransAlpha1", ATA1),
        ("AtmosphericTransAlpha2", ATA2),
        ("AtmosphericTransBeta1", ATB1),
        ("AtmosphericTransBeta2", ATB2),
        ("AtmosphericTransX", ATX),
    ] {
        obj.insert(k.to_string(), serde_json::json!(v));
    }
    serde_json::from_value(Value::Object(obj))
        .map_err(|e| format!("DJI 标定参数构造失败：{}", e))
}

/// 计算 DJI 图片的温度矩阵（℃）
pub fn compute_dji_temps(img: &DjiImage) -> Vec<f32> {
    compute_temps(&img.raw, &img.settings, img.params.object_distance)
}

// ---------- JPEG 段遍历 ----------

struct JpegSegments {
    /// APP3 段载荷（热数据，按出现顺序）
    app3: Vec<Vec<u8>>,
    /// APP4 段载荷（标定参数）
    app4: Option<Vec<u8>>,
}

fn walk_jpeg_segments(bytes: &[u8]) -> AppResult<JpegSegments> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("不是有效的 JPEG 文件".to_string());
    }
    let mut segs = JpegSegments {
        app3: Vec::new(),
        app4: None,
    };
    let mut pos = 2usize;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            return Err("JPEG 段标记损坏".to_string());
        }
        let mut marker = bytes[pos + 1];
        while marker == 0xFF && pos + 3 < bytes.len() {
            pos += 1;
            marker = bytes[pos + 1];
        }
        pos += 2;
        // EOI / SOS：SOS 之后是扫描数据，不再有段
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if pos + 2 > bytes.len() {
            return Err("JPEG 段长度字段不完整".to_string());
        }
        let len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        if len < 2 || pos + len > bytes.len() {
            return Err("JPEG 段长度异常".to_string());
        }
        let data = &bytes[pos + 2..pos + len];
        match marker {
            0xE3 => segs.app3.push(data.to_vec()),
            0xE4 => {
                if segs.app4.is_none() {
                    segs.app4 = Some(data.to_vec());
                }
            }
            _ => {}
        }
        pos += len;
    }
    if segs.app3.is_empty() {
        return Err("未找到 APP3 热数据段（不是大疆辐射原图 R-JPEG）".to_string());
    }
    Ok(segs)
}

/// 已知 DJI 热像仪分辨率（按字节数匹配）
fn match_dimensions(len: usize) -> AppResult<(usize, usize)> {
    const KNOWN: &[(usize, usize)] = &[
        (640, 512),  // M3T / M2EA / M30T / H20N / XT2
        (512, 384),
        (1024, 768),
        (1280, 960),
    ];
    for &(w, h) in KNOWN {
        if len == w * h * 2 {
            return Ok((w, h));
        }
    }
    if len >= 640 * 512 * 2 {
        Ok((640, 512))
    } else {
        Err(format!(
            "APP3 热数据共 {} 字节，与已知大疆机型分辨率不匹配",
            len
        ))
    }
}

// ---------- APP4 标定参数 ----------

/// 在 APP4 中按数值合理性定位 ThermalParams2 参数块（5 个 f32 小端）：
/// 环境温 / 距离 / 发射率 / 相对湿度 / 反射温。逐字节扫描，不假设对齐。
/// 发射率 ≥ 0.5 是关键判别条件（DJI 默认 0.95），可排除头部随机字节的误匹配。
fn parse_app4_params(app4: Option<&[u8]>) -> DjiParams {
    let Some(data) = app4 else {
        return DjiParams::default();
    };
    if data.len() < 20 {
        return DjiParams::default();
    }
    for off in 0..=data.len() - 20 {
        let vals: [f32; 5] = [
            f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
            f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]),
            f32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]),
            f32::from_le_bytes([data[off + 12], data[off + 13], data[off + 14], data[off + 15]]),
            f32::from_le_bytes([data[off + 16], data[off + 17], data[off + 18], data[off + 19]]),
        ];
        let [ambient, distance, emissivity, humidity, reflected] = vals;
        let ok = ambient.is_finite()
            && (-40.0..=80.0).contains(&ambient)
            && distance.is_finite()
            && (0.5..=1000.0).contains(&distance)
            && emissivity.is_finite()
            && (0.5..=1.0).contains(&emissivity)
            && humidity.is_finite()
            && (0.0..=1.0).contains(&humidity)
            && reflected.is_finite()
            && (-40.0..=80.0).contains(&reflected);
        if ok {
            return DjiParams {
                emissivity: emissivity as f64,
                object_distance: distance as f64,
                atmospheric_temperature: ambient as f64,
                reflected_temperature: reflected as f64,
                relative_humidity: humidity as f64 * 100.0,
                ir_window_temperature: 20.0,
                ir_window_transmission: 1.0,
                from_app4: true,
            };
        }
    }
    DjiParams::default()
}

// ---------- XMP 元数据补充（DJI 机型/GPS/时间写在 XMP 而非 EXIF） ----------

/// 从文件字节中提取 XMP 的 DJI 字段（latin1 视图正则匹配）
pub struct XmpMeta {
    pub make: Option<String>,
    pub model: Option<String>,
    pub gps: Option<String>,
    pub create_date: Option<String>,
}

pub fn read_xmp_meta(bytes: &[u8]) -> XmpMeta {
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(200_000)]);
    let grab = |key: &str| -> Option<String> {
        let pat = format!("{}=\"", key);
        let start = text.find(&pat)? + pat.len();
        let end = text[start..].find('"')? + start;
        Some(text[start..end].trim().to_string())
    };
    let make = grab("tiff:Make");
    let model = grab("tiff:Model");
    let lat = grab("drone-dji:GpsLatitude").and_then(|s| s.parse::<f64>().ok());
    let lon = grab("drone-dji:GpsLongitude").and_then(|s| s.parse::<f64>().ok());
    let gps = match (lat, lon) {
        (Some(a), Some(o)) => Some(format!("{:.6}, {:.6}", a, o)),
        _ => None,
    };
    let create_date = grab("xmp:CreateDate").or_else(|| grab("drone-dji:UTCAtExposure"));
    XmpMeta {
        make,
        model,
        gps,
        create_date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temperature::{compute_temps, temp_range, with_emissivity};
    use crate::thermal::load_rad_image;

    /// 样本路径：环境变量优先，其次仓库内相对路径
    fn sample_path(env_key: &str, fallback_rel: &str) -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var(env_key) {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(fallback_rel);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    }

    fn assert_sane(img: &crate::thermal::LoadedImage, label: &str) {
        assert_eq!(img.width, 640, "[{}] 宽度应为 640", label);
        assert_eq!(img.height, 512, "[{}] 高度应为 512", label);
        assert_eq!(img.temps.len(), 640 * 512);
        assert!(
            img.temps.iter().all(|t| t.is_finite()),
            "[{}] 温度矩阵存在 NaN/Inf",
            label
        );
        assert!(
            img.meta.temp_max > img.meta.temp_min,
            "[{}] min={} max={}",
            label,
            img.meta.temp_min,
            img.meta.temp_max
        );
        assert!(
            img.meta.temp_min > -80.0 && img.meta.temp_max < 650.0,
            "[{}] 温度超出合理量程：min={} max={}",
            label,
            img.meta.temp_min,
            img.meta.temp_max
        );
        assert!(
            img.meta.emissivity > 0.05 && img.meta.emissivity <= 1.0,
            "[{}] 发射率异常：{}",
            label,
            img.meta.emissivity
        );
    }

    #[test]
    fn build_settings_and_emissivity_roundtrip() {
        // 验证：DJI 参数能构造 ThermalSettings，且发射率修正往返生效
        let p = DjiParams::default();
        let s = build_settings(&p).expect("构造 settings");
        let temps_before = compute_temps(&[19388.0; 16], &s, p.object_distance);
        let s2 = crate::temperature::with_emissivity(&s, 0.90).expect("ε 往返");
        let temps_after = compute_temps(&[19388.0; 16], &s2, p.object_distance);
        assert!(
            (temps_after[0] - temps_before[0]).abs() > 0.1,
            "ε 修正未生效：{} -> {}",
            temps_before[0],
            temps_after[0]
        );
    }

    #[test]
    fn app4_params_heuristic_on_synthetic() {
        // 构造 [8 字节头][5 个 f32] 的 APP4，验证启发式定位
        let mut data = vec![0xFF, 0xD2, 0xD1, 0xFF, b'i', b'i', b'r', b'p'];
        data.extend_from_slice(&[0u8; 24]);
        for v in [25.0f32, 5.0, 0.95, 0.5, 25.0] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let p = parse_app4_params(Some(&data));
        assert!(p.from_app4);
        assert!((p.emissivity - 0.95).abs() < 1e-6);
        assert!((p.object_distance - 5.0).abs() < 1e-6);
        assert!((p.atmospheric_temperature - 25.0).abs() < 1e-6);
        assert!((p.relative_humidity - 50.0).abs() < 1e-6);
    }

    #[test]
    fn parse_m3t_sample() {
        // 样本缺失必须失败，不能静默跳过造成「零断言假绿」
        let Some(path) = sample_path("IR_SAMPLE_JPG", "DJI_20260804234634_0002_T.JPG") else {
            panic!("未找到御3T样本（设置 IR_SAMPLE_JPG 指向真实辐射原图）");
        };
        let img = load_rad_image(path.to_str().expect("path utf8"), None).expect("解析样本失败");
        assert_eq!(img.meta.source_kind, "dji");
        assert_sane(&img, "M3T");
        // 御3T 实拍样本（夜间场景，环境温 25℃）：参数块必须正确提取
        assert!(
            img.meta.emissivity > 0.8 && img.meta.emissivity <= 1.0,
            "发射率应约 0.95，实际 {}",
            img.meta.emissivity
        );
        assert!(
            img.meta.temp_min > 10.0 && img.meta.temp_max < 60.0,
            "夜间场景温度应在 10~60℃，实际 {}~{}",
            img.meta.temp_min,
            img.meta.temp_max
        );
        eprintln!("[M3T] meta = {:#?}", img.meta);
    }

    #[test]
    fn parse_dji_official_sample() {
        let Some(path) = sample_path("IR_SAMPLE_DJI", "scripts/samples/DJI_0001_R.jpg") else {
            panic!("未找到 DJI 官方样本（设置 IR_SAMPLE_DJI 指向真实辐射原图）");
        };
        let img = load_rad_image(path.to_str().expect("path utf8"), None).expect("解析样本失败");
        assert_eq!(img.meta.source_kind, "dji");
        assert_sane(&img, "DJI_OFFICIAL");
    }
}
