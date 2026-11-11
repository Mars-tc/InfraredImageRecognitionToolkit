<script setup>
// Fabric 画布：伪彩渲染（F2）、缩放到像素级（F2）、鼠标取温（F3）、ROI 绘制（F5/F10）
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { Canvas as FabricCanvas, Rect, Polygon, Polyline, Point, Image as FabricImage } from 'fabric'
import { useImageStore } from '../stores/image.js'
import { useRoiStore } from '../stores/roi.js'
import { fmtTemp } from '../utils/format.js'

const imageStore = useImageStore()
const roiStore = useRoiStore()

const wrapEl = ref(null)
const hostEl = ref(null)

let canvasEl = null // 程序化创建，脱离 Vue 管理（Fabric 会移动该节点）
let fabric = null
let baseImage = null
let draftObj = null
let drawingPoints = []
let rectStart = null
let fitScale = 1
let maxZoom = 8
let isPanning = false
let lastPan = null
let spaceDown = false

const cursor = ref({ x: 0, y: 0, show: false, temp: null })
const zoomPct = ref(100)
const canvasSize = ref('0x0')
const fabricSize = ref('0x0')
const iid = Math.random().toString(36).slice(2, 8)

const TOOLS = [
  { key: 'select', label: '选择' },
  { key: 'rect', label: '矩形' },
  { key: 'poly', label: '多边形' },
  { key: 'line', label: '线段' },
  { key: 'pan', label: '平移' }
]

const drawingActive = computed(() => ['poly', 'line'].includes(roiStore.activeTool))

onMounted(() => {
  // Fabric 会把 canvas 元素包进自建容器并移动 DOM 节点，
  // 因此 canvas 必须程序化创建，不能放在 Vue 模板里（否则破坏 Vue 的 patch）
  canvasEl = document.createElement('canvas')
  hostEl.value.appendChild(canvasEl)
  fabric = new FabricCanvas(canvasEl, {
    backgroundColor: '#1f2733',
    preserveObjectStacking: true,
    selection: false,
    stopContextMenu: true,
    fireRightClick: false
  })
  bindEvents()
  // 先按宿主实际尺寸设置画布，再渲染（避免用默认 300x150 计算适配）
  resizeCanvas()
  const ro = new ResizeObserver(resizeCanvas)
  ro.observe(hostEl.value)
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
  // 图片可能已加载（watcher 先于挂载触发），此处补渲染
  if (imageStore.hasImage) renderBase()
})

function resizeCanvas() {
  if (!fabric) return
  const w = hostEl.value?.clientWidth || 0
  const h = hostEl.value?.clientHeight || 0
  canvasSize.value = `${w}x${h}`
  window.__RSZ_LOG__ = window.__RSZ_LOG__ || []
  window.__RSZ_LOG__.push(`${w}x${h}`)
  if (w <= 0 || h <= 0) return
  fabric.setDimensions({ width: w, height: h })
  fabricSize.value = `${fabric.getWidth()}x${fabric.getHeight()}`
  if (baseImage) fitToView()
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
  fabric?.dispose()
  canvasEl?.remove()
  canvasEl = null
})

watch(() => imageStore.temps, () => {
  roiStore.clearAll()
  baseImage = null
  fabric?.clear()
  renderBase()
})
watch(() => [imageStore.palette, imageStore.scaleMin, imageStore.scaleMax], renderBase)
watch(() => roiStore.activeTool, updateCanvasMode)
watch(
  () => roiStore.rois.map((r) => r.id).join(','),
  syncRoiObjects
)

// ---------- 渲染 ----------

