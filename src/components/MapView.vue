<script setup>
// 离线点位地图：纯 SVG 绘制，无底图 / 无外部依赖。
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { Location } from '@element-plus/icons-vue'
import { DEFECT_LEVELS, levelInfo } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'
import {
  toPoints, computeBounds, createProjection, projectPoint, projectPoints,
  fitTransform, zoomAt, gridTicks, formatCoord
} from '../utils/mapProjection.js'
import { getThumbnail } from '../api/tauri.js'
import { useViewStore } from '../stores/view.js'

const props = defineProps({ items: { type: Array, default: () => [] } })
const view = useViewStore()

const host = ref(null)
const size = ref({ width: 960, height: 560 })
const transform = ref(fitTransform())
const selectedId = ref(null)
const hoverId = ref(null)
const dragging = ref(false)
const thumbnail = ref('')
const thumbnailLoading = ref(false)
const thumbnailError = ref('')
const thumbCache = new Map()
let requestSeq = 0
let dragStart = null
let observer = null
let updateSize = null

const points = computed(() => toPoints(props.items))
const missingCount = computed(() => Math.max(0, props.items.length - points.value.length))
const bounds = computed(() => computeBounds(points.value))
const proj = computed(() => createProjection(bounds.value, size.value.width, size.value.height))
const projected = computed(() =>
  projectPoints(points.value, bounds.value, size.value.width, size.value.height)
)
const ticks = computed(() => gridTicks(bounds.value))
const latLines = computed(() => proj.value
  ? ticks.value.lats.map((v) => ({ v, y: projectPoint(proj.value, v, bounds.value.minLon).y, label: formatCoord(v) }))
  : [])
const lonLines = computed(() => proj.value
  ? ticks.value.lons.map((v) => ({ v, x: projectPoint(proj.value, bounds.value.maxLat, v).x, label: formatCoord(v, 'lon') }))
  : [])
const selected = computed(() => projected.value.find((p) => p.id === selectedId.value) || null)
const radius = computed(() => 7 / transform.value.scale)
const fontSize = computed(() => 11 / transform.value.scale)

const popupStyle = computed(() => {
  const p = selected.value
  if (!p) return {}
  const x = transform.value.tx + transform.value.scale * p.x
  const y = transform.value.ty + transform.value.scale * p.y
  const w = 320, h = 420, gap = 16
  return {
    left: `${x + gap + w > size.value.width ? Math.max(8, x - gap - w) : x + gap}px`,
    top: `${Math.max(8, Math.min(Math.max(8, size.value.height - h - 8), y - 64))}px`
  }
})

const markerColor = (it) => (it.error ? '#8B98A9' : levelInfo(it.level).color)

watch(() => props.items, () => {
  selectedId.value = null
  transform.value = fitTransform()
})

watch(selectedId, async (id) => {
  const p = selected.value
  thumbnail.value = ''
  thumbnailError.value = ''
  if (!p) { thumbnailLoading.value = false; return }
  const path = p.item.path
  const seq = ++requestSeq
  if (thumbCache.has(path)) {
    thumbnail.value = thumbCache.get(path)
    thumbnailLoading.value = false
    return
  }
  thumbnailLoading.value = true
  try {
    const url = await getThumbnail(path)
    thumbCache.set(path, url)
    if (seq === requestSeq && selectedId.value === id) thumbnail.value = url
  } catch (e) {
    if (seq === requestSeq) thumbnailError.value = typeof e === 'string' ? e : e?.message || String(e)
    console.error('缩略图加载失败', p.item.file_name || path, e)
  } finally {
    if (seq === requestSeq) thumbnailLoading.value = false
  }
})

onMounted(() => {
  const el = host.value
  if (!el) return
  updateSize = () => {
    const r = el.getBoundingClientRect()
    if (r.width > 0 && r.height > 0) size.value = { width: r.width, height: r.height }
  }
  updateSize()
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(updateSize)
    observer.observe(el)
  } else {
    window.addEventListener('resize', updateSize)
  }
})
onBeforeUnmount(() => {
  observer?.disconnect()
  if (updateSize) window.removeEventListener('resize', updateSize)
})

