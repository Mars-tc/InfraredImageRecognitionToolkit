//! 辐射 R-JPEG 解析与 EXIF 元数据提取。
//!
//! 两条解析路径：
//! 1. **标准 FLIR**（T 系列等手持热像仪）：FFF 编码段，由 `thermal` crate 默认特性解析；
//! 2. **大疆**（御 3T / M2EA / M30T / H20N）：APP3 原始热数据 + APP4 标定参数，
//!    见 [`crate::dji`]（thermal crate 不支持，文档风险#1 的适配实现）。
//!
//! 普通导出预览图（无原始热数据）两条路径都失败，给出明确中文提示。

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

// 注意：kamadak-exif 包的 lib 名是 `exif`（Cargo.toml [lib] name 重命名）
use exif::{Reader as ExifReader, Tag};
use serde::Serialize;
use thermal::temperature::ThermalSettings;
use thermal::ThermalImage;

use crate::dji;
use crate::error::AppResult;
use crate::temperature::{self, DEFAULT_DISTANCE};

/// 前端展示的图片元数据（F6）
#[derive(Debug, Serialize, Clone)]
pub struct ImageMeta {
    pub path: String,
    pub file_name: String,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    /// 拍摄时间（EXIF DateTimeOriginal 或 XMP CreateDate）
    pub datetime: Option<String>,
    /// GPS（十进制 "lat, lon"）
    pub gps: Option<String>,
    pub width: usize,
    pub height: usize,
    /// 发射率（0~1）
    pub emissivity: f64,
    /// 测量距离（米）
    pub distance: f64,
    /// 相对湿度（%）
    pub humidity: Option<f64>,
    /// 反射表观温度（℃）
    pub reflected_temp: Option<f64>,
    /// 大气温度（℃）
    pub atmospheric_temp: Option<f64>,
    /// 红外窗口温度（℃）
    pub ir_window_temp: Option<f64>,
    /// 当前温度矩阵范围（℃）
    pub temp_min: f64,
    pub temp_max: f64,
    /// 距离是否来自文件记录（false 表示使用默认值）
    pub distance_from_exif: bool,
    /// 解析路径："flir" 或 "dji"
    pub source_kind: String,
}

/// 已加载的辐射图片会话：保留原始 DN 矩阵，发射率修正时无需重新读文件。
pub struct LoadedImage {
    /// 原始 DN 值矩阵（行主序，长度 w×h）
    pub raw: Vec<f64>,
    pub settings: ThermalSettings,
    /// settings 的 JSON 形式（读取私有字段 / 发射率往返用）
    pub settings_json: serde_json::Value,
    pub width: usize,
    pub height: usize,
    pub distance: f64,
    pub meta: ImageMeta,
    /// 当前 ε/距离下的温度矩阵（℃，f32，行主序）
    pub temps: Vec<f32>,
}

impl LoadedImage {
    /// 用新发射率重算温度矩阵并刷新 meta（F7）
    pub fn recompute_with_emissivity(&mut self, emissivity: f64) -> AppResult<()> {
        let new_settings = temperature::with_emissivity(&self.settings, emissivity)?;
        self.temps = temperature::compute_temps(&self.raw, &new_settings, self.distance);
        self.settings = new_settings;
        self.settings_json = serde_json::to_value(&self.settings)
            .map_err(|e| format!("标定参数序列化失败：{}", e))?;
        let (min, max) = temperature::temp_range(&self.temps);
        self.meta.emissivity = emissivity;
        self.meta.temp_min = min;
        self.meta.temp_max = max;
        Ok(())
    }

    /// 用新测量距离重算温度矩阵并刷新 meta
    pub fn recompute_with_distance(&mut self, distance: f64) -> AppResult<()> {
        self.temps = temperature::compute_temps(&self.raw, &self.settings, distance);
        self.distance = distance;
        self.meta.distance = distance;
        let (min, max) = temperature::temp_range(&self.temps);
        self.meta.temp_min = min;
        self.meta.temp_max = max;
        Ok(())
    }
}

