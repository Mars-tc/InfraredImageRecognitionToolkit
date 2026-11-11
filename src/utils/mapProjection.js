// 离线点位地图投影：零依赖纯函数，便于单测与复用。
// 采用等距圆柱投影，并按中心纬度做 cos 修正，减轻高纬度横向形变。

const MIN_SPAN = 1e-6
const MIN_SCALE = 0.4
const MAX_SCALE = 40

/** 有效坐标：数值有限、在经纬度量程内、排除 (0,0) 无定位占位值 */
export function isValidCoord(lat, lon) {
  return (
    Number.isFinite(lat) &&
    Number.isFinite(lon) &&
    lat >= -90 && lat <= 90 &&
    lon >= -180 && lon <= 180 &&
    !(Math.abs(lat) < 1e-6 && Math.abs(lon) < 1e-6)
  )
}

/** 批量结果 → 可投影点位；无坐标项由调用方统计提示 */
export function toPoints(items = []) {
  const list = Array.isArray(items) ? items : []
  return list
    .filter((it) => it && isValidCoord(it.lat, it.lon))
    .map((it, i) => ({
      id: it.path || String(i),
      lat: Number(it.lat),
      lon: Number(it.lon),
      item: it
    }))
}

/** 点位经纬度范围；无有效点时返回 null */
export function computeBounds(points = []) {
  const valid = points.filter((p) => isValidCoord(p.lat, p.lon))
  if (!valid.length) return null
  let minLat = Infinity, maxLat = -Infinity, minLon = Infinity, maxLon = -Infinity
  for (const p of valid) {
    minLat = Math.min(minLat, p.lat)
    maxLat = Math.max(maxLat, p.lat)
    minLon = Math.min(minLon, p.lon)
    maxLon = Math.max(maxLon, p.lon)
  }
  return { minLat, maxLat, minLon, maxLon, centerLat: (minLat + maxLat) / 2 }
}

/** 生成投影参数：等比缩放到画布可用区并居中；单点 / 同点退化时仍居中 */
export function createProjection(bounds, width, height, padding = 48) {
  if (!bounds) return null
  const pad = Math.max(0, Math.min(padding, Math.min(width, height) / 2 - 1))
  const availW = Math.max(1, width - pad * 2)
  const availH = Math.max(1, height - pad * 2)
  const cosLat = Math.max(0.1, Math.cos((bounds.centerLat * Math.PI) / 180))
  // 真实跨度用于居中；钳制后的跨度只用于算 scale，避免单点时除零
  const realXSpan = Math.max(0, (bounds.maxLon - bounds.minLon) * cosLat)
  const realYSpan = Math.max(0, bounds.maxLat - bounds.minLat)
  const xSpan = Math.max(MIN_SPAN, realXSpan)
  const ySpan = Math.max(MIN_SPAN, realYSpan)
  const scale = Math.min(availW / xSpan, availH / ySpan)
  return {
    bounds, cosLat, scale,
    originX: width / 2 - (realXSpan * scale) / 2,
    originY: height / 2 - (realYSpan * scale) / 2,
    width, height, padding: pad
  }
}

/** 单个经纬度 → 画布坐标（y 轴向下，北在上） */
export function projectPoint(proj, lat, lon) {
  if (!proj) return { x: 0, y: 0 }
  return {
    x: proj.originX + (lon - proj.bounds.minLon) * proj.cosLat * proj.scale,
    y: proj.originY + (proj.bounds.maxLat - lat) * proj.scale
  }
}

/** 一次性投影全部点位；缩放平移只改 SVG transform，不重算 */
export function projectPoints(points, bounds, width, height, padding = 48) {
  const proj = createProjection(bounds, width, height, padding)
  return points.map((p) => ({ ...p, ...projectPoint(proj, p.lat, p.lon) }))
}

/** 适应视图复位 */
export function fitTransform() {
  return { scale: 1, tx: 0, ty: 0 }
}

/** 以指针位置为锚缩放，保持指针下的点不动 */
export function zoomAt(transform, factor, pointerX, pointerY) {
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, transform.scale * factor))
  const ratio = scale / transform.scale
  return {
    scale,
    tx: pointerX - (pointerX - transform.tx) * ratio,
    ty: pointerY - (pointerY - transform.ty) * ratio
  }
}

/** 经纬网格刻度值（退化范围时返回空数组，由组件只画边框） */
export function gridTicks(bounds, count = 5) {
  if (!bounds) return { lats: [], lons: [] }
  const n = Math.max(2, Math.floor(count))
  const steps = Array.from({ length: n }, (_, i) => i / (n - 1))
  if (bounds.maxLat - bounds.minLat < MIN_SPAN || bounds.maxLon - bounds.minLon < MIN_SPAN) {
    return { lats: [], lons: [] }
  }
  return {
    lats: steps.map((t) => bounds.maxLat - (bounds.maxLat - bounds.minLat) * t),
    lons: steps.map((t) => bounds.minLon + (bounds.maxLon - bounds.minLon) * t)
  }
}

/** 刻度标签：21.88696°N / 111.75551°E */
export function formatCoord(value, axis = 'lat', digits = 5) {
  if (!Number.isFinite(value)) return '--'
  const hemi = axis === 'lat' ? (value >= 0 ? 'N' : 'S') : (value >= 0 ? 'E' : 'W')
  return `${Math.abs(value).toFixed(digits)}°${hemi}`
}