function localPoint(e) {
  const r = host.value.getBoundingClientRect()
  return { x: e.clientX - r.left, y: e.clientY - r.top }
}
function onWheel(e) {
  if (!projected.value.length) return
  const { x, y } = localPoint(e)
  transform.value = zoomAt(transform.value, e.deltaY < 0 ? 1.15 : 1 / 1.15, x, y)
}
function onMouseDown(e) {
  if (e.button !== 0 || !projected.value.length) return
  const { x, y } = localPoint(e)
  dragging.value = true
  dragStart = { x, y, tx: transform.value.tx, ty: transform.value.ty }
}
function onMouseMove(e) {
  if (!dragging.value || !dragStart) return
  const { x, y } = localPoint(e)
  transform.value = { ...transform.value, tx: dragStart.tx + x - dragStart.x, ty: dragStart.ty + y - dragStart.y }
}
function onMouseUp() {
  dragging.value = false
  dragStart = null
}
function resetView() {
  transform.value = fitTransform()
  selectedId.value = null
}
async function openInAnalysis() {
  if (selected.value) await view.openInAnalysis(selected.value.item.path)
}
</script>

<template>
  <div ref="host" class="map-host" :class="{ dragging }">
    <div class="map-topbar">
      <span class="map-badge">离线点位图 · {{ projected.length }} 个拍摄位置</span>
      <div class="map-actions">
        <span v-if="missingCount" class="map-warn">{{ missingCount }} 张无坐标未显示</span>
        <button type="button" class="map-btn" @click="resetView">适应视图</button>
      </div>
    </div>

    <svg v-if="projected.length" class="map-svg" :width="size.width" :height="size.height"
      @wheel.prevent="onWheel" @mousedown="onMouseDown" @mousemove="onMouseMove"
      @mouseup="onMouseUp" @mouseleave="onMouseUp">
      <defs>
        <radialGradient id="mapGlow" cx="50%" cy="45%" r="75%">
          <stop offset="0%" stop-color="#2b3b50" />
          <stop offset="100%" stop-color="#1f2733" />
        </radialGradient>
      </defs>
      <rect :width="size.width" :height="size.height" fill="url(#mapGlow)" pointer-events="none" />
      <g :transform="`translate(${transform.tx} ${transform.ty}) scale(${transform.scale})`">
        <line v-for="l in latLines" :key="`lat-${l.v}`" class="grid-line" :x1="0" :x2="size.width" :y1="l.y" :y2="l.y" vector-effect="non-scaling-stroke" />
        <line v-for="l in lonLines" :key="`lon-${l.v}`" class="grid-line" :y1="0" :y2="size.height" :x1="l.x" :x2="l.x" vector-effect="non-scaling-stroke" />
        <text v-for="l in latLines" :key="`lat-t-${l.v}`" class="grid-label" :x="8 / transform.scale" :y="l.y - 6 / transform.scale" :font-size="fontSize">{{ l.label }}</text>
        <text v-for="l in lonLines" :key="`lon-t-${l.v}`" class="grid-label" :x="l.x + 6 / transform.scale" :y="size.height - 10 / transform.scale" :font-size="fontSize">{{ l.label }}</text>
        <g v-for="p in projected" :key="p.id" class="marker" @click.stop="selectedId = p.id" @mouseenter="hoverId = p.id" @mouseleave="hoverId = null">
          <circle v-if="p.id === selectedId" class="pulse-ring" :cx="p.x" :cy="p.y" :r="radius * 2.2" :stroke="markerColor(p.item)" vector-effect="non-scaling-stroke" />
          <circle v-if="p.item.error" class="fail-ring" :cx="p.x" :cy="p.y" :r="radius * 1.7" vector-effect="non-scaling-stroke" />
          <circle class="dot" :cx="p.x" :cy="p.y" :r="radius * (hoverId === p.id ? 1.35 : 1)" :fill="markerColor(p.item)" vector-effect="non-scaling-stroke" />
          <title>{{ p.item.file_name || p.id }}</title>
        </g>
      </g>
    </svg>

    <div v-else class="map-empty">
      <el-icon :size="42" color="#6c7d92"><Location /></el-icon>
      <p>这批图片没有可用的 GPS 坐标</p>
      <span>照片含 EXIF 或大疆 XMP 定位信息时，会按拍摄位置显示点位</span>
    </div>

    <div v-if="selected" class="map-popup" :style="popupStyle">
      <div class="popup-head">
        <span class="popup-title" :title="selected.item.path">{{ selected.item.file_name || selected.id }}</span>
        <button type="button" class="popup-close" aria-label="关闭" @click="selectedId = null">×</button>
      </div>
      <div class="thumb-box">
        <img v-if="thumbnail" :src="thumbnail" alt="红外原图缩略图" />
        <div v-else-if="thumbnailLoading" class="thumb-placeholder loading">缩略图加载中…</div>
        <div v-else class="thumb-placeholder" :title="thumbnailError">{{ thumbnailError ? '缩略图不可用' : '暂无缩略图' }}</div>
      </div>
      <div class="popup-meta">
        <div><span>GPS</span><b>{{ formatCoord(selected.lat) }} · {{ formatCoord(selected.lon, 'lon') }}</b></div>
        <div><span>最高温</span><b class="hot">{{ selected.item.error ? '—' : fmtTemp(selected.item.max) }}</b></div>
        <div><span>最低温</span><b>{{ selected.item.error ? '—' : fmtTemp(selected.item.min) }}</b></div>
        <div><span>温差 ΔT</span><b>{{ selected.item.error ? '—' : fmtDelta(selected.item.delta_t) }}</b></div>
        <div><span>判定</span><b :style="{ color: markerColor(selected.item) }">{{ selected.item.error ? '解析失败' : levelInfo(selected.item.level).label }}</b></div>
        <div><span>拍摄时间</span><b>{{ selected.item.datetime || '--' }}</b></div>
      </div>
      <div class="popup-path" :title="selected.item.path">{{ selected.item.path }}</div>
      <button type="button" class="popup-open" @click="openInAnalysis">在图像分析中打开</button>
    </div>

    <div class="map-legend">
      <span class="legend-title">缺陷等级</span>
      <span v-for="l in DEFECT_LEVELS" :key="l.value" class="legend-item"><i :style="{ background: l.color }" />{{ l.label }}</span>
      <span class="legend-item"><i class="legend-fail" />解析失败</span>
    </div>
    <div class="map-status">共 {{ projected.length }} 个点位 · {{ selected ? '已选 1 个' : '未选择' }} · {{ transform.scale.toFixed(1) }}×</div>
  </div>