function renderBase() {
  if (!fabric || !imageStore.hasImage) return
  const w = imageStore.width
  const h = imageStore.height
  const temps = imageStore.temps
  const lut = imageStore.lut
  const min = imageStore.effectiveMin
  const range = Math.max(1e-6, imageStore.effectiveMax - min)

  const off = document.createElement('canvas')
  off.width = w
  off.height = h
  window.__RSZ_LOG__ = window.__RSZ_LOG__ || []
  window.__RSZ_LOG__.push(`render off=${w}x${h} temps=${temps ? temps.length : 'null'}`)
  const ctx = off.getContext('2d')
  const imgData = ctx.createImageData(w, h)
  const data = imgData.data
  for (let i = 0; i < temps.length; i++) {
    let t = (temps[i] - min) / range
    t = t < 0 ? 0 : t > 1 ? 1 : t
    const idx = (t * 255) | 0
    const p = i * 4
    const q = idx * 4
    data[p] = lut[q]
    data[p + 1] = lut[q + 1]
    data[p + 2] = lut[q + 2]
    data[p + 3] = 255
  }
  ctx.putImageData(imgData, 0, 0)

  if (!baseImage) {
    // Fabric 7.4 默认 origin 变为 center，必须显式指定 left/top 原点
    baseImage = new FabricImage(off, { selectable: false, evented: false, originX: 'left', originY: 'top' })
    fabric.add(baseImage)
    fabric.sendToBack(baseImage)
  } else {
    baseImage.setElement(off)
  }
  fitToView()
  fabric.requestRenderAll()
}

function fitToView() {
  if (!fabric || !imageStore.hasImage) return
  const cw = fabric.getWidth()
  const ch = fabric.getHeight()
  fitScale = Math.min(cw / imageStore.width, ch / imageStore.height) * 0.96
  maxZoom = Math.max(fitScale * 8, 4)
  // 适配并居中：直接计算平移量（zoomToPoint 会保留场景点，不适合作 fit）
  const vpt = [
    fitScale, 0, 0, fitScale,
    (cw - imageStore.width * fitScale) / 2,
    (ch - imageStore.height * fitScale) / 2
  ]
  fabric.setViewportTransform(vpt)
  zoomPct.value = 100
  fabric.requestRenderAll()
}

function applyZoom(z, cx, cy) {
  z = Math.min(maxZoom, Math.max(fitScale, z))
  fabric.zoomToPoint(new Point(cx, cy), z)
  zoomPct.value = Math.round((z / Math.max(fitScale, 0.001)) * 100)
  fabric.requestRenderAll()
}

// ---------- 事件 ----------

function bindEvents() {
  fabric.on('mouse:wheel', (opt) => {
    opt.e.preventDefault()
    opt.e.stopPropagation()
    const rect = hostEl.value.getBoundingClientRect()
    const factor = opt.e.deltaY > 0 ? 0.9 : 1.1
    const z = fabric.getZoom() * factor
    applyZoom(z, opt.e.clientX - rect.left, opt.e.clientY - rect.top)
  })

  fabric.on('mouse:move', (opt) => {
    const rect = hostEl.value.getBoundingClientRect()
    cursor.value.x = opt.e.clientX - rect.left
    cursor.value.y = opt.e.clientY - rect.top
    cursor.value.show = true
    const p = fabric.getScenePoint(opt.e)
    const ix = Math.floor(p.x)
    const iy = Math.floor(p.y)
    imageStore.setHover(ix, iy)
    cursor.value.temp = imageStore.tempAt(ix, iy)

    if (isPanning && lastPan) {
      const vpt = fabric.viewportTransform
      vpt[4] += opt.e.clientX - lastPan.x
      vpt[5] += opt.e.clientY - lastPan.y
      fabric.setViewportTransform(vpt)
      lastPan = { x: opt.e.clientX, y: opt.e.clientY }
    } else if (rectStart && draftObj) {
      const cur = fabric.getScenePoint(opt.e)
      draftObj.set({
        left: Math.min(rectStart.x, cur.x),
        top: Math.min(rectStart.y, cur.y),
        width: Math.abs(cur.x - rectStart.x),
        height: Math.abs(cur.y - rectStart.y)
      })
      fabric.requestRenderAll()
    }
  })

  fabric.on('mouse:down', (opt) => {
    const tool = roiStore.activeTool
    if (opt.e.button === 1 || spaceDown || tool === 'pan') {
      isPanning = true
      lastPan = { x: opt.e.clientX, y: opt.e.clientY }
      fabric.setCursor('grabbing')
      return
    }
    if (tool === 'rect') {
      rectStart = fabric.getScenePoint(opt.e)
      draftObj = new Rect({
        left: rectStart.x,
        top: rectStart.y,
        width: 0,
        height: 0,
        fill: 'transparent',
        stroke: '#00A650',
        strokeWidth: 2,
        strokeDashArray: [4, 3],
        selectable: false,
        evented: false,
        originX: 'left',
        originY: 'top'
      })
      fabric.add(draftObj)
    } else if (tool === 'poly' || tool === 'line') {
      const p = fabric.getScenePoint(opt.e)
      drawingPoints.push([p.x, p.y])
      redrawDraftPath()
    }
  })

  fabric.on('mouse:up', () => {
    if (isPanning) {
      isPanning = false
      lastPan = null
      fabric.setCursor('default')
      return
    }
    if (rectStart && draftObj) {
      const { left, top, width, height } = draftObj
      fabric.remove(draftObj)
      draftObj = null
      rectStart = null
      if (width > 3 && height > 3) {
        roiStore.addRoi('rect', { kind: 'rect', x: left, y: top, w: width, h: height })
      }
    }
  })

  fabric.on('mouse:dblclick', () => {
    if (drawingActive.value) finishDrawing()
  })

  fabric.on('selection:created', (e) => selectByObject(e))
  fabric.on('selection:updated', (e) => selectByObject(e))
  fabric.on('object:modified', (e) => {
    const obj = e.target
    if (obj?.data?.roiId) {
      roiStore.updateShape(obj.data.roiId, shapeOf(obj))
    }
  })
}

