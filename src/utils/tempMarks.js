// 温度极值标注组装：画布覆盖层与 F8 导出共用同一份结果，保证所见即所得。
import { fmtTemp } from './format.js'

export const HOT_COLOR = '#FF6B3D' // 最高温（暖色）
export const COLD_COLOR = '#40A9FF' // 最低温（冷色）

function hexToRgb(hex) {
  const h = String(hex || '').replace('#', '')
  const full = h.length === 3 ? h.split('').map((c) => c + c).join('') : h
  const n = parseInt(full, 16)
  if (Number.isNaN(n)) return [255, 255, 255]
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

function asciiTemp(t) {
  return `${Number.isFinite(t) ? t.toFixed(1) : '--'}C`
}

function makeMark({ x, y, temp, label, ascii, scope, hot }) {
  const color = hot ? HOT_COLOR : COLD_COLOR
  return { x, y, temp, label, ascii, scope, hot, color, rgb: hexToRgb(color) }
}

/**
 * 产出最多 4 个标注：全图最高/最低 + 当前选中 ROI 的最高/最低。
 * 坐标是图像像素索引（[列, 行]），画布绘制时自行 +0.5 对齐像素中心。
 * 任一侧数据缺失时只跳过对应项，其余照常显示。
 */
export function buildTempMarks({ extrema = null, rois = [], selectedId = null } = {}) {
  const marks = []

  if (extrema && Array.isArray(extrema.max_pos) && Array.isArray(extrema.min_pos)) {
    marks.push(makeMark({
      x: extrema.max_pos[0], y: extrema.max_pos[1], temp: extrema.max,
      label: `全图最高 ${fmtTemp(extrema.max)}`,
      ascii: `IMG MAX ${asciiTemp(extrema.max)}`,
      scope: 'image', hot: true
    }))
    marks.push(makeMark({
      x: extrema.min_pos[0], y: extrema.min_pos[1], temp: extrema.min,
      label: `全图最低 ${fmtTemp(extrema.min)}`,
      ascii: `IMG MIN ${asciiTemp(extrema.min)}`,
      scope: 'image', hot: false
    }))
  }

  const selected = rois.find((r) => r.id === selectedId)
  const stats = selected?.stats
  if (stats && Array.isArray(stats.max_pos) && Array.isArray(stats.min_pos)) {
    const name = selected.label || 'ROI'
    const prefix = String(name).toUpperCase()
    marks.push(makeMark({
      x: stats.max_pos[0], y: stats.max_pos[1], temp: stats.max,
      label: `${name} 最高 ${fmtTemp(stats.max)}`,
      ascii: `${prefix} MAX ${asciiTemp(stats.max)}`,
      scope: 'roi', hot: true
    }))
    marks.push(makeMark({
      x: stats.min_pos[0], y: stats.min_pos[1], temp: stats.min,
      label: `${name} 最低 ${fmtTemp(stats.min)}`,
      ascii: `${prefix} MIN ${asciiTemp(stats.min)}`,
      scope: 'roi', hot: false
    }))
  }

  return marks
}
