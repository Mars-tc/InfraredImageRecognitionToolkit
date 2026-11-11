// 真实样本基线校验：e2e/sample.json 是 dump_temps 对御 3T 辐射原图的冻结产物。
// 只读、单次解析，不重写 5.79MB 的样本文件。
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import assert from 'node:assert/strict'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const sample = JSON.parse(readFileSync(join(root, 'e2e', 'sample.json'), 'utf8'))

const FIELDS = [
  'path', 'file_name', 'camera_model', 'datetime', 'gps',
  'width', 'height', 'emissivity', 'distance',
  'humidity', 'reflected_temp', 'atmospheric_temp',
  'temp_min', 'temp_max', 'temps'
]
for (const key of FIELDS) {
  assert.ok(Object.hasOwn(sample, key), `样本缺少字段 ${key}`)
}

assert.equal(sample.width, 640, '御 3T 热数据宽度')
assert.equal(sample.height, 512, '御 3T 热数据高度')
assert.equal(sample.temps.length, 640 * 512, '温度矩阵长度应等于宽×高')
assert.ok(Math.abs(sample.emissivity - 0.95) < 0.01, '发射率应约 0.95')
assert.equal(sample.distance, 5, '样本标定距离 5m')
assert.equal(sample.camera_model, 'M3T')
assert.ok(sample.temp_min > 10 && sample.temp_max < 60, '夜间场景温度应在 10~60℃')
assert.ok(sample.temp_min <= sample.temp_max)

let min = Infinity
let max = -Infinity
for (const t of sample.temps) {
  assert.ok(Number.isFinite(t), '温度矩阵含非有限值')
  if (t < min) min = t
  if (t > max) max = t
}
assert.ok(Math.abs(min - sample.temp_min) < 1e-3, 'temp_min 与矩阵实际最小值不符')
assert.ok(Math.abs(max - sample.temp_max) < 1e-3, 'temp_max 与矩阵实际最大值不符')

console.log(
  `样本基线 OK: ${sample.file_name} ${sample.width}x${sample.height} ` +
  `ε=${sample.emissivity.toFixed(2)} 距离=${sample.distance}m ` +
  `温度 ${min.toFixed(2)}~${max.toFixed(2)}℃ (${sample.temps.length} 点)`
)
