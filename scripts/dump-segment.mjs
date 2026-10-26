// 转储指定 marker 段的内容：hex + f32/f64 解释
// 用法: node scripts/dump-segment.mjs <file.jpg> <marker_hex> [index]
import { readFileSync } from 'node:fs'

const buf = readFileSync(process.argv[2])
const wantMarker = parseInt(process.argv[3], 16)
const wantIndex = process.argv[4] ? parseInt(process.argv[4]) : 0

const segs = []
let pos = 2
while (pos < buf.length - 1) {
  if (buf[pos] !== 0xff) break
  let m = buf[pos + 1]
  while (m === 0xff) { pos++; m = buf[pos + 1] }
  pos += 2
  if (m === 0xd9 || m === 0xda) break
  const len = buf.readUInt16BE(pos)
  if (m === wantMarker) segs.push(buf.subarray(pos + 2, pos + len))
  pos += len
}

const d = segs[wantIndex]
if (!d) { console.log('segment not found'); process.exit(1) }
console.log(`marker 0x${wantMarker.toString(16)} #${wantIndex}: ${d.length} bytes`)

for (let off = 0; off < d.length; off += 16) {
  const chunk = d.subarray(off, Math.min(off + 16, d.length))
  const hex = Array.from(chunk).map((b) => b.toString(16).padStart(2, '0')).join(' ')
  const ascii = Array.from(chunk).map((c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : '.')).join('')
  console.log(`${off.toString(16).padStart(4, '0')}: ${hex.padEnd(47)} ${ascii}`)
}

console.log('\n-- f32 LE 逐 4 字节 --')
for (let off = 0; off + 4 <= d.length; off += 4) {
  const v = d.readFloatLE(off)
  if (Number.isFinite(v) && Math.abs(v) > 1e-6 && Math.abs(v) < 1e9) {
    console.log(`  +0x${off.toString(16)}: ${v}`)
  }
}
