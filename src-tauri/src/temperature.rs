//! 普朗克公式温度换算与发射率 / 距离修正。
//!
//! 算法由 `thermal` crate 提供（Thermimage R 库的移植），本模块负责：
//! 1. 把原始 DN 矩阵批量换算为温度矩阵（℃）；
//! 2. 通过 serde 往返修改发射率（ThermalSettings 字段私有但 derive Serialize/Deserialize）；
//! 3. 温度矩阵范围统计。

use thermal::temperature::ThermalSettings;

use crate::error::AppResult;

/// EXIF 无距离记录时的默认测量距离（米）。
/// thermal crate 文档指出：用 1.0 代替真实值 50.0 的绝对误差约 2-3℃，
/// 而温差（ΔT，本工具最关心的判据）的相对误差很小。
pub const DEFAULT_DISTANCE: f64 = 1.0;

/// 用指定距离把原始 DN 矩阵换算为温度矩阵（℃，行主序 f32）。
pub fn compute_temps(raw: &[f64], settings: &ThermalSettings, distance: f64) -> Vec<f32> {
    let transform = settings.temperature_transform(distance);
    raw.iter().map(|&r| transform(r) as f32).collect()
}

/// 修改发射率得到新的标定参数（F7 发射率修正）。
/// ThermalSettings 字段私有，借助其 Serialize/Deserialize（PascalCase）做 JSON 往返。
/// 注意：仅 RelativeHumidity / ReflectedApparentTemperature / IRWindowTemperature /
/// AtmosphericTemperature 四个字段带 float_with_suffix（须为字符串数值），
/// 其余字段（含 Emissivity）为普通 f64，必须保持 JSON 数字。
pub fn with_emissivity(settings: &ThermalSettings, emissivity: f64) -> AppResult<ThermalSettings> {
    if !(0.05..=1.0).contains(&emissivity) {
        return Err("发射率应在 0.05 ~ 1.0 之间".to_string());
    }
    let mut json = serde_json::to_value(settings)
        .map_err(|e| format!("标定参数序列化失败：{}", e))?;
    if let Some(obj) = json.as_object_mut() {
        for (k, v) in obj.iter_mut() {
            if STRING_FIELDS.contains(&k.as_str()) {
                if let Some(n) = v.as_f64() {
                    *v = serde_json::json!(num_to_str(n));
                }
            }
        }
        obj.insert(
            "Emissivity".to_string(),
            serde_json::json!(emissivity),
        );
    }
    serde_json::from_value(json).map_err(|e| format!("标定参数反序列化失败：{}", e))
}

/// ThermalSettings 中带 float_with_suffix 反序列化器的字段（须为字符串数值）
const STRING_FIELDS: &[&str] = &[
    "RelativeHumidity",
    "ReflectedApparentTemperature",
    "IRWindowTemperature",
    "AtmosphericTemperature",
];

/// f64 转十进制字符串（避免科学计数法，float_with_suffix 的正则只认十进制）
fn num_to_str(v: f64) -> String {
    let s = v.to_string();
    if s.contains('e') || s.contains('E') {
        format!("{:.*}", 17, v)
    } else {
        s
    }
}

/// [`num_to_str`] 的公开版本（供 dji 模块构造 settings 使用）
pub fn num_to_str_pub(v: f64) -> String {
    num_to_str(v)
}

/// 温度矩阵的 min / max（℃，过滤 NaN）。
pub fn temp_range(temps: &[f32]) -> (f64, f64) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &t in temps {
        if t.is_finite() {
            if t < min {
                min = t;
            }
            if t > max {
                max = t;
            }
        }
    }
    if min > max {
        return (f64::NAN, f64::NAN);
    }
    (min as f64, max as f64)
}

/// 从 ThermalSettings 序列化结果中读取数值字段（如 Emissivity / RelativeHumidity）。
pub fn settings_f64(settings_json: &serde_json::Value, key: &str) -> Option<f64> {
    settings_json.get(key).and_then(|v| v.as_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_range_filters_non_finite() {
        let (min, max) = temp_range(&[]);
        assert!(min.is_nan() && max.is_nan(), "空矩阵应返回 NaN");
        let (min, max) = temp_range(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);
        assert!(min.is_nan() && max.is_nan(), "全非有限值应返回 NaN");
        assert_eq!(
            temp_range(&[1.0, 5.0, 3.0, f32::NAN, f32::INFINITY]),
            (1.0, 5.0)
        );
    }

    #[test]
    fn num_to_str_never_uses_exponent() {
        for v in [25.0, 0.95, 1e-9, 1e20, -12.5, 0.0] {
            let s = num_to_str_pub(v);
            assert!(!s.contains('e') && !s.contains('E'), "{v} -> {s}");
            assert!(s.parse::<f64>().is_ok(), "{s} 应可解析回数值");
        }
        assert_eq!(num_to_str_pub(0.95), "0.95");
    }

    #[test]
    fn settings_f64_reads_numbers_only() {
        let v = serde_json::json!({ "Emissivity": 0.95, "Label": "x" });
        assert_eq!(settings_f64(&v, "Emissivity"), Some(0.95));
        assert_eq!(settings_f64(&v, "Label"), None);
        assert_eq!(settings_f64(&v, "Missing"), None);
    }

    #[test]
    fn with_emissivity_validates_range_and_writes_field() {
        let s = crate::dji::build_settings(&crate::dji::DjiParams::default())
            .expect("构造标定参数");
        assert!(with_emissivity(&s, 0.049).is_err(), "0.05 为下界");
        assert!(with_emissivity(&s, 1.01).is_err(), "1.0 为上界");
        for e in [0.05, 0.5, 1.0] {
            let changed = with_emissivity(&s, e).expect("边界内发射率应合法");
            let json = serde_json::to_value(&changed).expect("序列化标定参数");
            assert!((settings_f64(&json, "Emissivity").unwrap() - e).abs() < 1e-9);
        }
    }

    #[test]
    fn compute_temps_produces_finite_matrix() {
        let s = crate::dji::build_settings(&crate::dji::DjiParams::default())
            .expect("构造标定参数");
        let temps = compute_temps(&[19388.0; 16], &s, DEFAULT_DISTANCE);
        assert_eq!(temps.len(), 16);
        assert!(temps.iter().all(|t| t.is_finite()));
    }
}
