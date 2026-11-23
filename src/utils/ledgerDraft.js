// 巡检台账入库草稿：分析页单条入库与批量入库共用，保持纯函数便于测试。

function round1(v) {
  return Number.isFinite(v) ? Math.round(v * 10) / 10 : 0
}

/**
 * 分析页入库草稿：优先使用选中 ROI 的统计/定级，缺失时回退到全图极值。
 */
export function buildLedgerDraft({ meta = null, roi = null, extrema = null } = {}) {
  const stats = roi?.stats || null
  const max = stats?.max ?? extrema?.max ?? 0
  const delta = stats?.delta_t ?? (extrema ? extrema.max - extrema.min : 0)
  return {
    tower_no: '',
    equipment: '',
    phase: roi?.phase || '',
    image_path: meta?.path || '',
    max_temp: round1(max),
    delta_t: round1(delta),
    level: roi?.verdict?.level ?? 0,
    note: ''
  }
}

/**
 * 批量解析结果转台账输入：只取解析成功项，按需仅保留超限项（level >= 1）。
 */
export function buildBatchLedgerInputs({
  items = [],
  towerNo = '',
  equipment = '',
  note = '',
  onlyOverLimit = true
} = {}) {
  return items
    .filter((it) => !it.error && (!onlyOverLimit || it.level >= 1))
    .map((it) => ({
      tower_no: towerNo,
      equipment,
      phase: '',
      image_path: it.path,
      max_temp: round1(it.max),
      delta_t: round1(it.delta_t),
      level: it.level,
      note: note ? `${it.file_name} ${note}` : it.file_name
    }))
}
