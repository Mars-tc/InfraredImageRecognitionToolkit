import test, { beforeEach, after } from 'node:test'
import assert from 'node:assert/strict'
import { setActivePinia, createPinia } from 'pinia'
import { installMock } from '../scripts/fixtures/tauri-ipc-mock.js'

const mock = installMock()
const { useRoiStore, ROI_COLORS } = await import('../src/stores/roi.js')
const { useSettingsStore } = await import('../src/stores/settings.js')
const { useImageStore } = await import('../src/stores/image.js')
const { useLedgerStore } = await import('../src/stores/ledger.js')

const flush = () => new Promise((r) => setTimeout(r, 0))

beforeEach(() => {
  setActivePinia(createPinia())
  mock.reset()
})
after(() => mock.uninstall())

test('ROI：新增后回填统计与定级', async () => {
  mock.setHandler((msg) => {
    if (msg.cmd === 'roi_stats') {
      return {
        max: 75, min: 70, avg: 72.5, delta_t: 5, count: 10,
        verdict: { level: 1, reason: '超阈值', exceeded: true }
      }
    }
    return null
  })
  const store = useRoiStore()
  const roi = store.addRoi('rect', { kind: 'rect', x: 0, y: 0, w: 10, h: 10 })
  await flush()

  assert.equal(store.rois.length, 1)
  assert.equal(store.selectedId, roi.id)
  assert.equal(roi.label, 'ROI-1')
  assert.equal(roi.color, ROI_COLORS[0])
  assert.deepEqual(roi.stats, { max: 75, min: 70, avg: 72.5, delta_t: 5, count: 10 })
  assert.equal(roi.verdict.level, 1)
  assert.equal(roi.error, '')
  assert.equal(mock.calls[0].cmd, 'roi_stats')
  assert.deepEqual(mock.calls[0].shape, { kind: 'rect', x: 0, y: 0, w: 10, h: 10 })
})

test('ROI：统计失败时记录错误并清空旧结果', async () => {
  mock.setHandler(() => {
    throw 'ROI 不含有效像素'
  })
  const store = useRoiStore()
  const roi = store.addRoi('rect', { kind: 'rect' })
  await flush()
  assert.equal(roi.stats, null)
  assert.equal(roi.verdict, null)
  assert.equal(roi.error, 'ROI 不含有效像素')
})

test('ROI：相别标记触发三相比对，清空后复位', async () => {
  mock.setHandler((msg) => {
    if (msg.cmd === 'roi_stats') return { max: 50, min: 40, avg: 45, delta_t: 10, count: 5, verdict: null }
    if (msg.cmd === 'phase_compare') return { level: 1, reason: '相间温差超限', exceeded: true }
    return null
  })
  const store = useRoiStore()
  const a = store.addRoi('rect', { kind: 'rect' })
  const b = store.addRoi('rect', { kind: 'rect' })
  await flush()

  store.setPhase(a.id, 'A')
  store.setPhase(b.id, 'B')
  await flush()
  assert.equal(store.phaseAlert.level, 1)
  assert.equal(store.phaseAlert.exceeded, true)
  assert.ok(mock.calls.some((c) => c.cmd === 'phase_compare'))

  store.clearAll()
  assert.equal(store.rois.length, 0)
  assert.equal(store.selectedId, null)
  assert.equal(store.phaseAlert, null)
})

test('ROI：删除与形状更新', async () => {
  mock.setHandler((msg) =>
    msg.cmd === 'roi_stats' ? { max: 1, min: 0, avg: 0.5, delta_t: 1, count: 1, verdict: null } : null
  )
  const store = useRoiStore()
  const roi = store.addRoi('rect', { kind: 'rect' })
  await flush()
  store.updateShape(roi.id, { kind: 'rect', x: 5 })
  await flush()
  assert.equal(roi.shape.x, 5)
  store.removeRoi(roi.id)
  assert.equal(store.rois.length, 0)
  assert.equal(store.selectedId, null)
})

