// 生成应用图标源图（1024x1024 PNG，纯 Node 无第三方依赖）
// 用法: node scripts/gen-icon.mjs [输出路径]
import { deflateSync } from 'node:zlib'
import { writeFileSync } from 'node:fs'

const SIZE = 1024
const OUT = process.argv[2] || 'app-icon.png'

// ---- PNG 编码 ----
const crcTable = (() => {
  const t = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    t[n] = c
  }
  return t
})()

function crc32(buf) {
  let c = 0xffffffff
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

function chunk(type, data) {
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crc])
}

function lerp(a, b, t) {
  return a + (b - a) * t
}

function mix(c1, c2, t) {
  return [lerp(c1[0], c2[0], t), lerp(c1[1], c2[1], t), lerp(c1[2], c2[2], t)]
}

// 铁红色谱（黑-紫-红-橙-黄-白）
const IRON = [
  [10, 8, 20],
  [60, 12, 90],
  [150, 20, 80],
  [220, 60, 30],
  [250, 150, 30],
  [255, 220, 120],
  [255, 255, 240]
]

function ironAt(t) {
  const x = Math.min(0.9999, Math.max(0, t)) * (IRON.length - 1)
  const i = Math.floor(x)
  return mix(IRON[i], IRON[i + 1], x - i)
}

const raw = Buffer.alloc(SIZE * (SIZE * 3 + 1))
const c = SIZE / 2
const radius = SIZE * 0.46
const corner = SIZE * 0.22

for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 3 + 1)
  raw[rowStart] = 0 // filter: none
  for (let x = 0; x < SIZE; x++) {
    // 圆角矩形背景（深蓝渐变）
    const bgTop = [8, 30, 66]
    const bgBottom = [0, 91, 172]
    let bg = mix(bgTop, bgBottom, y / SIZE)

    // 圆角判定
    const inX = x >= corner && x < SIZE - corner
    const inY = y >= corner && y < SIZE - corner
    let inside = inX || inY
    if (!inside) {
      const dx = Math.max(corner - x, x - (SIZE - corner), 0)
      const dy = Math.max(corner - y, y - (SIZE - corner), 0)
      inside = dx * dx + dy * dy <= corner * corner
    }

    let col = inside ? bg : [0, 0, 0]

    if (inside) {
      // 热区圆盘（中心亮斑）
      const dx = (x - c) / radius
      const dy = (y - c) / radius
      const d = Math.sqrt(dx * dx + dy * dy)
      if (d <= 1) {
        const heat = Math.pow(1 - d, 1.6)
        const thermal = ironAt(heat)
        const blend = Math.min(1, heat * 1.35)
        col = mix(col, thermal, blend)
      }
      // 等温线圈
      const ring = Math.abs((d * 5) % 1 - 0.5)
      if (d <= 1.06 && ring < 0.045) {
        col = mix(col, [255, 255, 255], 0.35 * (1 - d))
      }
      // 外描边
      const edge = Math.max(Math.abs(dx), Math.abs(dy))
      if (edge > 0.965) col = mix(col, [255, 255, 255], 0.55)
    }

    const p = rowStart + 1 + x * 3
    raw[p] = Math.round(col[0])
    raw[p + 1] = Math.round(col[1])
    raw[p + 2] = Math.round(col[2])
  }
}

const ihdr = Buffer.alloc(13)
ihdr.writeUInt32BE(SIZE, 0)
ihdr.writeUInt32BE(SIZE, 4)
ihdr[8] = 8 // bit depth
ihdr[9] = 2 // color type RGB
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0))
])

writeFileSync(OUT, png)
console.log(`icon written: ${OUT} (${png.length} bytes)`)
