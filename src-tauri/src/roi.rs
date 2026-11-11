//! ROI 统计（F5 / F10）与极值定位：矩形、多边形、线段及整图的 max/min/avg/ΔT 与像素坐标。
//!
//! 按文档架构原则，矩阵计算在 Rust 后端完成；前端只负责绘制 ROI 形状
//! 并把形状参数通过 IPC 传进来。

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// ROI 形状（前端 Fabric 对象序列化而来，坐标为图像像素坐标）
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RoiShape {
    /// 矩形（x, y 为左上角，w/h 为尺寸，允许负数外扩由裁剪处理）
    Rect { x: f64, y: f64, w: f64, h: f64 },
    /// 多边形（点列表，按顺序相连闭合）
    Poly { points: Vec<(f64, f64)> },
    /// 线段 / 折线（点列表，沿路径 1 像素宽取样）
    Line { points: Vec<(f64, f64)> },
}

/// ROI 统计结果（℃）
#[derive(Debug, Serialize, Clone)]
pub struct RoiStats {
    pub max: f64,
    pub min: f64,
    pub avg: f64,
    /// 温差 ΔT（max - min）
    pub delta_t: f64,
    /// 参与统计的像素数
    pub count: usize,
}

/// 极值及其在图像中的位置（[列, 行] 像素坐标）
#[derive(Debug, Serialize, Clone)]
pub struct Extrema {
    pub max: f64,
    pub min: f64,
    pub max_pos: [usize; 2],
    pub min_pos: [usize; 2],
}

/// 一次扫描的完整结果：统计数值 + 极值坐标
#[derive(Debug, Clone)]
pub struct Scan {
    pub stats: RoiStats,
    pub extrema: Extrema,
}

/// 扫描温度矩阵：shape 为 None 表示整张图，否则只统计 ROI 覆盖的像素。
/// 数值与极值坐标在同一次遍历中产出，保证标注温度与统计面板完全同源。
pub fn scan(
    temps: &[f32],
    width: usize,
    height: usize,
    shape: Option<&RoiShape>,
) -> AppResult<Scan> {
    let mut acc = Acc::default();
    match shape {
        None => {
            for row in 0..height {
                for col in 0..width {
                    acc.push(temps[row * width + col], col, row);
                }
            }
        }
        Some(RoiShape::Rect { x, y, w, h }) => {
            let (x0, x1) = ordered(*x, *x + *w);
            let (y0, y1) = ordered(*y, *y + *h);
            let ix0 = x0.max(0.0).floor() as usize;
            let ix1 = (x1.min(width as f64).ceil() as usize).min(width);
            let iy0 = y0.max(0.0).floor() as usize;
            let iy1 = (y1.min(height as f64).ceil() as usize).min(height);
            for row in iy0..iy1 {
                for col in ix0..ix1 {
                    acc.push(temps[row * width + col], col, row);
                }
            }
        }
        Some(RoiShape::Poly { points }) => {
            if points.len() < 3 {
                return Err("多边形至少需要 3 个点".to_string());
            }
            for row in 0..height {
                for col in 0..width {
                    if point_in_polygon(col as f64 + 0.5, row as f64 + 0.5, points) {
                        acc.push(temps[row * width + col], col, row);
                    }
                }
            }
        }
        Some(RoiShape::Line { points }) => {
            if points.len() < 2 {
                return Err("线段至少需要 2 个点".to_string());
            }
            for pair in points.windows(2) {
                let (mut x0, mut y0) = pair[0];
                let (x1, y1) = pair[1];
                let steps = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0).ceil() as usize;
                let steps = steps.max(1);
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let px = x0 + (x1 - x0) * t;
                    let py = y0 + (y1 - y0) * t;
                    let col = px.floor() as isize;
                    let row = py.floor() as isize;
                    if col >= 0 && row >= 0 && (col as usize) < width && (row as usize) < height {
                        acc.push(
                            temps[row as usize * width + col as usize],
                            col as usize,
                            row as usize,
                        );
                    }
                    x0 = px;
                    y0 = py;
                }
            }
        }
    }
    acc.finish()
}

/// 对温度矩阵做 ROI 统计（保持原签名，供 batch / diagnosis 复用）
pub fn roi_stats(
    temps: &[f32],
    width: usize,
    height: usize,
    shape: &RoiShape,
) -> AppResult<RoiStats> {
    scan(temps, width, height, Some(shape)).map(|s| s.stats)
}

/// 整张图的最高温 / 最低温及其像素坐标
pub fn image_extrema(temps: &[f32], width: usize, height: usize) -> AppResult<Extrema> {
    scan(temps, width, height, None).map(|s| s.extrema)
}

#[derive(Default)]
struct Acc {
    max: f64,
    min: f64,
    sum: f64,
    count: usize,
    max_pos: [usize; 2],
    min_pos: [usize; 2],
}

impl Acc {
    fn push(&mut self, t: f32, col: usize, row: usize) {
        if !t.is_finite() {
            return;
        }
        let v = t as f64;
        if self.count == 0 {
            self.max = v;
            self.min = v;
            self.max_pos = [col, row];
            self.min_pos = [col, row];
        } else {
            // 严格大于/小于：并列极值时保留遍历中第一个命中的像素
            if v > self.max {
                self.max = v;
                self.max_pos = [col, row];
            }
            if v < self.min {
                self.min = v;
                self.min_pos = [col, row];
            }
        }
        self.sum += v;
        self.count += 1;
    }