/// 解析辐射 R-JPEG。`distance_override` 可手动指定测量距离（米）。
pub fn load_rad_image(path: &str, distance_override: Option<f64>) -> AppResult<LoadedImage> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("文件不存在：{}", path));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(ext.as_str(), "jpg" | "jpeg") {
        return Err(format!(
            "仅支持 JPG 格式的辐射原图（R-JPEG），当前文件扩展名为 .{}",
            ext
        ));
    }

    // 1) 标准 FLIR 辐射图（FFF 段）
    if let Ok(ti) = ThermalImage::try_from_rjpeg_path(p) {
        let width = ti.image.ncols();
        let height = ti.image.nrows();
        if width == 0 || height == 0 {
            return Err("热数据尺寸异常（0 像素）".to_string());
        }
        let raw: Vec<f64> = ti.image.iter().copied().collect();
        let settings = ti.settings;
        let settings_json = serde_json::to_value(&settings)
            .map_err(|e| format!("标定参数序列化失败：{}", e))?;

        let exif = read_exif(p);
        let distance = distance_override
            .or(exif.subject_distance)
            .unwrap_or(DEFAULT_DISTANCE);
        let temps = temperature::compute_temps(&raw, &settings, distance);
        let (temp_min, temp_max) = temperature::temp_range(&temps);

        let meta = ImageMeta {
            path: path.to_string(),
            file_name: file_name_of(p),
            camera_make: exif.make.clone(),
            camera_model: exif.model.clone(),
            datetime: exif.datetime.clone(),
            gps: exif.gps_string(),
            width,
            height,
            emissivity: temperature::settings_f64(&settings_json, "Emissivity").unwrap_or(f64::NAN),
            distance,
            humidity: temperature::settings_f64(&settings_json, "RelativeHumidity"),
            reflected_temp: temperature::settings_f64(&settings_json, "ReflectedApparentTemperature"),
            atmospheric_temp: temperature::settings_f64(&settings_json, "AtmosphericTemperature"),
            ir_window_temp: temperature::settings_f64(&settings_json, "IRWindowTemperature"),
            temp_min,
            temp_max,
            distance_from_exif: distance_override.is_none() && exif.subject_distance.is_some(),
            source_kind: "flir".to_string(),
        };
        return Ok(LoadedImage {
            raw,
            settings,
            settings_json,
            width,
            height,
            distance,
            meta,
            temps,
        });
    }

    // 2) 大疆 R-JPEG（APP3/APP4）
    let bytes = std::fs::read(p).map_err(|e| format!("读取文件失败：{}", e))?;
    match dji::parse_dji_rjpeg(&bytes) {
        Ok(img) => {
            let distance = distance_override.unwrap_or(img.params.object_distance);
            let temps = temperature::compute_temps(&img.raw, &img.settings, distance);
            let (temp_min, temp_max) = temperature::temp_range(&temps);
            let exif = read_exif(p);
            let xmp = dji::read_xmp_meta(&bytes);

            let meta = ImageMeta {
                path: path.to_string(),
                file_name: file_name_of(p),
                camera_make: exif.make.clone().or(xmp.make.clone()),
                camera_model: exif.model.clone().or(xmp.model.clone()),
                datetime: exif.datetime.clone().or(xmp.create_date.clone()),
                gps: exif.gps_string().or(xmp.gps.clone()),
                width: img.width,
                height: img.height,
                emissivity: img.params.emissivity,
                distance,
                humidity: Some(img.params.relative_humidity),
                reflected_temp: Some(img.params.reflected_temperature),
                atmospheric_temp: Some(img.params.atmospheric_temperature),
                ir_window_temp: Some(img.params.ir_window_temperature),
                temp_min,
                temp_max,
                distance_from_exif: distance_override.is_none() && img.params.from_app4,
                source_kind: "dji".to_string(),
            };
            Ok(LoadedImage {
                raw: img.raw,
                settings: img.settings,
                settings_json: img.settings_json,
                width: img.width,
                height: img.height,
                distance,
                meta,
                temps,
            })
        }
        Err(e) => Err(format!(
            "{}\n（标准 FLIR 解析也未成功：请确认这是辐射原图 R-JPEG，而非普通导出预览图）",
            e
        )),
    }
}

fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

// ---------- EXIF ----------

struct ExifInfo {
    make: Option<String>,
    model: Option<String>,
    datetime: Option<String>,
    gps_lat: Option<f64>,
    gps_lon: Option<f64>,
    subject_distance: Option<f64>,
}

impl ExifInfo {
    fn gps_string(&self) -> Option<String> {
        match (self.gps_lat, self.gps_lon) {
            (Some(lat), Some(lon)) => Some(format!("{:.6}, {:.6}", lat, lon)),
            _ => None,
        }
    }
}

fn read_exif(path: &Path) -> ExifInfo {
    let mut info = ExifInfo {
        make: None,
        model: None,
        datetime: None,
        gps_lat: None,
        gps_lon: None,
        subject_distance: None,
    };
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return info,
    };
    let exif = match ExifReader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(e) => e,
        Err(_) => return info,
    };

    let mut lat_ref: Option<String> = None;
    let mut lon_ref: Option<String> = None;

    for field in exif.fields() {
        let text = || {
            field
                .value
                .display_as(field.tag)
                .to_string()
                .trim()
                .trim_end_matches('\0')
                .trim_matches('"')
                .to_string()
        };
        match field.tag {
            Tag::Make => info.make = Some(text()),
            Tag::Model => info.model = Some(text()),
            Tag::DateTimeOriginal | Tag::DateTime => {
                if info.datetime.is_none() {
                    let t = text();
                    if !t.is_empty() {
                        info.datetime = Some(t);
                    }
                }
            }
            Tag::SubjectDistance => info.subject_distance = first_rational(&field.value),
            Tag::GPSLatitudeRef => lat_ref = Some(text()),
            Tag::GPSLongitudeRef => lon_ref = Some(text()),
            Tag::GPSLatitude => {
                info.gps_lat = dms_to_decimal(&field.value, lat_ref.as_deref());
            }
            Tag::GPSLongitude => {
                info.gps_lon = dms_to_decimal(&field.value, lon_ref.as_deref());
            }
            _ => {}
        }
    }
    info
}

/// 度分秒（3 个有理数）转十进制度，按 N/S/E/W 参考定符号
fn dms_to_decimal(value: &exif::Value, ref_str: Option<&str>) -> Option<f64> {
    let parts = rationals_to_f64(value)?;
    if parts.len() < 3 {
        return None;
    }
    let mut dec = parts[0] + parts[1] / 60.0 + parts[2] / 3600.0;
    if let Some(r) = ref_str {
        if r.starts_with('S') || r.starts_with('W') {
            dec = -dec;
        }
    }
    Some(dec)
}

fn first_rational(value: &exif::Value) -> Option<f64> {
    rationals_to_f64(value).and_then(|v| v.first().copied())
}

fn rationals_to_f64(value: &exif::Value) -> Option<Vec<f64>> {
    use exif::Value;
    match value {
        Value::Rational(rs) => Some(
            rs.iter()
                .map(|r| if r.denom == 0 { 0.0 } else { r.num as f64 / r.denom as f64 })
                .collect(),
        ),
        Value::SRational(rs) => Some(
            rs.iter()
                .map(|r| if r.denom == 0 { 0.0 } else { r.num as f64 / r.denom as f64 })
                .collect(),
        ),
        _ => None,
    }
}
