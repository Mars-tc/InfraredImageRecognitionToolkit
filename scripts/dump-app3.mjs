// 深入分析 DJI R-JPEG 的 APP3 段：打印每段头部字节，寻找原始热数据容器格式
// 用法: node scripts/dump-app3.mjs <file.jpg>
import { readFileSync } from 'node:fs'

const buf = readFileSync(process.argv[2])
const segments = []
let pos = 2
while (pos < buf.length - 1) {
  if (buf[pos] !== 0xff) break
  let m = buf[pos + 1]
  while (m === 0xff) { pos++; m = buf[pos + 1] }
  pos += 2
  if (m === 0xd9) break
  if (m === 0xda) break
  const len = buf.readUInt16BE(pos)
  const data = buf.subarray(pos + 2, pos + len)
  if (m === 0xe3) segments.push({ offset: pos - 2, data })
  pos += len
}

console.log(`APP3 segments: ${segments.length}, total payload=${segments.reduce((s, x) => s + x.data.length, 0)}`)
for (let i = 0; i < segments.length; i++) {
  const d = segments[i].data
  const head = d.subarray(0, 24).toString('hex').replace(/(..)/g, '$1 ').trim()
  const ascii = Array.from(d.subarray(0, 16)).map((c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : '.')).join('')
  console.log(`#${i} @${segments[i].offset} len=${d.length} head=[${head}] ascii="${ascii}"`)
}

// 拼接全部 APP3 payload 看整体结构
const all = Buffer.concat(segments.map((s) => s.data))
console.log('\nconcatenated first 64 bytes:')
console.log(all.subarray(0, 64).toString('hex').replace(/(..)/g, '$1 ').trim())
console.log('TIFF magic?', all.subarray(0, 4).toString('hex'), '(49492a49=II*, 4d4d002a=MM*)')
// 是否有 TIFF 头在某个偏移
for (let i = 0; i < Math.min(all.length - 4, 4096); i++) {
  const h = all.readUInt32BE(i)
  if (h === 0x49492a00 || h === 0x4d4d002a) {
    console.log(`TIFF header found at concat offset ${i}: ${all.subarray(i, i + 8).toString('hex')}`)
    break
  }
}
