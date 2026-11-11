<script setup>
// Fabric 画布：伪彩渲染（F2）、缩放到像素级（F2）、鼠标取温（F3）、ROI 绘制（F5/F10）
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { Canvas as FabricCanvas, Rect, Polygon, Polyline, Point, Image as FabricImage } from 'fabric'
import { useImageStore } from '../stores/image.js'
import { useRoiStore } from '../stores/roi.js'
import { fmtTemp } from '../utils/format.js'
import { shapeOf, hexToRgba, roiTargetsOf } from '../utils/roiShape.js'
import { buildTempMarks } from '../utils/tempMarks.js'

const imageStore = useImageStore()
const roiStore = useRoiStore()

const wrapEl = ref(null)
const hostEl = ref(null)

let canvasEl = null // 程序化创建，脱离 Vue 管理（Fabric 会移动该节点）
let fabric = null
let overlayEl = null // 独立的标注画布（高分辨率超采样，文字不随主画布发虚）
let overlayCtx = null
let overlayDpr = 1
let baseImage = null
let draftObj = null // 仅矩形拖拽预览使用
let rectStart = null
const drawingPoints = ref([]) // 多边形/线段顶点（场景坐标），引导层据此绘制
let hoverScene = null // 绘制中鼠标所在的场景坐标
let fitScale = 1
let maxZoom = 8
let isPanning = false
let lastPan = null
let spaceDown = false
let resizeObserver = null
let movingTimer = null

const cursor = ref({ x: 0, y: 0, show: false, temp: null })
const zoomPct = ref(100)
const TOOLS = [
  { key: 'select', label: '选择' },
  { key: 'rect', label: '矩形' },
  { key: 'poly', label: '多边形' },
  { key: 'line', label: '线段' },
  { key: 'pan', label: '平移' }
]

// 独立标注层按 2x 物理像素渲染，用常规字重避免部分 WebView 合成粗体发虚
const OVERLAY_FONT = '12px system-ui, sans-serif'
const TOOLBAR_BOTTOM = 44 // 工具栏是 HTML 覆盖层，标签贴顶会被遮住

const drawingActive = computed(() => ['poly', 'line'].includes(roiStore.activeTool))
const drawHint = computed(() => {
  const need = roiStore.activeTool === 'poly' ? 3 : 2
  const left = Math.max(0, need - drawingPoints.value.length)
  const tip = left > 0 ? `还需 ${left} 个点` : '双击 / 回车 / 点起点完成'
  return `${drawingPoints.value.length} 个点 · 单击加点 · ${tip} · Backspace 撤销`
})

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
  // 标注文字改画在独立画布上：Fabric 主画布的像素缓冲区在部分 WebView
  // 下只有 CSS 分辨率，浏览器缩放时会把整块位图（含文字）拉伸发虚。
  // 独立层强制 2x 超采样，文字始终按物理像素矢量渲染。
  overlayEl = document.createElement('canvas')
  // scoped 样式无法命中程序化创建的节点，直接用内联样式
  overlayEl.style.cssText =
    'position:absolute;top:0;left:0;display:block;pointer-events:none;z-index:3;'
  hostEl.value.appendChild(overlayEl)
  overlayCtx = overlayEl.getContext('2d')
  bindEvents()
  // 先按宿主实际尺寸设置画布，再渲染（避免用默认 300x150 计算适配）
  resizeCanvas()
  resizeObserver = new ResizeObserver(resizeCanvas)
  resizeObserver.observe(hostEl.value)
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
  // 图片可能已加载（watcher 先于挂载触发），此处补渲染
  if (imageStore.hasImage) renderBase()
})

function resizeCanvas() {
  if (!fabric) return
  const w = hostEl.value?.clientWidth || 0
  const h = hostEl.value?.clientHeight || 0
  if (w <= 0 || h <= 0) return
  fabric.setDimensions({ width: w, height: h })
  resizeOverlay(w, h)
  if (baseImage) fitToView()
}

