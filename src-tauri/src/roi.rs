//! ROI 统计（F5 / F10）：矩形框选、多边形、线段的 max/min/avg/ΔT。
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

/// 对温度矩阵做 ROI 统计
pub fn roi_stats(
    temps: &[f32],
    width: usize,
    height: usize,
    shape: &RoiShape,
) -> AppResult<RoiStats> {
    let mut acc = Acc::default();
    match shape {
        RoiShape::Rect { x, y, w, h } => {
            let (x0, x1) = ordered(*x, *x + *w);
            let (y0, y1) = ordered(*y, *y + *h);
            let ix0 = x0.max(0.0).floor() as usize;
            let ix1 = (x1.min(width as f64).ceil() as usize).min(width);
            let iy0 = y0.max(0.0).floor() as usize;
            let iy1 = (y1.min(height as f64).ceil() as usize).min(height);
            for row in iy0..iy1 {
                for col in ix0..ix1 {
                    acc.push(temps[row * width + col]);
                }
            }
        }
        RoiShape::Poly { points } => {
            if points.len() < 3 {
                return Err("多边形至少需要 3 个点".to_string());
            }
            for row in 0..height {
                for col in 0..width {
                    if point_in_polygon(col as f64 + 0.5, row as f64 + 0.5, points) {
                        acc.push(temps[row * width + col]);
                    }
                }
            }
        }
        RoiShape::Line { points } => {
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
                        acc.push(temps[row as usize * width + col as usize]);
                    }
                    x0 = px;
                    y0 = py;
                }
            }
        }
    }
    acc.finish()
}

#[derive(Default)]
struct Acc {
    max: f64,
    min: f64,
    sum: f64,
    count: usize,
}

impl Acc {
    fn push(&mut self, t: f32) {
        if !t.is_finite() {
            return;
        }
        let v = t as f64;
        if self.count == 0 {
            self.max = v;
            self.min = v;
        } else {
            if v > self.max {
                self.max = v;
            }
            if v < self.min {
                self.min = v;
            }
        }
        self.sum += v;
        self.count += 1;
    }

    fn finish(self) -> AppResult<RoiStats> {
        if self.count == 0 {
            return Err("ROI 区域内没有有效像素".to_string());
        }
        Ok(RoiStats {
            max: self.max,
            min: self.min,
            avg: self.sum / self.count as f64,
            delta_t: self.max - self.min,
            count: self.count,
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
}
