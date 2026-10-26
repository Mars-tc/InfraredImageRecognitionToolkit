// 在文件中搜索 FLIR 风格标定参数（f32/f64 小端）
// FLIR 大气传输常量几乎出现在所有 FLIR 参数块中，可作为定位锚点
// 用法: node scripts/scan-params.mjs <file.jpg>
import { readFileSync } from 'node:fs'

const buf = readFileSync(process.argv[2])

// (值, 说明) — f64 优先
const f64Targets = [
  [0.006569, 'AtmosphericTransAlpha1'],
  [0.012620, 'AtmosphericTransAlpha2'],
  [-0.002276, 'AtmosphericTransBeta1'],
  [-0.006670, 'AtmosphericTransBeta2'],
  [1.900000, 'AtmosphericTransX'],
  [17096.667, 'PlanckR1(Boson?)'],
  [21106.77, 'PlanckR1(Lepton?)'],
  [1391.5, 'PlanckB(Boson?)'],
  [1501.0, 'PlanckB(Lepton?)'],
  [-138.0, 'PlanckF?'],
  [-7342.0, 'PlanckO?'],
  [0.0125, 'PlanckR2?'],
  [0.95, 'Emissivity?'],
  [1.0, 'IRWindowTransmission?']
]
const f32Targets = f64Targets.slice(0, 9)

function findDouble(value) {
  const b = Buffer.alloc(8)
  b.writeDoubleLE(value)
  const hits = []
  let idx = buf.indexOf(b)
  while (idx !== -1) {
    hits.push(idx)
    idx = buf.indexOf(b, idx + 1)
  }
  return hits
}
function findFloat(value) {
  const b = Buffer.alloc(4)
  b.writeFloatLE(value)
  const hits = []
  let idx = buf.indexOf(b)
  while (idx !== -1) {
    hits.push(idx)
    idx = buf.indexOf(b, idx + 1)
  }
  return hits
}

for (const [v, name] of f64Targets) {
  const hits = findDouble(v)
  if (hits.length) console.log(`f64 ${name} (${v}) @ ${hits.join(', ')}`)
}
for (const [v, name] of f32Targets) {
  const hits = findFloat(v)
  if (hits.length) console.log(`f32 ${name} (${v}) @ ${hits.join(', ')}`)
}

// 打印 EXIF APP1 段中的可读字符串（找标定参数块位置线索）
let pos = 2
while (pos < buf.length - 1) {
  if (buf[pos] !== 0xff) break
  let m = buf[pos + 1]
  while (m === 0xff) { pos++; m = buf[pos + 1] }
  pos += 2
  if (m === 0xd9 || m === 0xda) break
  const len = buf.readUInt16BE(pos)
  if (m === 0xe1 && pos + len < buf.length) {
    const data = buf.subarray(pos + 2, pos + len)
    const strings = []
    let cur = ''
    for (const c of data) {
      if (c >= 32 && c < 127) cur += String.fromCharCode(c)
      else { if (cur.length >= 5) strings.push(cur); cur = '' }
    }
    if (cur.length >= 5) strings.push(cur)
    console.log('\nEXIF strings:', strings.slice(0, 60).join(' | '))
  }
  pos += len
}
