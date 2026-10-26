// 统计 APP3 拼接后的 16bit 原始热数据值域与直方图
import { readFileSync } from 'node:fs'

const buf = readFileSync(process.argv[2])
const parts = []
let pos = 2
while (pos < buf.length - 1) {
  if (buf[pos] !== 0xff) break
  let m = buf[pos + 1]
  while (m === 0xff) { pos++; m = buf[pos + 1] }
  pos += 2
  if (m === 0xd9 || m === 0xda) break
  const len = buf.readUInt16BE(pos)
  if (m === 0xe3) parts.push(buf.subarray(pos + 2, pos + len))
  pos += len
}
const all = Buffer.concat(parts)
const n = all.length / 2
let min = Infinity, max = -Infinity, sum = 0
const hist = new Map()
for (let i = 0; i < n; i++) {
  const v = all.readUInt16LE(i * 2)
  if (v < min) min = v
  if (v > max) max = v
  sum += v
  const b = Math.floor(v / 1000) * 1000
  hist.set(b, (hist.get(b) || 0) + 1)
}
console.log(`pixels=${n} min=${min} max=${max} mean=${(sum / n).toFixed(1)}`)
console.log('histogram (1000 buckets):')
for (const [b, c] of [...hist.entries()].sort((a, b2) => a[0] - b2[0])) {
  console.log(`  ${b}-${b + 999}: ${'#'.repeat(Math.round(c / n * 200))} ${c}`)
}
// 角点采样（看是否是有序图像）
console.log('\ncorner samples (LE u16):')
for (const [x, y] of [[0, 0], [639, 0], [0, 511], [639, 511], [320, 256]]) {
  const i = (y * 640 + x) * 2
  console.log(`  (${x},${y}) = ${all.readUInt16LE(i)}`)
}