test('阈值 store：load 幂等、save 回填', async () => {
  let getCalls = 0
  mock.setHandler((msg) => {
    if (msg.cmd === 'get_settings') {
      getCalls++
      return { joint_temp: 80, delta_t: 12, phase_delta_t: 6 }
    }
    if (msg.cmd === 'set_settings') {
      return { joint_temp: msg.joint_temp, delta_t: msg.delta_t, phase_delta_t: msg.phase_delta_t }
    }
    return null
  })
  const store = useSettingsStore()
  await store.load()
  await store.load()
  assert.equal(getCalls, 1, 'load 应幂等')
  assert.equal(store.loaded, true)
  assert.equal(store.thresholds.joint_temp, 80)

  const saved = await store.save({ joint_temp: 90, delta_t: 15, phase_delta_t: 8 })
  assert.equal(saved.joint_temp, 90)
  assert.equal(store.thresholds.delta_t, 15)
})

test('图片 store：取温边界、加载与发射率修正', async () => {
  const meta = { width: 2, height: 2, temp_min: 10, temp_max: 13 }
  const bytes = new Uint8Array(new Float32Array([10, 11, 12, 13]).buffer)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  const b64 = btoa(bin)

  mock.setHandler((msg) => {
    if (msg.cmd === 'load_image') return meta
    if (msg.cmd === 'get_temps') return b64
    if (msg.cmd === 'set_emissivity') return { ...meta, emissivity: msg.value }
    return null
  })
  const store = useImageStore()
  assert.equal(store.tempAt(0, 0), null, '无图像时返回 null')
  await store.load('/tmp/a.jpg')

  assert.equal(store.hasImage, true)
  assert.equal(store.tempAt(1, 1), 13)
  assert.equal(store.tempAt(-1, 0), null)
  assert.equal(store.tempAt(2, 0), null)
  assert.equal(store.effectiveMin, 10)
  assert.equal(store.effectiveMax, 13)
  assert.equal(store.error, '')

  store.setScale(0, 20)
  assert.equal(store.effectiveMin, 0)
  store.autoScale()
  assert.equal(store.effectiveMax, 13)

  await store.changeEmissivity(0.9)
  assert.equal(store.meta.emissivity, 0.9)
  assert.equal(store.temps.length, 4)

  store.setHover(1, 0)
  assert.deepEqual({ ...store.hover }, { x: 1, y: 0, temp: 11 })
  store.clearHover()
  assert.equal(store.hover, null)
})

test('图片 store：加载失败写入可读错误', async () => {
  mock.setHandler(() => {
    throw '普通预览图无热数据'
  })
  const store = useImageStore()
  await store.load('/tmp/bad.jpg')
  assert.equal(store.hasImage, false)
  assert.equal(store.meta, null)
  assert.equal(store.error, '普通预览图无热数据')
  assert.equal(store.loading, false)
})

test('台账 store：查询、筛选与增删改', async () => {
  const records = [{ id: 1, tower_no: 'T1' }]
  mock.setHandler((msg) => {
    if (msg.cmd === 'ledger_query') return records
    if (msg.cmd === 'ledger_stats') return [1, 0, 0, 0]
    return null
  })
  const store = useLedgerStore()
  await store.refresh()
  assert.equal(store.records.length, 1)
  assert.equal(store.levelCounts[0], 1)
  assert.equal(mock.last().cmd, 'ledger_stats')

  store.setFilter('T1', 2)
  await store.refresh()
  const q = mock.calls.find((c) => c.cmd === 'ledger_query' && c.keyword === 'T1')
  assert.equal(q.level, 2)
  assert.equal(store.keyword, 'T1')
  assert.equal(store.levelFilter, 2)
})

