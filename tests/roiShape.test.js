import test from 'node:test'
import assert from 'node:assert/strict'
import { Polygon, Polyline, Rect, ActiveSelection } from 'fabric'
import { shapeOf, hexToRgba, roiTargetsOf } from '../src/utils/roiShape.js'

const near = (a, b) => Math.abs(a - b) < 1e-6
const assertPoints = (actual, expected) => {
  assert.equal(actual.length, expected.length)
  actual.forEach(([x, y], i) => {
    assert.ok(near(x, expected[i][0]), `点 ${i} x: ${x} != ${expected[i][0]}`)
    assert.ok(near(y, expected[i][1]), `点 ${i} y: ${y} != ${expected[i][1]}`)
  })
}

const makePoly = (Ctor, points, kind) =>
  new Ctor(points.map(([x, y]) => ({ x, y })), {
    originX: 'left',
    originY: 'top',
    data: { roiId: 'r1', roiKind: kind }
  })

test('多边形：初始形状与绘制时的场景坐标一致', () => {
  const points = [[10, 10], [110, 10], [110, 110], [10, 110]]
  const shape = shapeOf(makePoly(Polygon, points, 'poly'))
  assert.equal(shape.kind, 'poly')
  assertPoints(shape.points, points)
})

test('多边形：拖动后形状随之平移（回归：直接读 points 会拿到旧坐标）', () => {
  const points = [[10, 10], [110, 10], [110, 110], [10, 110]]
  const obj = makePoly(Polygon, points, 'poly')
  obj.set({ left: obj.left + 50, top: obj.top + 30 })
  obj.setCoords()
  // Fabric 拖动只改 left/top，points 保持不变
  assert.deepEqual(obj.points.map((p) => [p.x, p.y]), points)
  assertPoints(shapeOf(obj).points, points.map(([x, y]) => [x + 50, y + 30]))
})

test('多边形：控制点缩放后区域随之放大', () => {
  const points = [[10, 10], [110, 10], [110, 110], [10, 110]]
  const obj = makePoly(Polygon, points, 'poly')
  const before = shapeOf(obj)
  obj.set({ scaleX: 2, scaleY: 2 })
  obj.setCoords()
  const after = shapeOf(obj)
  const width = (s) => Math.max(...s.points.map((p) => p[0])) - Math.min(...s.points.map((p) => p[0]))
  assert.ok(near(width(after), width(before) * 2), '缩放后宽度应加倍')
  assert.ok(after.points.some(([x], i) => !near(x, before.points[i][0])), '点坐标必须随缩放变化')
})

test('线段：拖动后折线点同步平移', () => {
  const points = [[0, 5], [9, 5]]
  const obj = makePoly(Polyline, points, 'line')
  obj.set({ left: obj.left + 20, top: obj.top - 10 })
  obj.setCoords()
  const shape = shapeOf(obj)
  assert.equal(shape.kind, 'line')
  assertPoints(shape.points, points.map(([x, y]) => [x + 20, y - 10]))
})

test('矩形：拖动与缩放后形状正确', () => {
  const obj = new Rect({
    left: 10, top: 20, width: 30, height: 40,
    originX: 'left', originY: 'top',
    data: { roiId: 'r2', roiKind: 'rect' }
  })
  obj.set({ left: 15, top: 25, scaleX: 2, scaleY: 3 })
  obj.setCoords()
  assert.deepEqual(shapeOf(obj), { kind: 'rect', x: 15, y: 25, w: 60, h: 120 })
})

test('缺少 data.roiKind 时按 Fabric 对象类型推断', () => {
  const poly = shapeOf(makePoly(Polygon, [[0, 0], [10, 0], [5, 10]], undefined))
  assert.equal(poly.kind, 'poly')
  const rect = shapeOf(new Rect({ left: 0, top: 0, width: 5, height: 5, originX: 'left', originY: 'top' }))
  assert.equal(rect.kind, 'rect')
})

test('ROI 颜色转半透明填充', () => {
  assert.equal(hexToRgba('#00A650', 0.14), 'rgba(0, 166, 80, 0.14)')
  assert.equal(hexToRgba('#fff', 0.5), 'rgba(255, 255, 255, 0.5)')
  assert.equal(hexToRgba('', 0.2), 'rgba(0, 166, 80, 0.2)')
})

const assertRect = (shape, x, y, w, h) => {
  assert.equal(shape.kind, 'rect')
  const values = [[shape.x, x], [shape.y, y], [shape.w, w], [shape.h, h]]
  values.forEach(([a, b]) => assert.ok(near(a, b), `${a} != ${b}`))
}

test('框选：事件目标可展开为选择框内的全部 ROI 对象', () => {
  const rect = new Rect({ left: 10, top: 20, width: 30, height: 40, data: { roiId: 'r1', roiKind: 'rect' } })
  const poly = makePoly(Polygon, [[100, 100], [150, 100], [150, 150]], 'poly')
  const targets = roiTargetsOf(new ActiveSelection([rect, poly]))
  assert.equal(targets.length, 2)
  assert.ok(targets.includes(rect))
  assert.ok(targets.includes(poly))
  assert.equal(roiTargetsOf(rect)[0], rect)
  assert.deepEqual(roiTargetsOf(undefined), [])
  assert.deepEqual(roiTargetsOf(new Rect({ left: 0, top: 0, width: 5, height: 5 })), [])
})

test('框选多个 ROI 后整体移动：矩形与多边形都换算为场景绝对坐标', () => {
  const rect = new Rect({
    left: 10, top: 20, width: 30, height: 40,
    originX: 'left', originY: 'top',
    data: { roiId: 'r1', roiKind: 'rect' }
  })
  const points = [[100, 100], [150, 100], [150, 150]]
  const poly = makePoly(Polygon, points, 'poly')
  const sel = new ActiveSelection([rect, poly])

  // 进入选择框后 left/top 变成局部坐标，形状仍应是原来的场景位置
  assertRect(shapeOf(rect), 10, 20, 30, 40)
  assertPoints(shapeOf(poly).points, points)

  sel.set({ left: sel.left + 50, top: sel.top + 30 })
  sel.setCoords()
  assertRect(shapeOf(rect), 60, 50, 30, 40)
  assertPoints(shapeOf(poly).points, points.map(([x, y]) => [x + 50, y + 30]))
})