    fn finish(self) -> AppResult<Scan> {
        if self.count == 0 {
            return Err("ROI 区域内没有有效像素".to_string());
        }
        Ok(Scan {
            stats: RoiStats {
                max: self.max,
                min: self.min,
                avg: self.sum / self.count as f64,
                delta_t: self.max - self.min,
                count: self.count,
            },
            extrema: Extrema {
                max: self.max,
                min: self.min,
                max_pos: self.max_pos,
                min_pos: self.min_pos,
            },
        })
    }
}

fn ordered(a: f64, b: f64) -> (f64, f64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// 射线法判断点是否在多边形内
fn point_in_polygon(x: f64, y: f64, points: &[(f64, f64)]) -> bool {
    let n = points.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        let mut v = Vec::with_capacity(w * h);
        for r in 0..h {
            for c in 0..w {
                v.push(f(c, r));
            }
        }
        v
    }

    #[test]
    fn rect_stats() {
        let temps = matrix(10, 10, |c, r| (c + r) as f32);
        let s = roi_stats(&temps, 10, 10, &RoiShape::Rect { x: 0.0, y: 0.0, w: 3.0, h: 3.0 }).unwrap();
        assert_eq!(s.count, 9);
        assert_eq!(s.min, 0.0);
        assert_eq!(s.max, 4.0);
        assert!((s.avg - 2.0).abs() < 1e-6);
        assert!((s.delta_t - 4.0).abs() < 1e-6);
    }

    #[test]
    fn poly_stats() {
        let temps = matrix(10, 10, |_, r| r as f32);
        // 覆盖 0~5 行的多边形
        let s = roi_stats(
            &temps,
            10,
            10,
            &RoiShape::Poly {
                points: vec![(0.0, 0.0), (9.0, 0.0), (9.0, 5.9), (0.0, 5.9)],
            },
        )
        .unwrap();
        assert!(s.count >= 50 && s.count <= 60, "count={}", s.count);
        assert_eq!(s.min, 0.0);
        assert!(s.max >= 5.0 && s.max <= 6.0);
    }

    #[test]
    fn line_stats() {
        let temps = matrix(10, 10, |c, _| c as f32);
        let s = roi_stats(
            &temps,
            10,
            10,
            &RoiShape::Line {
                points: vec![(0.0, 5.0), (9.0, 5.0)],
            },
        )
        .unwrap();
        assert!(s.count >= 10);
        assert_eq!(s.min, 0.0);
        assert!(s.max >= 8.0);
    }

    /// 极值坐标处的温度必须与统计值一致（标注点与面板数值同源）
    fn assert_pos_matches(temps: &[f32], width: usize, e: &Extrema) {
        assert_eq!(temps[e.max_pos[1] * width + e.max_pos[0]] as f64, e.max);
        assert_eq!(temps[e.min_pos[1] * width + e.min_pos[0]] as f64, e.min);
    }

    #[test]
    fn rect_extrema_positions() {
        let temps = matrix(10, 10, |c, r| (c + r) as f32);
        let shape = RoiShape::Rect { x: 2.0, y: 3.0, w: 3.0, h: 3.0 };
        let s = scan(&temps, 10, 10, Some(&shape)).unwrap();
        assert_eq!(s.extrema.min_pos, [2, 3]);
        assert_eq!(s.extrema.max_pos, [4, 5]);
        assert_pos_matches(&temps, 10, &s.extrema);
        // roi_stats 保持原签名且数值与 scan 一致
        let legacy = roi_stats(&temps, 10, 10, &shape).unwrap();
        assert_eq!(legacy.max, s.stats.max);
        assert_eq!(legacy.count, s.stats.count);
    }

    #[test]
    fn poly_and_line_extrema_positions() {
        let temps = matrix(10, 10, |c, r| (c as f32) * 0.5 + r as f32);
        let poly = RoiShape::Poly {
            points: vec![(0.0, 0.0), (9.0, 0.0), (9.0, 5.9), (0.0, 5.9)],
        };
        let line = RoiShape::Line { points: vec![(1.0, 2.0), (8.0, 6.0)] };
        for shape in [poly, line] {
            let e = scan(&temps, 10, 10, Some(&shape)).unwrap().extrema;
            assert_pos_matches(&temps, 10, &e);
            assert!(e.max_pos[0] < 10 && e.max_pos[1] < 10);
            assert!(e.min_pos[0] < 10 && e.min_pos[1] < 10);
        }
    }

    #[test]
    fn image_extrema_covers_whole_matrix() {
        let temps = matrix(10, 10, |c, r| (c + r) as f32);
        let e = image_extrema(&temps, 10, 10).unwrap();
        assert_eq!(e.min, 0.0);
        assert_eq!(e.max, 18.0);
        assert_eq!(e.min_pos, [0, 0]);
        assert_eq!(e.max_pos, [9, 9]);
        assert_pos_matches(&temps, 10, &e);
    }

    #[test]
    fn tied_extrema_keep_first_pixel() {
        let temps = matrix(4, 3, |_, _| 25.0);
        let e = image_extrema(&temps, 4, 3).unwrap();
        assert_eq!(e.max_pos, [0, 0]);
        assert_eq!(e.min_pos, [0, 0]);
    }

    #[test]
    fn empty_region_reports_error() {
        let temps = matrix(10, 10, |_, _| 20.0);
        let shape = RoiShape::Rect { x: 20.0, y: 20.0, w: 5.0, h: 5.0 };
        assert!(scan(&temps, 10, 10, Some(&shape)).is_err());
        assert!(roi_stats(&temps, 10, 10, &shape).is_err());
    }
}
