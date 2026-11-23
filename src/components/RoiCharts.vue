<script setup>
// ROI 图表：线段温度剖面 + 温度直方图（纯 SVG，无第三方依赖）
import { computed } from 'vue'
import { fmtTemp } from '../utils/format.js'

const props = defineProps({
  profile: { type: Array, default: null },
  histogram: { type: Object, default: null }
})

const W = 260
const H = 84
const PAD = 18

const profileGeom = computed(() => {
  const values = props.profile
  if (!Array.isArray(values) || values.length < 2) return null
  const nums = values.filter((v) => Number.isFinite(v))
  if (nums.length < 2) return null
  const min = Math.min(...nums)
  const max = Math.max(...nums)
  const span = max - min || 1
  const innerW = W - PAD * 2
  const innerH = H - PAD * 2
  const points = values
    .map((t, i) => {
      const x = PAD + (i / (values.length - 1)) * innerW
      const y = PAD + innerH - ((Number.isFinite(t) ? t : min) - min) / span * innerH
      return `${x.toFixed(1)},${y.toFixed(1)}`
    })
    .join(' ')
  return { min, max, points }
})

const histogramGeom = computed(() => {
  const h = props.histogram
  if (!h || !Array.isArray(h.counts) || !h.counts.length) return null
  const maxCount = Math.max(...h.counts, 1)
  const innerW = W - PAD * 2
  const innerH = H - PAD * 2
  const gap = 1
  const bw = Math.max(1, (innerW - gap * (h.counts.length - 1)) / h.counts.length)
  const bars = h.counts.map((c, i) => {
    const bh = (c / maxCount) * innerH
    return { x: PAD + i * (bw + gap), y: PAD + innerH - bh, w: bw, h: bh }
  })
  return { min: h.min, max: h.max, bars }
})
</script>

<template>
  <div class="roi-charts">
    <div v-if="profileGeom" class="chart-block">
      <span class="chart-title">温度剖面（沿线段）</span>
      <svg :viewBox="`0 0 ${W} ${H}`" class="chart" role="img" aria-label="线段温度剖面">
        <rect :x="PAD" :y="PAD" :width="W - PAD * 2" :height="H - PAD * 2" class="plot-bg" />
        <polyline :points="profileGeom.points" class="profile-line" />
      </svg>
      <div class="chart-axis">
        <span>{{ fmtTemp(profileGeom.max) }}</span>
        <span>{{ fmtTemp(profileGeom.min) }}</span>
      </div>
    </div>

    <div v-if="histogramGeom" class="chart-block">
      <span class="chart-title">温度分布（16 桶）</span>
      <svg :viewBox="`0 0 ${W} ${H}`" class="chart" role="img" aria-label="ROI 温度直方图">
        <rect :x="PAD" :y="PAD" :width="W - PAD * 2" :height="H - PAD * 2" class="plot-bg" />
        <rect
          v-for="(b, i) in histogramGeom.bars"
          :key="i"
          :x="b.x"
          :y="b.y"
          :width="b.w"
          :height="b.h"
          class="hist-bar"
        />
      </svg>
      <div class="chart-axis">
        <span>{{ fmtTemp(histogramGeom.max) }}</span>
        <span>{{ fmtTemp(histogramGeom.min) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.roi-charts {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}

.chart-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.chart-title {
  font-size: 11px;
  color: var(--ip-text-secondary);
}

.chart {
  width: 100%;
  height: auto;
}

.plot-bg {
  fill: rgba(31, 39, 51, 0.04);
  stroke: var(--ip-border);
  stroke-width: 0.8;
}

.profile-line {
  fill: none;
  stroke: var(--ip-primary);
  stroke-width: 1.4;
  vector-effect: non-scaling-stroke;
}

.hist-bar {
  fill: rgba(0, 166, 80, 0.65);
}

.chart-axis {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: var(--ip-text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
