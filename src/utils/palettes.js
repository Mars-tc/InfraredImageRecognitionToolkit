// 伪彩色调色板（F4）：铁红 / 彩虹 / 灰度 / 白热
// 每套生成 256 级 RGBA LUT（Uint8ClampedArray，供 Canvas ImageData 直写）

const PALETTES = {
  // 铁红（FLIR 经典 Ironbow）
  iron: [
    [0.0, 0, 0, 0],
    [0.25, 0.45, 0, 0.55],
    [0.5, 0.9, 0.1, 0.1],
    [0.75, 1, 0.55, 0],
    [1.0, 1, 1, 0.85]
  ],
  // 彩虹
  rainbow: [
    [0.0, 0, 0, 1],
    [0.25, 0, 1, 1],
    [0.5, 0, 1, 0],
    [0.75, 1, 1, 0],
    [1.0, 1, 0, 0]
  ],
  // 灰度
  gray: [
    [0.0, 0, 0, 0],
    [1.0, 1, 1, 1]
  ],
  // 白热（略暖的白）
  whitehot: [
    [0.0, 0, 0, 0],
    [0.5, 0.75, 0.7, 0.68],
    [1.0, 1, 0.98, 0.95]
  ]
}

export const PALETTE_OPTIONS = [
  { value: 'iron', label: '铁红' },
  { value: 'rainbow', label: '彩虹' },
  { value: 'gray', label: '灰度' },
  { value: 'whitehot', label: '白热' }
]

function sample(stops, t) {
  t = Math.min(1, Math.max(0, t))
  for (let i = 0; i < stops.length - 1; i++) {
    const [t0, r0, g0, b0] = stops[i]
    const [t1, r1, g1, b1] = stops[i + 1]
    if (t >= t0 && t <= t1) {
      const k = t1 === t0 ? 0 : (t - t0) / (t1 - t0)
      return [
        Math.round((r0 + (r1 - r0) * k) * 255),
        Math.round((g0 + (g1 - g0) * k) * 255),
        Math.round((b0 + (b1 - b0) * k) * 255)
      ]
    }
  }
  const last = stops[stops.length - 1]
  return [last[1] * 255, last[2] * 255, last[3] * 255]
}

/** 生成 256 级 LUT：Uint8ClampedArray(256*4)，RGBA 顺序 */
export function buildLut(name) {
  const stops = PALETTES[name] || PALETTES.iron
  const lut = new Uint8ClampedArray(256 * 4)
  for (let i = 0; i < 256; i++) {
    const [r, g, b] = sample(stops, i / 255)
    lut[i * 4] = r
    lut[i * 4 + 1] = g
    lut[i * 4 + 2] = b
    lut[i * 4 + 3] = 255
  }
  return lut
}
