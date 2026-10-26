// 展示格式化工具

/** 温度：28.96 ℃；无效值显示 -- */
export function fmtTemp(t, digits = 1) {
  if (t === null || t === undefined || Number.isNaN(t)) return '--'
  return `${Number(t).toFixed(digits)} ℃`
}

/** 温差：+4.2 K */
export function fmtDelta(d) {
  if (d === null || d === undefined || Number.isNaN(d)) return '--'
  const sign = d > 0 ? '+' : ''
  return `${sign}${Number(d).toFixed(1)} K`
}

/** GPS 十进制坐标 */
export function fmtGps(g) {
  return g || '--'
}

/** 发射率：0.95 */
export function fmtEmissivity(e) {
  if (e === null || e === undefined || Number.isNaN(e)) return '--'
  return Number(e).toFixed(2)
}

/** 湿度：50 % */
export function fmtHumidity(h) {
  if (h === null || h === undefined || Number.isNaN(h)) return '--'
  return `${Number(h).toFixed(0)} %`
}

/** 距离：5.0 m */
export function fmtDistance(d) {
  if (d === null || d === undefined || Number.isNaN(d)) return '--'
  return `${Number(d).toFixed(1)} m`
}