function selectByObject(e) {
  const obj = e.selected?.[0]
  if (obj?.data?.roiId) roiStore.selectRoi(obj.data.roiId)
}

function onKeyDown(e) {
  if (e.code === 'Space') spaceDown = true
  if (e.key === 'Enter' && drawingActive.value) finishDrawing()
  if (e.key === 'Escape') cancelDrawing()
  if ((e.key === 'Delete' || e.key === 'Backspace') && roiStore.selectedId) {
    removeRoiObject(roiStore.selectedId)
    roiStore.removeRoi(roiStore.selectedId)
  }
}

function onKeyUp(e) {
  if (e.code === 'Space') spaceDown = false
}

// ---------- ROI 对象同步 ----------

function updateCanvasMode() {
  if (!fabric) return
  const selectable = roiStore.activeTool === 'select'
  fabric.selection = selectable
  fabric.forEachObject((o) => {
    if (o.data?.roiId) {
      o.selectable = selectable
      o.evented = selectable
    }
  })
  fabric.defaultCursor =
    roiStore.activeTool === 'pan' ? 'grab' : selectable ? 'default' : 'crosshair'
  fabric.requestRenderAll()
}

function syncRoiObjects() {
  if (!fabric) return
  const existing = new Map()
  fabric.forEachObject((o) => {
    if (o.data?.roiId) existing.set(o.data.roiId, o)
  })
  for (const roi of roiStore.rois) {
    if (existing.has(roi.id)) {
      existing.delete(roi.id)
      continue
    }
    fabric.add(makeRoiObject(roi))
  }
  // 移除已删除的
  for (const [, obj] of existing) fabric.remove(obj)
  updateCanvasMode()
  fabric.requestRenderAll()
}

function makeRoiObject(roi) {
  const common = {
    stroke: roi.color,
    strokeWidth: 2,
    fill: 'transparent',
    selectable: roiStore.activeTool === 'select',
    evented: roiStore.activeTool === 'select',
    // Fabric 7.4 默认 origin 为 center，统一显式用 left/top
    originX: 'left',
    originY: 'top',
    data: { roiId: roi.id }
  }
  if (roi.kind === 'rect') {
    return new Rect({ ...common, left: roi.shape.x, top: roi.shape.y, width: roi.shape.w, height: roi.shape.h })
  }
  const pts = roi.shape.points.map(([x, y]) => ({ x, y }))
  return roi.kind === 'poly'
    ? new Polygon(pts, { ...common, objectCaching: false })
    : new Polyline(pts, { ...common, fill: 'transparent', objectCaching: false })
}

function shapeOf(obj) {
  if (obj.type === 'rect') {
    return { kind: 'rect', x: obj.left, y: obj.top, w: obj.width * obj.scaleX, h: obj.height * obj.scaleY }
  }
  const pts = (obj.points || []).map((p) => [p.x, p.y])
  return { kind: obj.type === 'polygon' ? 'poly' : 'line', points: pts }
}

function removeRoiObject(id) {
  fabric.forEachObject((o) => {
    if (o.data?.roiId === id) fabric.remove(o)
  })
}