function resizeOverlay(w, h) {
  if (!overlayEl || !overlayCtx) return
  // 下限 2 倍：即使 WebView 上报的 DPR 不准，也有足够的物理像素
  overlayDpr = Math.max(2, window.devicePixelRatio || 1)
  overlayEl.width = Math.round(w * overlayDpr)
  overlayEl.height = Math.round(h * overlayDpr)
  overlayEl.style.width = `${w}px`
  overlayEl.style.height = `${h}px`
  drawOverlay()
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
  resizeObserver?.disconnect()
  if (movingTimer) clearTimeout(movingTimer)
  fabric?.dispose()
  canvasEl?.remove()
  canvasEl = null
  overlayEl?.remove()
  overlayEl = null
  overlayCtx = null
})

watch(() => imageStore.temps, () => {
  roiStore.clearAll()
  baseImage = null
  fabric?.clear()
  renderBase()
})
watch(() => [imageStore.palette, imageStore.scaleMin, imageStore.scaleMax], renderBase)
watch(() => roiStore.activeTool, (tool) => {
  // 矩形拖拽到一半就切工具时会留下草稿框，先清理
  if (tool !== 'rect') cancelRectDraft()
  updateCanvasMode()
})
watch(
  () => roiStore.rois.map((r) => r.id).join(','),
  syncRoiObjects
)
// 覆盖层是普通函数，不会建立 Vue 渲染依赖：标注相关状态变化后显式请求重绘
watch(
  () => [
    imageStore.showTempMarks,
    imageStore.extrema,
    roiStore.selectedId,
    roiStore.rois
      .map((r) => `${r.stats?.max_pos?.join(',') ?? ''}|${r.stats?.min_pos?.join(',') ?? ''}`)
      .join(';')
  ],
  () => fabric?.requestRenderAll()
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
    // Fabric 6+ 起画布堆叠方法更名为 sendObjectToBack（旧版为 sendToBack）
    fabric.sendObjectToBack(baseImage)
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
      draftObj.set({
        left: Math.min(rectStart.x, p.x),
        top: Math.min(rectStart.y, p.y),
        width: Math.abs(p.x - rectStart.x),
        height: Math.abs(p.y - rectStart.y)
      })
      fabric.requestRenderAll()
    } else if (drawingActive.value) {
      // 引导层绘制在 after:render 中，跟随鼠标刷新预览线
      hoverScene = { x: p.x, y: p.y }
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
      const pts = drawingPoints.value
      const px = (v) => v / Math.max(fabric.getZoom(), 0.001) // 屏幕像素 -> 场景距离
      const last = pts[pts.length - 1]
      // 双击的第二次点击会先触发 mouse:down，忽略重合点避免多出重复顶点
      if (last && Math.hypot(p.x - last[0], p.y - last[1]) < px(4)) return
      // 靠近起点即闭合（>=3 点才构成多边形）
      const first = pts[0]
      if (tool === 'poly' && pts.length >= 3 && Math.hypot(p.x - first[0], p.y - first[1]) < px(10)) {
        finishDrawing()
        return
      }
      pts.push([p.x, p.y])
      roiStore.drawing = true
      hoverScene = { x: p.x, y: p.y }
      fabric.requestRenderAll()
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

  // 拖动/缩放过程中节流刷新统计；框选多个 ROI 时事件目标是 ActiveSelection，
  // 需展开成子对象逐个同步，否则整批 ROI 的形状与统计都不会更新
  fabric.on('object:moving', (e) => scheduleShapeRefresh(e.target))
  fabric.on('object:scaling', (e) => scheduleShapeRefresh(e.target))
  fabric.on('object:modified', (e) => {
    if (movingTimer) {
      clearTimeout(movingTimer)
      movingTimer = null
    }
    commitShapes(roiTargetsOf(e.target))
  })

  // 标注层画在独立画布上；renderTop 也会触发 after:render，跳过它避免重复绘制
  fabric.on('after:render', ({ ctx }) => {
    if (ctx === fabric.getContext()) drawOverlay()
  })
}

function scheduleShapeRefresh(target) {
  const objects = roiTargetsOf(target)
  if (!objects.length || movingTimer) return
  movingTimer = setTimeout(() => {
    movingTimer = null
    commitShapes(objects)
  }, 120)
}

function commitShapes(objects) {
  for (const obj of objects) {
    roiStore.updateShape(obj.data.roiId, shapeOf(obj))
  }
}

