import test from 'node:test'
import assert from 'node:assert/strict'
import {
  isValidCoord, toPoints, computeBounds, createProjection, projectPoint,
  projectPoints, fitTransform, zoomAt, gridTicks, formatCoord
} from '../src/utils/mapProjection.js'

const items = [
  { path: '/a.jpg', lat: 30, lon: 120 },
  { path: '/b.jpg', lat: 31, lon: 121 },
  { path: '/no-gps.jpg', lat: null, lon: null },
  { path: '/zero.jpg', lat: 0, lon: 0 },
  { path: '/bad.jpg', lat: 95, lon: 200 }
]

test('isValidCoord：过滤非数值、越界与零坐标', () => {
  assert.equal(isValidCoord(21.8869589, 111.755511474), true)
  assert.equal(isValidCoord(-33.9, 151.2), true)
  assert.equal(isValidCoord(0, 0), false)
  assert.equal(isValidCoord(0.0000001, 0), false)
  assert.equal(isValidCoord(91, 120), false)
  assert.equal(isValidCoord(30, 181), false)
  assert.equal(isValidCoord(NaN, 120), false)
  assert.equal(isValidCoord(null, undefined), false)
  assert.equal(isValidCoord('30', '120'), false)
})

test('toPoints：只保留有效点并生成稳定 id', () => {
  const pts = toPoints(items)
  assert.equal(pts.length, 2)
  assert.equal(pts[0].id, '/a.jpg')
  assert.equal(pts[1].lon, 121)
  assert.equal(toPoints([]).length, 0)
  assert.equal(toPoints(null).length, 0)
})

test('computeBounds：范围、中心纬度与空输入', () => {
  const b = computeBounds(toPoints(items))
  assert.deepEqual(b, { minLat: 30, maxLat: 31, minLon: 120, maxLon: 121, centerLat: 30.5 })
  assert.equal(computeBounds([]), null)
  assert.equal(computeBounds([{ lat: 0, lon: 0 }]), null)
})

test('createProjection：等比缩放、padding 内、单点居中', () => {
  const pts = toPoints(items)
  const b = computeBounds(pts)
  const proj = createProjection(b, 800, 600, 50)
  assert.ok(proj.scale > 0)
  assert.ok(proj.cosLat < 1 && proj.cosLat > 0.5)

  const projected = projectPoints(pts, b, 800, 600, 50)
  assert.equal(projected.length, 2)
  for (const p of projected) {
    assert.ok(p.x >= 50 - 1e-6 && p.x <= 750 + 1e-6, `x=${p.x}`)
    assert.ok(p.y >= 50 - 1e-6 && p.y <= 550 + 1e-6, `y=${p.y}`)
  }
  // 纬度更高 → 更靠上；经度更大 → 更靠右
  assert.ok(projected[1].y < projected[0].y)
  assert.ok(projected[1].x > projected[0].x)

  const single = projectPoints([{ id: 's', lat: 31, lon: 121 }], computeBounds([{ lat: 31, lon: 121 }]), 800, 600, 50)
  assert.ok(Math.abs(single[0].x - 400) < 1e-6, `单点 x=${single[0].x}`)
  assert.ok(Math.abs(single[0].y - 300) < 1e-6, `单点 y=${single[0].y}`)
  assert.equal(createProjection(null, 800, 600), null)
  assert.deepEqual(projectPoint(null, 30, 120), { x: 0, y: 0 })
})

test('cos 纬度修正：高纬度相同经度差对应更小的横向像素距离', () => {
  // 纬度跨度同为 1°，让高度成为缩放约束，才能体现 cos 修正的横向压缩
  const low = computeBounds(toPoints([{ lat: 0, lon: 120 }, { lat: 1, lon: 121 }]))
  const high = computeBounds(toPoints([{ lat: 70, lon: 120 }, { lat: 71, lon: 121 }]))
  const lowProj = createProjection(low, 800, 600, 50)
  const highProj = createProjection(high, 800, 600, 50)
  const dx = (b, proj) => projectPoint(proj, b.maxLat, b.maxLon).x - projectPoint(proj, b.maxLat, b.minLon).x
  assert.ok(dx(high, highProj) < dx(low, lowProj))
})

test('zoomAt：指针锚点不动、缩放钳制与复位', () => {
  const t = { scale: 1, tx: 0, ty: 0 }
  assert.deepEqual(fitTransform(), t)
  const z = zoomAt(t, 2, 300, 200)
  assert.equal(z.scale, 2)
  assert.deepEqual(z, { scale: 2, tx: -300, ty: -200 })
  // 同一指针位置再缩回 1 倍，锚点仍保持
  const back = zoomAt(z, 0.5, 300, 200)
  assert.ok(Math.abs(back.scale - 1) < 1e-9)
  assert.ok(Math.abs(back.tx) < 1e-9 && Math.abs(back.ty) < 1e-9)
  assert.equal(zoomAt(t, 1000, 0, 0).scale, 40)
  assert.equal(zoomAt(t, 0.0001, 0, 0).scale, 0.4)
})

test('gridTicks / formatCoord：刻度顺序、退化范围与方位符号', () => {
  const b = computeBounds(toPoints(items))
  const ticks = gridTicks(b, 5)
  assert.equal(ticks.lats.length, 5)
  assert.equal(ticks.lons.length, 5)
  assert.equal(ticks.lats[0], 31)
  assert.equal(ticks.lats[4], 30)
  assert.equal(ticks.lons[0], 120)
  assert.equal(ticks.lons[4], 121)
  assert.deepEqual(gridTicks(computeBounds([{ lat: 30, lon: 120 }])), { lats: [], lons: [] })
  assert.deepEqual(gridTicks(null), { lats: [], lons: [] })
  assert.equal(formatCoord(21.8869589), '21.88696°N')
  assert.equal(formatCoord(-21.8869589), '21.88696°S')
  assert.equal(formatCoord(111.755511474, 'lon'), '111.75551°E')
  assert.equal(formatCoord(-111.7555, 'lon'), '111.75550°W')
  assert.equal(formatCoord(undefined, 'lon'), '--')
})