// ---------- 绘制中状态 ----------

function redrawDraftPath() {
  if (draftObj) fabric.remove(draftObj)
  if (drawingPoints.length === 0) return
  const pts = drawingPoints.map(([x, y]) => ({ x, y }))
  const common = {
    stroke: '#00A650',
    strokeWidth: 2,
    fill: 'transparent',
    selectable: false,
    evented: false,
    objectCaching: false,
    originX: 'left',
    originY: 'top'
  }
  draftObj =
    roiStore.activeTool === 'poly' && drawingPoints.length >= 3
      ? new Polygon(pts, common)
      : new Polyline(pts, common)
  fabric.add(draftObj)
  fabric.requestRenderAll()
}

function finishDrawing() {
  const kind = roiStore.activeTool === 'poly' ? 'poly' : 'line'
  const need = kind === 'poly' ? 3 : 2
  if (drawingPoints.length >= need) {
    roiStore.addRoi(kind, { kind, points: [...drawingPoints] })
  }
  cancelDrawing()
}

function cancelDrawing() {
  if (draftObj) fabric.remove(draftObj)
  draftObj = null
  drawingPoints = []
  roiStore.drawing = false
  fabric?.requestRenderAll()
}

watch(drawingActive, (v) => {
  if (!v) cancelDrawing()
})

defineExpose({ fitToView, getFabric: () => fabric, iid })
</script>

<template>
  <div class="canvas-view" ref="wrapEl">
    <div class="canvas-host" ref="hostEl">
      <div v-if="cursor.show" class="crosshair-v" :style="{ left: cursor.x + 'px' }" />
      <div v-if="cursor.show" class="crosshair-h" :style="{ top: cursor.y + 'px' }" />
      <div
        v-if="cursor.show && cursor.temp !== null"
        class="cursor-tip"
        :style="{ left: cursor.x + 12 + 'px', top: cursor.y + 12 + 'px' }"
      >
        {{ fmtTemp(cursor.temp) }}
      </div>

      <div class="canvas-tools">
        <el-button-group>
          <el-button
            v-for="t in TOOLS"
            :key="t.key"
            size="small"
            :type="roiStore.activeTool === t.key ? 'primary' : 'default'"
            @click="roiStore.setTool(t.key)"
          >
            {{ t.label }}
          </el-button>
        </el-button-group>
        <template v-if="drawingActive">
          <el-button size="small" type="success" @click="finishDrawing">完成</el-button>
          <el-button size="small" @click="cancelDrawing">取消</el-button>
          <span class="draw-hint">单击添加点，双击或回车完成</span>
        </template>
      </div>

      <div class="zoom-badge" style="top: 48px; right: 10px; bottom: auto; font-size: 14px; padding: 6px 12px">
        <span>[{{ iid }}] host={{ canvasSize }} canvas={{ fabricSize }} zoom={{ zoomPct }}%</span>
        <el-button size="small" text @click="fitToView">适应窗口</el-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.canvas-view {
  width: 100%;
  height: 100%;
}

.canvas-host {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.canvas-host canvas {
  display: block;
}

.crosshair-v,
.crosshair-h {
  position: absolute;
  background: rgba(255, 255, 255, 0.45);
  pointer-events: none;
}

.crosshair-v {
  top: 0;
  bottom: 0;
  width: 1px;
  z-index: 3;
}

.crosshair-h {
  left: 0;
  right: 0;
  height: 1px;
  z-index: 3;
}

.cursor-tip {
  position: absolute;
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(31, 39, 51, 0.85);
  color: #fff;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
  white-space: nowrap;
  z-index: 4;
}

.canvas-tools {
  position: absolute;
  top: 10px;
  left: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  z-index: 5;
}

.draw-hint {
  padding: 3px 10px;
  border-radius: 4px;
  background: rgba(31, 39, 51, 0.75);
  color: #dfe6ee;
  font-size: 12px;
}

.zoom-badge {
  position: absolute;
  right: 10px;
  bottom: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 10px;
  border-radius: 6px;
  background: rgba(31, 39, 51, 0.75);
  color: #dfe6ee;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  z-index: 5;
}

.zoom-badge :deep(.el-button) {
  color: #dfe6ee;
}
</style>