function selectByObject(e) {
  const obj = e.selected?.[0]
  if (obj?.data?.roiId) roiStore.selectRoi(obj.data.roiId)
}

function onToggleTempMarks() {
  imageStore.toggleTempMarks()
  fabric?.requestRenderAll()
}

function onKeyDown(e) {
  // 输入框内的按键交给控件自己处理，避免退格/回车误删 ROI 或结束绘制
  const tag = e.target?.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || e.target?.isContentEditable) return
  if (e.code === 'Space') spaceDown = true
  if (e.key === 'Enter' && drawingActive.value) finishDrawing()
  if (e.key === 'Escape') cancelDrawing()
  // 绘制中 Backspace 撤销上一个顶点，其余情况用于删除选中 ROI
  if (e.key === 'Backspace' && drawingActive.value && drawingPoints.value.length) {
    e.preventDefault()
    drawingPoints.value.pop()
    fabric?.requestRenderAll()
    return
  }
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
    strokeUniform: true, // 线宽固定在屏幕像素，缩放时不会变粗/变细
    // 多边形用同色淡填充标出测温区域，线段保持无填充
    fill: roi.kind === 'poly' ? hexToRgba(roi.color, 0.14) : 'transparent',
    selectable: roiStore.activeTool === 'select',
    evented: roiStore.activeTool === 'select',
    // Fabric 7.4 默认 origin 为 center，统一显式用 left/top
    originX: 'left',
    originY: 'top',
    data: { roiId: roi.id, roiKind: roi.kind }
  }
  if (roi.kind === 'rect') {
    return new Rect({ ...common, left: roi.shape.x, top: roi.shape.y, width: roi.shape.w, height: roi.shape.h })
  }
  const pts = roi.shape.points.map(([x, y]) => ({ x, y }))
  return roi.kind === 'poly'
    ? new Polygon(pts, { ...common, objectCaching: false })
    : new Polyline(pts, { ...common, objectCaching: false })
}

function removeRoiObject(id) {
  fabric.forEachObject((o) => {
    if (o.data?.roiId === id) fabric.remove(o)
  })
}

// ---------- 绘制引导层 / 绘制中状态 ----------

// after:render 时手动套用 viewport 变换即可得到屏幕像素坐标，
// 这样顶点手柄与线宽不随画布缩放变形。
function sceneToScreen(vpt, p) {
  return { x: vpt[0] * p.x + vpt[2] * p.y + vpt[4], y: vpt[1] * p.x + vpt[3] * p.y + vpt[5] }
}

function drawOverlay() {
  if (!fabric || !overlayCtx) return
  const ctx = overlayCtx
  ctx.save()
  // 标注层自己的坐标系是 CSS 像素，物理缓冲区按 overlayDpr 放大，
  // 因此文字始终以矢量字形渲染到足量物理像素，不会被浏览器拉伸放大。
  ctx.setTransform(overlayDpr, 0, 0, overlayDpr, 0, 0)
  ctx.clearRect(0, 0, fabric.getWidth(), fabric.getHeight())
  const toScreen = (p) => sceneToScreen(fabric.viewportTransform, p)
  drawRoiLabels(ctx, toScreen)
  if (drawingActive.value && drawingPoints.value.length) drawDraft(ctx, toScreen)
  drawTempMarks(ctx, toScreen)
  ctx.restore()
}

