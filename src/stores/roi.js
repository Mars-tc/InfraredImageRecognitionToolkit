// ROI 状态：绘制工具、ROI 列表、后端统计与定级结果、三相比对告警
import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '../api/tauri.js'

let seq = 0
const nextId = () => `roi_${Date.now().toString(36)}_${seq++}`

export const ROI_COLORS = ['#00A650', '#5B8DEF', '#F2B705', '#E02020', '#9B59B6', '#00BCD4']

export const useRoiStore = defineStore('roi', () => {
  const rois = ref([])
  const activeTool = ref('select') // select | rect | poly | line | pan
  const selectedId = ref(null)
  const drawing = ref(false) // 是否正在绘制（poly/line 未完成）
  const phaseAlert = ref(null) // F11 三相比对告警

  function setTool(tool) {
    activeTool.value = tool
    drawing.value = false
  }

  function addRoi(kind, shape, color) {
    const roi = {
      id: nextId(),
      kind,
      shape,
      label: `ROI-${rois.value.length + 1}`,
      phase: '', // 相别标记（A/B/C/N），三相比对用
      color: color || ROI_COLORS[rois.value.length % ROI_COLORS.length],
      stats: null, // { max, min, avg, delta_t, count }
      verdict: null, // { level, reason, exceeded }
      error: ''
    }
    rois.value.push(roi)
    selectedId.value = roi.id
    refreshStats(roi.id)
    return roi
  }

  function removeRoi(id) {
    const i = rois.value.findIndex((r) => r.id === id)
    if (i >= 0) rois.value.splice(i, 1)
    if (selectedId.value === id) selectedId.value = null
    comparePhases()
  }

  function clearAll() {
    rois.value = []
    selectedId.value = null
    drawing.value = false
    phaseAlert.value = null
  }

  function selectRoi(id) {
    selectedId.value = id
  }

  function updateShape(id, shape) {
    const roi = rois.value.find((r) => r.id === id)
    if (roi) {
      roi.shape = shape
      refreshStats(id)
    }
  }

  function setPhase(id, phase) {
    const roi = rois.value.find((r) => r.id === id)
    if (roi) {
      roi.phase = phase
      comparePhases()
    }
  }

  async function refreshStats(id) {
    const roi = rois.value.find((r) => r.id === id)
    if (!roi) return
    try {
      const r = await api.roiStats(roi.shape)
      roi.stats = { max: r.max, min: r.min, avg: r.avg, delta_t: r.delta_t, count: r.count }
      roi.verdict = r.verdict || null
      roi.error = ''
    } catch (e) {
      roi.stats = null
      roi.verdict = null
      roi.error = typeof e === 'string' ? e : e?.message || String(e)
    }
    comparePhases()
  }

  async function refreshAll() {
    await Promise.all(rois.value.map((r) => refreshStats(r.id)))
  }

  /// F11：A/B/C 相间最大温差比对（阈值由后端配置）
  async function comparePhases() {
    const entries = rois.value
      .filter((r) => r.phase && r.stats)
      .map((r) => [r.phase, r.stats.max])
    if (entries.length < 2) {
      phaseAlert.value = null
      return
    }
    try {
      phaseAlert.value = await api.phaseCompare(entries)
    } catch (e) {
      console.error('三相比对失败', e)
    }
  }

  return {
    rois, activeTool, selectedId, drawing, phaseAlert,
    setTool, addRoi, removeRoi, clearAll, selectRoi, updateShape, setPhase,
    refreshStats, refreshAll, comparePhases
  }
})
