import test from 'node:test'
import assert from 'node:assert/strict'
import { buildLut, PALETTE_OPTIONS } from '../src/utils/palettes.js'
import { DEFECT_LEVELS, levelInfo } from '../src/utils/defects.js'
import {
  fmtTemp, fmtDelta, fmtGps, fmtEmissivity, fmtHumidity, fmtDistance
} from '../src/utils/format.js'

test('四套调色板均生成 256 级 RGBA LUT', () => {
  assert.equal(PALETTE_OPTIONS.length, 4)
  for (const { value } of PALETTE_OPTIONS) {
    const lut = buildLut(value)
    assert.ok(lut instanceof Uint8ClampedArray, `${value} 应返回 Uint8ClampedArray`)
    assert.equal(lut.length, 256 * 4)
    for (let i = 0; i < 256; i++) {
      assert.equal(lut[i * 4 + 3], 255, `${value} alpha`)
      for (let c = 0; c < 3; c++) {
        const v = lut[i * 4 + c]
        assert.ok(Number.isInteger(v) && v >= 0 && v <= 255, `${value} 通道应为整数: ${v}`)
      }
    }
  }
})

test('调色板端点与灰度语义', () => {
  const iron = buildLut('iron')
  assert.deepEqual([...iron.slice(0, 3)], [0, 0, 0])
  assert.deepEqual([...iron.slice(255 * 4, 255 * 4 + 3)], [255, 255, 217])

  const gray = buildLut('gray')
  for (let i = 0; i < 256; i++) {
    assert.equal(gray[i * 4], gray[i * 4 + 1])
    assert.equal(gray[i * 4 + 1], gray[i * 4 + 2])
  }

  const whitehot = buildLut('whitehot')
  assert.deepEqual([...whitehot.slice(0, 3)], [0, 0, 0])
  assert.deepEqual([...whitehot.slice(255 * 4, 255 * 4 + 3)], [255, 250, 242])
})

test('未知调色板回退铁红', () => {
  assert.deepEqual([...buildLut('__unknown__')], [...buildLut('iron')])
  assert.deepEqual([...buildLut(undefined)], [...buildLut('iron')])
})

test('缺陷等级映射与越界回退', () => {
  assert.equal(DEFECT_LEVELS.length, 4)
  assert.equal(levelInfo(0).label, '其他')
  assert.equal(levelInfo(1).label, '一般')
  assert.equal(levelInfo(2).label, '重大')
  assert.equal(levelInfo(3).label, '紧急')
  assert.strictEqual(levelInfo(4), DEFECT_LEVELS[0])
  assert.strictEqual(levelInfo(-1), DEFECT_LEVELS[0])
  assert.strictEqual(levelInfo(null), DEFECT_LEVELS[0])
  assert.strictEqual(levelInfo('2'), DEFECT_LEVELS[0])
})

test('展示格式化：无效值统一显示 --', () => {
  for (const fn of [fmtTemp, fmtDelta, fmtEmissivity, fmtHumidity, fmtDistance]) {
    assert.equal(fn(null), '--')
    assert.equal(fn(undefined), '--')
    assert.equal(fn(NaN), '--')
  }
  assert.equal(fmtGps(null), '--')
  assert.equal(fmtGps(''), '--')
  assert.equal(fmtGps('21.89, 111.75'), '21.89, 111.75')
})

test('展示格式化：数值与符号', () => {
  assert.equal(fmtTemp(28.955), '29.0 ℃')
  assert.equal(fmtTemp(28.956, 2), '28.96 ℃')
  assert.equal(fmtTemp(-3.14), '-3.1 ℃')
  assert.equal(fmtDelta(4.24), '+4.2 K')
  assert.equal(fmtDelta(-4.24), '-4.2 K')
  assert.equal(fmtDelta(0), '0.0 K')
  assert.equal(fmtEmissivity(0.956), '0.96')
  assert.equal(fmtHumidity(49.6), '50 %')
  assert.equal(fmtDistance(5), '5.0 m')
})