function drawRoiLabels(ctx, toScreen) {
  ctx.font = OVERLAY_FONT
  ctx.textBaseline = 'middle'
  const ch = fabric.getHeight()
  for (const roi of roiStore.rois) {
    const obj = fabric.getObjects().find((o) => o.data?.roiId === roi.id)
    if (!obj) continue
    const shape = shapeOf(obj)
    let x, y
    if (shape.kind === 'rect') {
      x = shape.x
      y = shape.y
    } else {
      if (!shape.points.length) continue
      x = Math.min(...shape.points.map((p) => p[0]))
      y = Math.min(...shape.points.map((p) => p[1]))
    }
    const p = toScreen({ x, y })
    const w = Math.ceil(ctx.measureText(roi.label).width) + 18
    const h = 22
    const bx = Math.round(p.x)
    let by = p.y - h - 4
    // 贴顶的标签会被工具栏遮住，改放到框角下方
    if (by < TOOLBAR_BOTTOM) by = p.y + 6
    by = Math.round(Math.min(Math.max(0, by), Math.max(0, ch - h)))
    // 整数像素坐标 + 不透明底色，避免边缘抗锯齿导致文字发虚
    ctx.fillStyle = 'rgba(16, 22, 30, 0.94)'
    ctx.fillRect(bx, by, w, h)
    ctx.fillStyle = roi.color
    ctx.fillRect(bx, by, 3, h)
    ctx.fillStyle = '#ffffff'
    ctx.fillText(roi.label, bx + 9, by + h / 2)

    // 选中多边形/线段时标出顶点，便于辨认与拖动
    if (roi.id === roiStore.selectedId && shape.kind !== 'rect') {
      for (const [px, py] of shape.points) {
        const c = toScreen({ x: px, y: py })
        ctx.beginPath()
        ctx.arc(Math.round(c.x), Math.round(c.y), 3, 0, Math.PI * 2)
        ctx.fillStyle = '#ffffff'
        ctx.fill()
        ctx.lineWidth = 1.5
        ctx.strokeStyle = roi.color
        ctx.stroke()
      }
    }
  }
}

/** 温度极值标注：全图最高/最低 + 选中 ROI 的最高/最低 */
function drawTempMarks(ctx, toScreen) {
  if (!imageStore.showTempMarks) return
  const marks = buildTempMarks({
    extrema: imageStore.extrema,
    rois: roiStore.rois,
    selectedId: roiStore.selectedId
  })
  if (!marks.length) return

  const cw = fabric.getWidth()
  const ch = fabric.getHeight()
  const placed = []
  ctx.font = OVERLAY_FONT
  ctx.textBaseline = 'middle'
  ctx.setLineDash([])

  for (const m of marks) {
    const s = toScreen({ x: m.x + 0.5, y: m.y + 0.5 })
    const p = { x: Math.round(s.x), y: Math.round(s.y) }
    // 准星：圆环 + 断开的十字，中心留空不遮挡热点像素
    ctx.strokeStyle = m.color
    ctx.lineWidth = 1.5
    ctx.beginPath()
    ctx.arc(p.x, p.y, 6, 0, Math.PI * 2)
    ctx.stroke()
    ctx.beginPath()
    ctx.moveTo(p.x - 11, p.y)
    ctx.lineTo(p.x - 3, p.y)
    ctx.moveTo(p.x + 3, p.y)
    ctx.lineTo(p.x + 11, p.y)
    ctx.moveTo(p.x, p.y - 11)
    ctx.lineTo(p.x, p.y - 3)
    ctx.moveTo(p.x, p.y + 3)
    ctx.lineTo(p.x, p.y + 11)
    ctx.stroke()

    // 标签：整数像素坐标 + 不透明底色，钳制在画布内并避开工具栏与已有标签
    const bw = Math.ceil(ctx.measureText(m.label).width) + 20
    const bh = 24
    const x = Math.round(Math.min(Math.max(0, p.x + 12), Math.max(0, cw - bw)))
    let rawY = p.y - bh - 8
    if (rawY < TOOLBAR_BOTTOM) rawY = p.y + 12
    let y = Math.round(Math.min(Math.max(0, rawY), Math.max(0, ch - bh)))
    for (let i = 0; i < 4; i++) {
      const hit = placed.some(
        (r) => x < r.x + r.w && x + bw > r.x && y < r.y + r.h && y + bh > r.y
      )
      if (!hit) break
      y = Math.min(y + bh + 4, Math.max(0, ch - bh))
    }
    placed.push({ x, y, w: bw, h: bh })

    ctx.fillStyle = 'rgba(16, 22, 30, 0.94)'
    ctx.fillRect(x, y, bw, bh)
    ctx.fillStyle = m.color
    ctx.fillRect(x, y, 3, bh)
    ctx.fillStyle = '#ffffff'
    ctx.fillText(m.label, x + 10, y + bh / 2)
  }
}

