// 当前图片状态：温度矩阵、元数据、调色板与色标、缩放平移
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '../api/tauri.js'
import { buildLut } from '../utils/palettes.js'

export const useImageStore = defineStore('image', () => {
  const meta = ref(null)
  const temps = ref(null) // Float32Array（行主序 w×h）
  const width = ref(0)
  const height = ref(0)

  const palette = ref('iron')
  const scaleMin = ref(null) // null = 自动（用矩阵实际范围）
  const scaleMax = ref(null)
  const zoom = ref(1)
  const panX = ref(0)
  const panY = ref(0)

  const loading = ref(false)
  const error = ref('')
  const hover = ref(null) // { x, y, temp }
  const extrema = ref(null) // 全图极值 { max, min, max_pos, min_pos }
  const showTempMarks = ref(true) // 画布温度标注总开关（会话内有效，不落盘）

  const lut = computed(() => buildLut(palette.value))

  const hasImage = computed(() => !!temps.value)

  const effectiveMin = computed(() =>
    scaleMin.value !== null ? scaleMin.value : meta.value?.temp_min ?? 0
  )
  const effectiveMax = computed(() =>
    scaleMax.value !== null ? scaleMax.value : meta.value?.temp_max ?? 1
  )

  function tempAt(x, y) {
    if (!temps.value) return null
    const ix = Math.floor(x)
    const iy = Math.floor(y)
    if (ix < 0 || iy < 0 || ix >= width.value || iy >= height.value) return null
    return temps.value[iy * width.value + ix]
  }

  // 全图极值由后端一次扫描产出并缓存；失败时静默降级为「无全图标注」
  async function refreshExtrema() {
    try {
      extrema.value = await api.imageExtrema()
    } catch (e) {
      extrema.value = null
      console.error('读取全图极值失败', e)
    }
  }

  function toggleTempMarks() {
    showTempMarks.value = !showTempMarks.value
  }

  async function load(path, distance = null) {
    loading.value = true
    error.value = ''
    try {
      const m = await api.loadImage(path, distance)
      meta.value = m
      width.value = m.width
      height.value = m.height
      const t = await api.getTemps()
      temps.value = t
      await refreshExtrema()
      // 新图片重置色标与视图
      scaleMin.value = null
      scaleMax.value = null
      zoom.value = 1
      panX.value = 0
      panY.value = 0
    } catch (e) {
      error.value = typeof e === 'string' ? e : e?.message || String(e)
      meta.value = null
      temps.value = null
      extrema.value = null
    } finally {
      loading.value = false
    }
  }

  // 开发/自验钩子：URL 带 ?img=绝对路径 或 vite 环境变量 VITE_AUTO_LOAD_IMG 时自动加载
  function autoLoadFromUrl() {
    const params = new URLSearchParams(window.location.search)
    const img = params.get('img') || import.meta.env.VITE_AUTO_LOAD_IMG || ''
    if (img) load(img)
  }

  async function setPalette(name) {
    palette.value = name
  }

  function setScale(min, max) {
    scaleMin.value = min
    scaleMax.value = max
  }

  function autoScale() {
    scaleMin.value = null
    scaleMax.value = null
  }

  async function changeEmissivity(value) {
    try {
      meta.value = await api.setEmissivity(value)
      temps.value = await api.getTemps()
      await refreshExtrema()
    } catch (e) {
      error.value = typeof e === 'string' ? e : e?.message || String(e)
    }
  }

  async function changeDistance(value) {
    try {
      meta.value = await api.setDistance(value)
      temps.value = await api.getTemps()
      await refreshExtrema()
    } catch (e) {
      error.value = typeof e === 'string' ? e : e?.message || String(e)
    }
  }

  function setHover(x, y) {
    const t = tempAt(x, y)
    hover.value = { x, y, temp: t }
  }

  function clearHover() {
    hover.value = null
  }

  function reset() {
    meta.value = null
    temps.value = null
    width.value = 0
    height.value = 0
    error.value = ''
    hover.value = null
    extrema.value = null
  }

  return {
    meta, temps, width, height,
    palette, scaleMin, scaleMax, zoom, panX, panY,
    loading, error, hover, extrema, showTempMarks, lut, hasImage, effectiveMin, effectiveMax,
    tempAt, load, setPalette, setScale, autoScale,
    changeEmissivity, changeDistance, setHover, clearHover, reset, autoLoadFromUrl,
    refreshExtrema, toggleTempMarks
  }
})
