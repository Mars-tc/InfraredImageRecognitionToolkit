//! 缺陷阈值配置（F12）：JSON 持久化到应用配置目录。
//!
//! 阈值在 AppState 内存持有，修改时同步落盘；启动时读取，缺失/损坏则用默认值。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 缺陷判定阈值（F12 可配置）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Thresholds {
    /// 接头/连接部位温度阈值（℃），超过即缺陷
    pub joint_temp: f64,
    /// 相对温差阈值 ΔT（℃）
    pub delta_t: f64,
    /// 三相间同一部位最大温差阈值（℃，F11）
    pub phase_delta_t: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            joint_temp: 70.0,
            delta_t: 10.0,
            phase_delta_t: 5.0,
        }
    }
}

impl Thresholds {
    fn sanitize(&mut self) {
        self.joint_temp = self.joint_temp.clamp(0.0, 500.0);
        self.delta_t = self.delta_t.clamp(0.0, 200.0);
        self.phase_delta_t = self.phase_delta_t.clamp(0.0, 200.0);
    }
}

/// 配置文件路径：app_config_dir/thresholds.json
pub fn config_path(config: &tauri::Config) -> Option<PathBuf> {
    let dir = tauri::api::path::app_config_dir(config)?;
    Some(dir.join("thresholds.json"))
}

/// 从磁盘加载（不存在/解析失败 → 默认值）
pub fn load(path: &Path) -> Thresholds {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut t: Thresholds = serde_json::from_str(&text).unwrap_or_default();
            t.sanitize();
            t
        }
        Err(_) => Thresholds::default(),
    }
}

/// 保存到磁盘（父目录不存在则创建）
pub fn save(path: &Path, t: &Thresholds) -> Result<(), String> {
    let mut clean = t.clone();
    clean.sanitize();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let text = serde_json::to_string_pretty(&clean).map_err(|e| format!("序列化失败：{e}"))?;
    std::fs::write(path, text).map_err(|e| format!("写入配置失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let t = Thresholds::default();
        assert_eq!(t.joint_temp, 70.0);
        assert_eq!(t.delta_t, 10.0);
        assert_eq!(t.phase_delta_t, 5.0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("ir_test_settings");
        let path = dir.join("thresholds.json");
        let _ = std::fs::remove_file(&path);
        let mut t = Thresholds::default();
        t.joint_temp = 65.5;
        save(&path, &t).expect("save");
        let loaded = load(&path);
        assert_eq!(loaded.joint_temp, 65.5);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_corrupt_falls_back_to_default() {
        let dir = std::env::temp_dir().join("ir_test_settings");
        let path = dir.join("bad.json");
        std::fs::write(&path, "{not json").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.joint_temp, 70.0);
        let _ = std::fs::remove_file(&path);
    }
}