function drawDraft(ctx, toScreen) {
  const pts = drawingPoints.value.map(([x, y]) => {
    const s = toScreen({ x, y })
    return { x: Math.round(s.x), y: Math.round(s.y) }
  })
  const hs = hoverScene ? toScreen(hoverScene) : null
  const hover = hs ? { x: Math.round(hs.x), y: Math.round(hs.y) } : null
  const isPoly = roiStore.activeTool === 'poly'
  const last = pts[pts.length - 1]

  // 已有点 + 光标构成的临时区域填充
  if (isPoly && pts.length >= 2 && hover) {
    ctx.beginPath()
    ctx.moveTo(pts[0].x, pts[0].y)
    for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i].x, pts[i].y)
    ctx.lineTo(hover.x, hover.y)
    ctx.closePath()
    ctx.fillStyle = 'rgba(0, 166, 80, 0.14)'
    ctx.fill()
  }

  // 已确认的边
  if (pts.length >= 2) {
    ctx.beginPath()
    ctx.moveTo(pts[0].x, pts[0].y)
    for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i].x, pts[i].y)
    ctx.lineWidth = 2
    ctx.strokeStyle = '#00A650'
    ctx.stroke()
  }

  // 橡皮筋预览：最后一点 -> 光标，多边形再 -> 起点（提示闭合）
  if (hover && last) {
    ctx.beginPath()
    ctx.moveTo(last.x, last.y)
    ctx.lineTo(hover.x, hover.y)
    if (isPoly && pts.length >= 2) ctx.lineTo(pts[0].x, pts[0].y)
    ctx.lineWidth = 1.5
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)'
    ctx.setLineDash([5, 4])
    ctx.stroke()
    ctx.setLineDash([])
  }

  // 顶点手柄，起点用橙色高亮（靠近时可点击闭合）
  pts.forEach((p, i) => {
    const isFirst = i === 0
    const snap = isFirst && isPoly && pts.length >= 3 && hover &&
      Math.hypot(p.x - hover.x, p.y - hover.y) <= 10
    ctx.beginPath()
    ctx.arc(p.x, p.y, snap ? 7 : 4.5, 0, Math.PI * 2)
    ctx.fillStyle = isFirst ? '#F2B705' : '#ffffff'
    ctx.fill()
    ctx.lineWidth = 2
    ctx.strokeStyle = isFirst ? '#ffffff' : '#00A650'
    ctx.stroke()
  })
}

function undoPoint() {
  drawingPoints.value.pop()
  fabric?.requestRenderAll()
}

function finishDrawing() {
  if (!drawingActive.value) return
  const kind = roiStore.activeTool === 'poly' ? 'poly' : 'line'
  const need = kind === 'poly' ? 3 : 2
  const points = drawingPoints.value.map(([x, y]) => [x, y])
  cancelDrawing()
  if (points.length >= need) roiStore.addRoi(kind, { kind, points })
  // 完成后回到选择工具，可直接拖动刚画的 ROI 查看右侧统计变化
  roiStore.setTool('select')
}

function cancelRectDraft() {
  if (draftObj && fabric) fabric.remove(draftObj)
  draftObj = null
  rectStart = null
  fabric?.requestRenderAll()
}

function cancelDrawing() {
  cancelRectDraft()
  drawingPoints.value = []
  hoverScene = null
  roiStore.drawing = false
  fabric?.requestRenderAll()
}

watch(drawingActive, (v) => {
  if (!v) cancelDrawing()
})

defineExpose({ fitToView, getFabric: () => fabric })
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
        <div class="mark-toggle">
          <span>温度标注</span>
          <el-switch
            :model-value="imageStore.showTempMarks"
            size="small"
            @change="onToggleTempMarks"
          />
        </div>
        <template v-if="drawingActive">
          <el-button size="small" type="success" @click="finishDrawing">完成</el-button>
          <el-button size="small" :disabled="!drawingPoints.length" @click="undoPoint">撤销</el-button>
          <el-button size="small" @click="cancelDrawing">取消</el-button>
          <span class="draw-hint">{{ drawHint }}</span>
        </template>
      </div>

      <div class="zoom-badge" style="top: 48px; right: 10px; bottom: auto; font-size: 13px; font-weight: 600; padding: 5px 10px">
        <span>缩放 {{ zoomPct }}%</span>
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

.mark-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  border-radius: 4px;
  background: rgba(31, 39, 51, 0.75);
  color: #dfe6ee;
  font-size: 12px;
  white-space: nowrap;
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
