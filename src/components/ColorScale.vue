<script setup>
// F4：调色板切换 + 色标上下限（色标缩放）
import { ref, watch, onMounted, nextTick } from 'vue'
import { useImageStore } from '../stores/image.js'
import { PALETTE_OPTIONS } from '../utils/palettes.js'
import { fmtTemp } from '../utils/format.js'

const imageStore = useImageStore()
const barEl = ref(null)
const minInput = ref(null)
const maxInput = ref(null)

watch(
  () => imageStore.lut,
  drawBar,
  { deep: false }
)
watch(() => [imageStore.effectiveMin, imageStore.effectiveMax], syncInputs)
onMounted(async () => {
  await nextTick()
  drawBar()
  syncInputs()
})

function syncInputs() {
  minInput.value = Number(imageStore.effectiveMin.toFixed(1))
  maxInput.value = Number(imageStore.effectiveMax.toFixed(1))
}

function drawBar() {
  const el = barEl.value
  if (!el) return
  const ctx = el.getContext('2d')
  const w = el.width
  const h = el.height
  const lut = imageStore.lut
  for (let x = 0; x < w; x++) {
    const idx = Math.round((x / (w - 1)) * 255) * 4
    ctx.fillStyle = `rgb(${lut[idx]},${lut[idx + 1]},${lut[idx + 2]})`
    ctx.fillRect(x, 0, 1, h)
  }
}

function applyScale() {
  const min = Number(minInput.value)
  const max = Number(maxInput.value)
  if (!Number.isFinite(min) || !Number.isFinite(max) || min >= max) return
  imageStore.setScale(min, max)
}

function autoScale() {
  imageStore.autoScale()
  syncInputs()
}
</script>

<template>
  <div class="color-scale">
    <div class="scale-header">
      <span class="scale-title">调色板</span>
      <el-select v-model="imageStore.palette" size="small" style="width: 96px" @change="imageStore.setPalette">
        <el-option v-for="p in PALETTE_OPTIONS" :key="p.value" :label="p.label" :value="p.value" />
      </el-select>
    </div>

    <canvas ref="barEl" class="scale-bar" width="260" height="14" />

    <div class="scale-labels">
      <span>{{ fmtTemp(imageStore.effectiveMax) }}</span>
      <span class="scale-mid">色标上下限</span>
      <span>{{ fmtTemp(imageStore.effectiveMin) }}</span>
    </div>

    <div class="scale-inputs">
      <el-input-number v-model="minInput" size="small" :controls="false" :step="0.5" style="width: 84px" @change="applyScale" />
      <span class="tilde">~</span>
      <el-input-number v-model="maxInput" size="small" :controls="false" :step="0.5" style="width: 84px" @change="applyScale" />
      <el-button size="small" @click="autoScale">自动</el-button>
    </div>
    <p class="scale-tip">超出上下限的温度将钳位到端点色</p>
  </div>
</template>

<style scoped>
.color-scale {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.scale-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.scale-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--ip-text);
}

.scale-bar {
  width: 100%;
  height: 14px;
  border-radius: 3px;
  border: 1px solid var(--ip-border);
}

.scale-labels {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--ip-text-secondary);
  font-variant-numeric: tabular-nums;
}

.scale-mid {
  color: var(--ip-text-muted);
}

.scale-inputs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tilde {
  color: var(--ip-text-muted);
}

.scale-tip {
  margin: 0;
  font-size: 11px;
  color: var(--ip-text-muted);
}
</style>