</template>

<style scoped>
.map-host { position: relative; height: 100%; min-height: 360px; overflow: hidden; border-radius: var(--ip-radius); background: #1f2733; }
.map-host.dragging { cursor: grabbing; }
.map-svg { position: absolute; inset: 0; display: block; cursor: grab; touch-action: none; }
.map-topbar { position: absolute; z-index: 2; top: 0; left: 0; right: 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 14px; pointer-events: none; }
.map-badge { padding: 6px 12px; border: 1px solid rgba(255,255,255,.14); border-radius: 999px; background: rgba(18,26,36,.72); color: #e8eef6; font-size: 12px; letter-spacing: .2px; backdrop-filter: blur(6px); }
.map-actions { display: flex; align-items: center; gap: 10px; pointer-events: auto; }
.map-warn { padding: 5px 10px; border-radius: 6px; background: rgba(242,183,5,.16); color: #ffdb70; font-size: 12px; }
.map-btn { padding: 7px 14px; border: 1px solid rgba(255,255,255,.2); border-radius: 7px; background: rgba(18,26,36,.75); color: #e8eef6; font-size: 12px; cursor: pointer; transition: all .18s ease; }
.map-btn:hover { background: var(--ip-primary-hover); border-color: var(--ip-primary-hover); }
.grid-line { stroke: rgba(160,190,220,.12); stroke-width: 1; }
.grid-label { fill: rgba(190,208,228,.55); font-family: inherit; font-variant-numeric: tabular-nums; }
.marker { cursor: pointer; }
.dot { stroke: rgba(255,255,255,.85); stroke-width: 1.5; transition: r .15s ease; filter: drop-shadow(0 0 5px rgba(255,255,255,.35)); }
.marker:hover .dot { filter: drop-shadow(0 0 9px rgba(255,255,255,.75)); }
.fail-ring { fill: none; stroke: #a8b3c1; stroke-width: 1; stroke-dasharray: 3 2; }
.pulse-ring { fill: none; stroke-width: 1.5; opacity: .8; transform-box: fill-box; transform-origin: center; animation: pulse 1.8s ease-out infinite; }
@keyframes pulse { 0% { transform: scale(.7); opacity: .9; } 100% { transform: scale(1.5); opacity: 0; } }
.map-empty { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: #9fb0c4; }
.map-empty p { margin: 4px 0 0; font-size: 15px; font-weight: 600; color: #dce6f2; }
.map-empty span { font-size: 12px; }
.map-popup { position: absolute; z-index: 4; width: 320px; padding: 14px; border: 1px solid rgba(255,255,255,.5); border-radius: 12px; background: rgba(255,255,255,.97); box-shadow: 0 14px 40px rgba(5,12,22,.45); animation: fade-up .2s ease both; }
@keyframes fade-up { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
.popup-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 10px; }
.popup-title { overflow: hidden; font-size: 13px; font-weight: 600; color: var(--ip-text); text-overflow: ellipsis; white-space: nowrap; }
.popup-close { border: none; background: transparent; color: var(--ip-text-muted); font-size: 20px; line-height: 1; cursor: pointer; }
.popup-close:hover { color: var(--ip-danger); }
.thumb-box { display: grid; place-items: center; height: 158px; overflow: hidden; border-radius: 8px; background: #1f2733; }
.thumb-box img { width: 100%; height: 100%; object-fit: contain; }
.thumb-placeholder { color: #9fb0c4; font-size: 12px; }
.thumb-placeholder.loading { animation: breathe 1.2s ease-in-out infinite; }
@keyframes breathe { 50% { opacity: .45; } }
.popup-meta { display: grid; gap: 5px; margin-top: 10px; font-size: 12px; }
.popup-meta > div { display: flex; justify-content: space-between; gap: 10px; }
.popup-meta span { color: var(--ip-text-muted); }
.popup-meta b { color: var(--ip-text); font-weight: 600; font-variant-numeric: tabular-nums; }
.popup-meta b.hot { color: var(--ip-danger); }
.popup-path { overflow: hidden; margin: 8px 0 10px; color: var(--ip-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.popup-open { width: 100%; padding: 9px; border: none; border-radius: 8px; background: linear-gradient(135deg, var(--ip-primary), var(--ip-primary-hover)); color: #fff; font-size: 13px; font-weight: 600; cursor: pointer; transition: filter .18s ease, transform .18s ease; }
.popup-open:hover { filter: brightness(1.1); transform: translateY(-1px); }
.map-legend { position: absolute; z-index: 2; bottom: 12px; left: 14px; display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 8px 12px; border: 1px solid rgba(255,255,255,.12); border-radius: 8px; background: rgba(18,26,36,.78); color: #c9d6e5; font-size: 11px; backdrop-filter: blur(6px); }
.legend-title { color: #8fa1b6; }
.legend-item { display: inline-flex; align-items: center; gap: 5px; }
.legend-item i { width: 8px; height: 8px; border-radius: 50%; }
.legend-item i.legend-fail { border: 1px dashed #a8b3c1; background: transparent; }
.map-status { position: absolute; z-index: 2; right: 14px; bottom: 12px; padding: 6px 10px; border-radius: 6px; background: rgba(18,26,36,.72); color: #c9d6e5; font-size: 11px; font-variant-numeric: tabular-nums; }
</style>
