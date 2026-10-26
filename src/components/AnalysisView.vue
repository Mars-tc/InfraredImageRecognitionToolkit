<script setup>
import { computed, ref, onMounted, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useImageStore } from '../stores/image.js'
import { useRoiStore } from '../stores/roi.js'
import { pickImage, pickSave, exportCsv, exportAnnotated } from '../api/tauri.js'
import { fmtTemp } from '../utils/format.js'
import CanvasView from './CanvasView.vue'
import ColorScale from './ColorScale.vue'
import RoiPanel from './RoiPanel.vue'
import MetaPanel from './MetaPanel.vue'

const store = useImageStore()
const roiStore = useRoiStore()
const dragging = ref(false)
const emissivityInput = ref(0.95)
const distanceInput = ref(5)

onMounted(() => {
  store.autoLoadFromUrl()
})

// meta 更新后同步工具栏输入框
watch(
  () => store.meta,
  (m) => {
    if (m) {
      emissivityInput.value = Number(m.emissivity.toFixed(2))
      distanceInput.value = Number(m.distance.toFixed(1))
    }
  }
)

const statusText = computed(() => {
  if (store.loading) return '解析中…'
  if (store.error) return store.error
  if (!store.hasImage) return '拖拽或点击导入辐射原图（R-JPEG）'
  return store.meta?.file_name || ''
})

const hoverText = computed(() => {
  const h = store.hover
  if (!h || h.temp === null) return '--'
  return `(${h.x}, ${h.y})  ${fmtTemp(h.temp)}`
})

async function onImport() {
  const path = await pickImage()
  if (path) await store.load(path)
}

function onDrop(e) {
  dragging.value = false
  const file = e.dataTransfer?.files?.[0]
  const path = file?.path || file?.rawPath
  if (!path) {
    ElMessage.warning('请直接拖入文件（从文件管理器拖入）')
    return
  }
  store.load(path)
}

async function applyEmissivity() {
  await store.changeEmissivity(Number(emissivityInput.value))
}

async function applyDistance() {
  await store.changeDistance(Number(distanceInput.value))
}

// F8：导出温度矩阵 CSV
async function onExportCsv() {
  const base = (store.meta?.file_name || 'image').replace(/\.jpe?g$/i, '')
  const path = await pickSave(`${base}_温度矩阵.csv`, ['csv'])
  if (!path) return
  try {
    const n = await exportCsv(path)
    ElMessage.success(`已导出 ${n} 个像素的温度矩阵`)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : '导出失败')
  }
}

// F8：导出带 ROI 标注的红外 PNG
async function onExportPng() {
  const base = (store.meta?.file_name || 'image').replace(/\.jpe?g$/i, '')
  const path = await pickSave(`${base}_标注图.png`, ['png'])
  if (!path) return
  try {
    await exportAnnotated(path, roiStore.rois.map((r) => r.shape))
    ElMessage.success('标注图已导出')
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : '导出失败')
  }
}
</script>

<template>
  <div class="analysis-view">
    <div class="toolbar">
      <el-button type="primary" :icon="'FolderOpened'" @click="onImport">导入辐射原图</el-button>
      <el-button :icon="'Download'" :disabled="!store.hasImage" @click="onExportCsv">导出 CSV</el-button>
      <el-button :icon="'Picture'" :disabled="!store.hasImage" @click="onExportPng">导出标注图</el-button>
      <div class="toolbar-spacer" />
      <div class="toolbar-field">
        <span class="field-label">发射率</span>
        <el-input-number v-model="emissivityInput" :min="0.05" :max="1" :step="0.01" :precision="2" size="small" :controls="false" style="width: 80px" @change="applyEmissivity" />
      </div>
      <div class="toolbar-field">
        <span class="field-label">距离(m)</span>
        <el-input-number v-model="distanceInput" :min="0.1" :max="10000" :step="1" size="small" :controls="false" style="width: 80px" @change="applyDistance" />
      </div>
    </div>

    <div class="analysis-body">
      <div
        class="canvas-area"
        :class="{ dragging }"
        @dragover.prevent="dragging = true"
        @dragleave.prevent="dragging = false"
        @drop.prevent="onDrop"
      >
        <div v-if="!store.hasImage" class="canvas-placeholder">
          <el-icon :size="52" color="#5E6D82"><Camera /></el-icon>
          <p class="ph-title">拖拽红外辐射原图到此处</p>
          <p class="ph-sub">支持御 3T / Mavic 2 行业进阶版 / FLIR 的 R-JPEG（需在相机端开启保存原图）</p>
        </div>
        <CanvasView v-else class="canvas-mounted" />
      </div>

      <aside class="side-panel">
        <section class="panel-card">
          <ColorScale />
        </section>

        <section class="panel-card">
          <RoiPanel />
        </section>

        <MetaPanel class="meta-holder" />
      </aside>
    </div>

    <footer class="status-bar">
      <span class="status-file">{{ statusText }}</span>
      <span class="status-hover">{{ hoverText }}</span>
    </footer>
  </div>
</template>

<style scoped>
.analysis-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: var(--ip-surface);
  border: 1px solid var(--ip-border);
  border-radius: var(--ip-radius);
  box-shadow: var(--ip-shadow);
}

.toolbar-spacer {
  flex: 1;
}

.toolbar-field {
  display: flex;
  align-items: center;
  gap: 6px;
}

.field-label {
  font-size: 12px;
  color: var(--ip-text-secondary);
}

.analysis-body {
  display: flex;
  flex: 1;
  min-height: 0;
  gap: 12px;
}

.canvas-area {
  position: relative;
  flex: 1;
  min-width: 0;
  background: var(--ip-canvas-bg);
  border: 1px solid var(--ip-border);
  border-radius: var(--ip-radius);
  overflow: hidden;
}

.canvas-area.dragging {
  outline: 2px dashed var(--ip-primary);
  outline-offset: -8px;
}

/* 占位符自行撑满并居中（不能用 grid 容器居中，否则子项百分比高度塌陷） */
.canvas-placeholder {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.canvas-mounted {
  width: 100%;
  height: 100%;
}

.ph-title {
  margin: 0;
  font-size: 15px;
  color: #c9d2dd;
}

.ph-sub,
.dim-text {
  margin: 0;
  font-size: 12px;
  color: #6b7a8d;
}

.side-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 300px;
  overflow-y: auto;
}

.panel-card {
  padding: 12px 14px;
  background: var(--ip-surface);
  border: 1px solid var(--ip-border);
  border-radius: var(--ip-radius);
  box-shadow: var(--ip-shadow);
}

.status-bar {
  display: flex;
  justify-content: space-between;
  padding: 6px 14px;
  background: var(--ip-surface);
  border: 1px solid var(--ip-border);
  border-radius: var(--ip-radius);
  font-size: 12px;
  color: var(--ip-text-secondary);
}

.status-hover {
  font-variant-numeric: tabular-nums;
}
</style>