test('图片 store：全图极值缓存、标注开关与发射率修正后重取', async () => {
  const meta = { width: 2, height: 2, temp_min: 10, temp_max: 13 }
  const bytes = new Uint8Array(new Float32Array([10, 11, 12, 13]).buffer)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  const b64 = btoa(bin)
  const ex = { max: 13, min: 10, max_pos: [1, 1], min_pos: [0, 0] }
  let extremaCalls = 0
  mock.setHandler((msg) => {
    if (msg.cmd === 'load_image') return meta
    if (msg.cmd === 'get_temps') return b64
    if (msg.cmd === 'image_extrema') {
      extremaCalls++
      return ex
    }
    if (msg.cmd === 'set_emissivity') return { ...meta, emissivity: msg.value }
    return null
  })

  const store = useImageStore()
  assert.equal(store.showTempMarks, true, '标注总开关默认开启')
  await store.load('/tmp/a.jpg')
  assert.deepEqual({ ...store.extrema }, ex)
  assert.equal(extremaCalls, 1)

  store.toggleTempMarks()
  assert.equal(store.showTempMarks, false)
  store.toggleTempMarks()
  assert.equal(store.showTempMarks, true)

  await store.changeEmissivity(0.9)
  assert.equal(extremaCalls, 2, '温度矩阵重算后应重新取极值')

  store.reset()
  assert.equal(store.extrema, null)
})

test('图片 store：极值读取失败静默降级，不阻断图片加载', async () => {
  const meta = { width: 2, height: 2, temp_min: 10, temp_max: 13 }
  const bytes = new Uint8Array(new Float32Array([10, 11, 12, 13]).buffer)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  const b64 = btoa(bin)
  mock.setHandler((msg) => {
    if (msg.cmd === 'load_image') return meta
    if (msg.cmd === 'get_temps') return b64
    if (msg.cmd === 'image_extrema') throw '极值接口不可用'
    return null
  })

  const originalError = console.error
  console.error = () => {}
  try {
    const store = useImageStore()
    await store.load('/tmp/a.jpg')
    assert.equal(store.hasImage, true)
    assert.equal(store.error, '')
    assert.equal(store.extrema, null)
  } finally {
    console.error = originalError
  }
})

test('ROI：统计回填极值坐标，旧响应保持兼容', async () => {
  mock.setHandler((msg) => {
    if (msg.cmd === 'roi_stats') {
      return {
        max: 75, min: 70, avg: 72.5, delta_t: 5, count: 10,
        extrema: { max: 75, min: 70, max_pos: [4, 5], min_pos: [1, 2] },
        verdict: null
      }
    }
    return null
  })
  const store = useRoiStore()
  const roi = store.addRoi('rect', { kind: 'rect' })
  await flush()
  assert.deepEqual(roi.stats.max_pos, [4, 5])
  assert.deepEqual(roi.stats.min_pos, [1, 2])
  assert.equal(roi.stats.max, 75)

  // 没有 extrema 字段的旧响应不应附加坐标字段
  mock.setHandler(() => ({
    max: 1, min: 0, avg: 0.5, delta_t: 1, count: 1, verdict: null
  }))
  store.updateShape(roi.id, { kind: 'rect', x: 5 })
  await flush()
  assert.equal(roi.stats.max_pos, undefined)
})

test('视图 store：openInAnalysis 加载原图并切回分析页', async () => {
  const meta = { width: 2, height: 2, temp_min: 10, temp_max: 13 }
  const bytes = new Uint8Array(new Float32Array([10, 11, 12, 13]).buffer)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  const b64 = btoa(bin)
  mock.setHandler((msg) => {
    if (msg.cmd === 'load_image') return meta
    if (msg.cmd === 'get_temps') return b64
    if (msg.cmd === 'image_extrema') return { max: 13, min: 10, max_pos: [1, 1], min_pos: [0, 0] }
    return null
  })
  const { useViewStore } = await import('../src/stores/view.js')
  const view = useViewStore()
  assert.equal(view.activeView, 'analysis')
  view.activeView = 'batch'
  await view.openInAnalysis('/tmp/map.jpg')
  assert.equal(view.activeView, 'analysis')
  assert.ok(mock.calls.some((c) => c.cmd === 'load_image' && c.path === '/tmp/map.jpg'))
  assert.equal(useImageStore().hasImage, true)
})
