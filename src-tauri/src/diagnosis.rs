//! 缺陷定级（F11/F12）：四级判定 + 三相 ΔT 比对。
//!
//! 等级（weight 由低到高）：0 其他 / 1 一般 / 2 重大 / 3 紧急。
//! 判定规则（依据 DL/T 664 温升定级思路简化，阈值可配置）：
//! - 其他：max < joint_temp 且 ΔT < delta_t
//! - 一般：max ≥ joint_temp 或 ΔT ≥ delta_t
//! - 重大：max ≥ joint_temp + 15 或 ΔT ≥ 2×delta_t
//! - 紧急：max ≥ joint_temp + 30 或 ΔT ≥ 3×delta_t

use serde::Serialize;

use crate::roi::RoiStats;
use crate::settings::Thresholds;

/// 缺陷等级常量（前端 defects.js 保持一致）
pub const LEVEL_OTHER: u8 = 0;
pub const LEVEL_MINOR: u8 = 1;
pub const LEVEL_MAJOR: u8 = 2;
pub const LEVEL_CRITICAL: u8 = 3;

/// 单个 ROI 的判定结果
#[derive(Debug, Serialize, Clone)]
pub struct Verdict {
    /// 0 其他 / 1 一般 / 2 重大 / 3 紧急
    pub level: u8,
    /// 判定说明（面向用户）
    pub reason: String,
    /// 是否超过任一阈值（一般及以上）
    pub exceeded: bool,
}

/// 按阈值给 ROI 统计结果定级
pub fn verdict(stats: &RoiStats, t: &Thresholds) -> Verdict {
    let max = stats.max;
    let dt = stats.delta_t;
    let (level, reason) = if max >= t.joint_temp + 30.0 || dt >= t.delta_t * 3.0 {
        (
            LEVEL_CRITICAL,
            format!("最高温 {max:.1}℃ 或温差 {dt:.1}℃ 达紧急档"),
        )
    } else if max >= t.joint_temp + 15.0 || dt >= t.delta_t * 2.0 {
        (
            LEVEL_MAJOR,
            format!("最高温 {max:.1}℃ 或温差 {dt:.1}℃ 达重大档"),
        )
    } else if max >= t.joint_temp || dt >= t.delta_t {
        (
            LEVEL_MINOR,
            format!("最高温 {max:.1}℃ 超阈值或温差 {dt:.1}℃ 超限"),
        )
    } else {
        (LEVEL_OTHER, "各项指标正常".to_string())
    };
    Verdict {
        level,
        reason,
        exceeded: level >= LEVEL_MINOR,
    }
}

