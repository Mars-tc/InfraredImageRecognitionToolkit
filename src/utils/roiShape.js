// Fabric ROI 对象 <-> 后端 roi_stats 使用的场景（图像像素）形状
import { Rect, Polygon, Point } from 'fabric'

/**
 * 把 Fabric 对象换算回后端需要的形状。
 *
 * Polygon/Polyline 渲染时绘制的是 (point - pathOffset)，再套对象变换矩阵；
 * 拖动只改 left/top、缩放只改 scaleX/scaleY，points 本身始终不变。直接把
 * points 送给后端会得到移动前的旧坐标，右侧统计自然不会更新。这里复用同一
 * 矩阵把局部点变换回场景坐标，保证统计区域与画布显示完全一致。
 * 框选多个 ROI 时对象会临时进入 ActiveSelection，left/top 变成相对选择框
 * 的局部坐标；calcTransformMatrix() 已包含父组变换，故多边形/线段天然正确，
 * 矩形则需用 getXY()/getObjectScaling() 取场景绝对位置与总缩放。
 * kind 取自 data，避免依赖 Fabric 的 type 字符串。
 */
export function shapeOf(obj) {
  const kind = obj.data?.roiKind || (obj instanceof Rect ? 'rect' : obj instanceof Polygon ? 'poly' : 'line')
  if (kind === 'rect') {
    const p = obj.getXY()
    const s = obj.getObjectScaling()
    return { kind: 'rect', x: p.x, y: p.y, w: obj.width * s.x, h: obj.height * s.y }
  }
  const matrix = obj.calcTransformMatrix()
  const offset = obj.pathOffset || { x: 0, y: 0 }
  const points = (obj.points || []).map((p) => {
    const v = new Point(p.x - offset.x, p.y - offset.y).transform(matrix)
    return [v.x, v.y]
  })
  return { kind, points }
}

/**
 * 把变换事件的目标展开成需要同步的 ROI 对象。
 * 框选时事件目标是 ActiveSelection（自身没有 data.roiId），必须遍历其子对象，
 * 否则整批 ROI 的形状与统计都不会更新。
 */
export function roiTargetsOf(target) {
  if (!target) return []
  const objects = typeof target.getObjects === 'function' ? target.getObjects() : [target]
  return objects.filter((o) => o.data?.roiId)
}

/** ROI 颜色（#RRGGBB / #RGB）转半透明填充色 */
export function hexToRgba(hex, alpha) {
  const h = String(hex || '').replace('#', '')
  const full = h.length === 3 ? h.split('').map((c) => c + c).join('') : h
  const n = parseInt(full, 16)
  if (Number.isNaN(n)) return `rgba(0, 166, 80, ${alpha})`
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`
}
