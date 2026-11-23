import test from 'node:test'
import assert from 'node:assert/strict'
import { buildBatchLedgerInputs, buildLedgerDraft } from '../src/utils/ledgerDraft.js'

test('入库草稿：优先使用选中 ROI 的统计、定级与相别', () => {
  const draft = buildLedgerDraft({
    meta: { path: '/imgs/a.jpg' },
    roi: {
      phase: 'B',
      stats: { max: 92.46, delta_t: 18.24 },
      verdict: { level: 3 }
    },
    extrema: { max: 99, min: 20 }
  })
  assert.equal(draft.phase, 'B')
  assert.equal(draft.image_path, '/imgs/a.jpg')
  assert.equal(draft.max_temp, 92.5)
  assert.equal(draft.delta_t, 18.2)
  assert.equal(draft.level, 3)
  assert.equal(draft.tower_no, '')
  assert.equal(draft.equipment, '')
  assert.equal(draft.note, '')
})

test('入库草稿：无 ROI 时回退到全图极值', () => {
  const draft = buildLedgerDraft({
    meta: { path: '/imgs/b.jpg' },
    roi: null,
    extrema: { max: 55.55, min: 30.05 }
  })
  assert.equal(draft.max_temp, 55.6)
  assert.equal(draft.delta_t, 25.5)
  assert.equal(draft.level, 0)
  assert.equal(draft.phase, '')
})

test('批量入库：仅超限与全部成功项两种范围', () => {
  const items = [
    { file_name: 'a.JPG', path: '/a.JPG', max: 80.04, delta_t: 12.06, level: 1, error: null },
    { file_name: 'b.JPG', path: '/b.JPG', max: 40, delta_t: 2, level: 0, error: null },
    { file_name: 'c.JPG', path: '/c.JPG', max: 0, delta_t: 0, level: 0, error: '解析失败' }
  ]

  const over = buildBatchLedgerInputs({
    items, towerNo: 'N-1', equipment: '接头', note: '夜巡', onlyOverLimit: true
  })
  assert.equal(over.length, 1)
  assert.deepEqual(over[0], {
    tower_no: 'N-1',
    equipment: '接头',
    phase: '',
    image_path: '/a.JPG',
    max_temp: 80,
    delta_t: 12.1,
    level: 1,
    note: 'a.JPG 夜巡'
  })

  const all = buildBatchLedgerInputs({ items, onlyOverLimit: false })
  assert.equal(all.length, 2)
  assert.equal(all[1].note, 'b.JPG')
  assert.equal(all[1].tower_no, '')
})

test('批量入库：空列表返回空数组', () => {
  assert.deepEqual(buildBatchLedgerInputs(), [])
})
