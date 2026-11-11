import test from 'node:test'
import assert from 'node:assert/strict'
import { buildTempMarks, HOT_COLOR, COLD_COLOR } from '../src/utils/tempMarks.js'

const extrema = { max: 75.2, min: 30.1, max_pos: [10, 20], min_pos: [3, 4] }
const rois = [
  { id: 'a', label: 'ROI-1', stats: { max: 60, min: 35, max_pos: [5, 6], min_pos: [7, 8] } },
  { id: 'b', label: 'ROI-2', stats: { max: 80, min: 20, max_pos: [1, 1], min_pos: [2, 2] } }
]

test('无数据时返回空标注', () => {
  assert.deepEqual(buildTempMarks(), [])
  assert.deepEqual(buildTempMarks({ extrema: null, rois: [], selectedId: null }), [])
  assert.equal(buildTempMarks({ extrema: null, rois: [{ id: 'a', stats: null }], selectedId: 'a' }).length, 0)
})

test('全图标注：坐标、温度、颜色与导出字段同源', () => {
  const marks = buildTempMarks({ extrema, rois: [], selectedId: null })
  assert.equal(marks.length, 2)

  const [hot, cold] = marks
  assert.deepEqual([hot.x, hot.y], [10, 20])
  assert.equal(hot.temp, 75.2)
  assert.equal(hot.scope, 'image')
  assert.equal(hot.hot, true)
  assert.equal(hot.color, HOT_COLOR)
  assert.deepEqual(hot.rgb, [255, 107, 61])
  assert.equal(hot.ascii, 'IMG MAX 75.2C')
  assert.ok(hot.label.includes('全图最高') && hot.label.includes('75.2'))

  assert.deepEqual([cold.x, cold.y], [3, 4])
  assert.equal(cold.temp, 30.1)
  assert.equal(cold.color, COLD_COLOR)
  assert.deepEqual(cold.rgb, [64, 169, 255])
  assert.equal(cold.ascii, 'IMG MIN 30.1C')
})

test('只标注当前选中的 ROI，未选中项不出现', () => {
  const marks = buildTempMarks({ extrema, rois, selectedId: 'b' })
  assert.equal(marks.length, 4)
  const roiMarks = marks.filter((m) => m.scope === 'roi')
  assert.equal(roiMarks.length, 2)
  assert.deepEqual([roiMarks[0].x, roiMarks[0].y], [1, 1])
  assert.equal(roiMarks[0].temp, 80)
  assert.equal(roiMarks[0].ascii, 'ROI-2 MAX 80.0C')
  assert.ok(roiMarks[0].label.includes('ROI-2 最高'))
  assert.equal(roiMarks[1].ascii, 'ROI-2 MIN 20.0C')
  assert.ok(!marks.some((m) => m.label.includes('ROI-1')))
})

test('无选中项或统计缺失时跳过 ROI 标注', () => {
  assert.equal(buildTempMarks({ extrema, rois, selectedId: null }).length, 2)
  assert.equal(buildTempMarks({ extrema, rois, selectedId: 'missing' }).length, 2)
  const noStats = [{ id: 'a', label: 'ROI-1', stats: { max: 60, min: 35 } }]
  assert.equal(buildTempMarks({ extrema, rois: noStats, selectedId: 'a' }).length, 2)
  const nullStats = [{ id: 'a', label: 'ROI-1', stats: null }]
  assert.equal(buildTempMarks({ extrema, rois: nullStats, selectedId: 'a' }).length, 2)
})

test('全图极值缺坐标时仍可标注选中 ROI', () => {
  const partial = { max: 75.2, min: 30.1 }
  const marks = buildTempMarks({ extrema: partial, rois, selectedId: 'a' })
  assert.equal(marks.length, 2)
  assert.ok(marks.every((m) => m.scope === 'roi'))
})