/// 三相同部位比对（F11）：A/B/C 相 ROI 的最高温两两差超过 phase_delta_t 即告警。
/// `entries` 为 (相别, 最高温) 列表，只比较 A/B/C 三相。
pub fn phase_compare(entries: &[(String, f64)], t: &Thresholds) -> Option<Verdict> {
    let mut phases: Vec<(char, f64)> = entries
        .iter()
        .filter_map(|(phase, max)| {
            let mut it = phase.chars();
            let c = it.next()?;
            if "ABC".contains(c) {
                Some((c, *max))
            } else {
                None
            }
        })
        .collect();
    // 同相取最高
    phases.sort_by(|a, b| a.0.cmp(&b.0));
    phases.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1 = b.1.max(a.1);
            true
        } else {
            false
        }
    });
    if phases.len() < 2 {
        return None;
    }
    let max = phases.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    let min = phases.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let delta = max - min;
    if delta >= t.phase_delta_t {
        let names: Vec<String> = phases.iter().map(|(c, _)| c.to_string()).collect();
        Some(Verdict {
            level: if delta >= t.phase_delta_t * 2.0 {
                LEVEL_MAJOR
            } else {
                LEVEL_MINOR
            },
            reason: format!(
                "{} 相间最大温差 {delta:.1}℃ 超过阈值 {}℃",
                names.join("/"),
                t.phase_delta_t
            ),
            exceeded: true,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(max: f64, min: f64) -> RoiStats {
        RoiStats {
            max,
            min,
            avg: (max + min) / 2.0,
            delta_t: max - min,
            count: 100,
        }
    }

    #[test]
    fn levels() {
        let t = Thresholds::default();
        assert_eq!(verdict(&stats(30.0, 28.0), &t).level, LEVEL_OTHER);
        assert_eq!(verdict(&stats(75.0, 70.0), &t).level, LEVEL_MINOR);
        assert_eq!(verdict(&stats(90.0, 70.0), &t).level, LEVEL_MAJOR);
        assert_eq!(verdict(&stats(110.0, 70.0), &t).level, LEVEL_CRITICAL);
        // 温差路径（默认阈值 ΔT=10：≥10 一般 / ≥20 重大 / ≥30 紧急）
        assert_eq!(verdict(&stats(50.0, 35.0), &t).level, LEVEL_MINOR);
        assert_eq!(verdict(&stats(50.0, 30.0), &t).level, LEVEL_MAJOR);
        assert_eq!(verdict(&stats(50.0, 20.0), &t).level, LEVEL_CRITICAL);
        assert_eq!(verdict(&stats(50.0, 10.0), &t).level, LEVEL_CRITICAL);
    }

    #[test]
    fn phase_compare_works() {
        let t = Thresholds::default();
        assert!(phase_compare(&[], &t).is_none());
        assert!(phase_compare(&[("A".into(), 50.0)], &t).is_none());
        let v = phase_compare(
            &[
                ("A".into(), 50.0),
                ("B".into(), 58.0),
                ("C".into(), 51.0),
            ],
            &t,
        )
        .expect("应告警");
        assert_eq!(v.level, LEVEL_MINOR);
        assert!(v.reason.contains("A/B/C"));
        // 相差小于阈值不告警
        assert!(phase_compare(
            &[
                ("A".into(), 50.0),
                ("B".into(), 52.0),
                ("C".into(), 51.0),
            ],
            &t,
        )
        .is_none());
    }

    #[test]
    fn thresholds_are_inclusive_and_configurable() {
        let t = Thresholds::default();
        // 温差路径：阈值边界取等号即触发，略低于阈值不触发
        assert_eq!(verdict(&stats(50.0, 40.1), &t).level, LEVEL_OTHER);
        assert_eq!(verdict(&stats(50.0, 40.0), &t).level, LEVEL_MINOR);
        // 温度路径与温差解耦（max 单独决定档位）
        assert_eq!(verdict(&stats(69.9, 65.0), &t).level, LEVEL_OTHER);
        assert_eq!(verdict(&stats(70.0, 65.0), &t).level, LEVEL_MINOR);
        assert_eq!(verdict(&stats(85.0, 80.0), &t).level, LEVEL_MAJOR);
        assert_eq!(verdict(&stats(100.0, 95.0), &t).level, LEVEL_CRITICAL);

        // 自定义阈值：倍数关系随配置变化
        let custom = Thresholds {
            joint_temp: 100.0,
            delta_t: 20.0,
            phase_delta_t: 5.0,
        };
        assert_eq!(verdict(&stats(90.0, 80.0), &custom).level, LEVEL_OTHER);
        assert_eq!(verdict(&stats(100.0, 95.0), &custom).level, LEVEL_MINOR);
        assert_eq!(verdict(&stats(90.0, 50.0), &custom).level, LEVEL_MAJOR);
        assert_eq!(verdict(&stats(90.0, 30.0), &custom).level, LEVEL_CRITICAL);
    }

    #[test]
    fn phase_compare_ignores_invalid_phases() {
        let t = Thresholds::default();
        assert!(phase_compare(&[("N".into(), 50.0), ("X".into(), 90.0)], &t).is_none());
        let v = phase_compare(
            &[("A".into(), 50.0), ("A".into(), 90.0), ("B".into(), 51.0)],
            &t,
        )
        .expect("同相取最大值后仍应比对");
        // 相间告警最高只到「重大」（2 倍阈值即封顶），同相重复值取最大
        assert_eq!(v.level, LEVEL_MAJOR);
    }
}
