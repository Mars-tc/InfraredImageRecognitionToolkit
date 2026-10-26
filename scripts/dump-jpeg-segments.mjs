// 分析 R-JPEG 的段结构：遍历 JPEG marker，打印 APPn 段标识与长度
// 用法: node scripts/dump-jpeg-segments.mjs <file.jpg>
import { readFileSync } from 'node:fs'

const buf = readFileSync(process.argv[2])
let pos = 2 // skip SOI
const markerNames = {
  0xd8: 'SOI', 0xd9: 'EOI', 0xda: 'SOS', 0xdb: 'DQT', 0xc0: 'SOF0', 0xc2: 'SOF2',
  0xc4: 'DHT', 0xdd: 'DRI', 0xfe: 'COM'
}
while (pos < buf.length - 1) {
  if (buf[pos] !== 0xff) { console.log(`!! 非 marker 字节 @${pos}: ${buf[pos].toString(16)}`); break }
  let m = buf[pos + 1]
  while (m === 0xff) { pos++; m = buf[pos + 1] }
  pos += 2
  if (m === 0xd9) { console.log('EOI'); break }
  if (m === 0xda) {
    const len = buf.readUInt16BE(pos)
    console.log(`SOS @${pos - 2} len=${len} (扫描数据紧随其后)`)
    break
  }
  const len = buf.readUInt16BE(pos)
  const data = buf.subarray(pos + 2, pos + len)
  const name = markerNames[m] || `0x${m.toString(16).padStart(2, '0')}`
  let desc = ''
  if (m >= 0xe0 && m <= 0xef) {
    // APPn: 前几个字节是标识字符串
    let id = ''
    for (let i = 0; i < Math.min(data.length, 16); i++) {
      const c = data[i]
      if (c === 0) break
      id += String.fromCharCode(c)
    }
    desc = ` id="${id}"`
    if (id.startsWith('FLIR') || id.includes('DJ') || id.includes('Therm')) {
      desc += ` hex=${data.subarray(0, 32).toString('hex')}`
    }
  }
  console.log(`${name} @${pos - 2} len=${len}${desc}`)
  pos += len
}
console.log(`total file size: ${buf.length}`)

// 全局搜索关键字符串
for (const kw of ['FLIR', 'DJI', 'R-JPEG', 'FFF', 'Lepton', 'Boson', 'Adobe', 'Exif']) {
  const idx = buf.indexOf(Buffer.from(kw, 'ascii'))
  if (idx >= 0) console.log(`found "${kw}" @${idx}`)
}
