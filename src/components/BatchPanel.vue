<script setup>
// F14：批量处理面板（文件夹遍历 + 进度 + 结果表 / 离线 GPS 点位地图）
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { FolderOpened } from '@element-plus/icons-vue'
import { useBatchStore } from '../stores/batch.js'
import { useSettingsStore } from '../stores/settings.js'
import { levelInfo } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'
import { pickFolder, pickSave, exportBatchCsv } from '../api/tauri.js'
import MapView from './MapView.vue'
import BatchLedgerDialog from './BatchLedgerDialog.vue'

const batch = useBatchStore()
const settings = useSettingsStore()
const ledgerOpen = ref(false)
const recursive = ref(true)
const resultView = ref('table')
const page = ref(1)
const pageSize = ref(100)
// 结果表分页：Element Plus 表格无虚拟滚动，全量渲染数千行会阻塞主线程
const pagedItems = computed(() =>
  batch.items.slice((page.value - 1) * pageSize.value, page.value * pageSize.value)
)
const overLimitCount = computed(
  () => batch.items.filter((i) => !i.error && i.level >= 1).length
)
const showPagination = computed(() => batch.items.length > pageSize.value)
// 分页条显示时预留 44px 高度，避免表格底部被遮挡 / 小批量时留白
const tableHeight = computed(() =>
  showPagination.value ? 'calc(100% - 214px)' : 'calc(100% - 170px)'
)

// 解析成功后：小批量直接看点位，大批量保持表格（分页），避免一次性挂载地图
watch(
  () => batch.running,
  (running, wasRunning) => {
    if (wasRunning && !running && batch.summary && batch.items.length && !batch.error) {
      page.value = 1
      resultView.value = batch.items.length <= 300 ? 'map' : 'table'
    }
  }
)

async function chooseFolder() {
  let dir
  try {
    dir = await pickFolder()
  } catch (e) {
    ElMessage.error(e?.message || '选择文件夹失败')
    return
  }
  if (!dir) return
  settings.load()
  await batch.start(dir, recursive.value)
  if (batch.error) {
    ElMessage.error(batch.error)
  } else {
    const skippedText = batch.summary.skipped ? `，已忽略 ${batch.summary.skipped} 张非红外照片` : ''
    ElMessage.success(`解析完成：成功 ${batch.summary.ok} 张${skippedText}`)
  }
}

async function onExportBatchCsv() {
  let path
  try {
    path = await pickSave('批量解析结果.csv', ['csv'])
  } catch (e) {
    ElMessage.error(e?.message || '导出失败')
    return
  }
  if (!path) return
  try {
    const rows = batch.items.map((it) => ({
      file_name: it.file_name,
      max: it.error ? 0 : it.max,
      min: it.error ? 0 : it.min,
      delta_t: it.error ? 0 : it.delta_t,
      level: it.error ? '解析失败' : levelInfo(it.level).label,
      path: it.path
    }))
    const n = await exportBatchCsv(rows, path)
    ElMessage.success(`已导出 ${n} 行批量结果`)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : '导出失败')
  }
}

const levelText = (it) => (it.error ? '失败' : levelInfo(it.level).label)
</script>

<template>
  <div class="batch-panel">
    <div class="batch-toolbar">
      <el-button type="primary" :icon="FolderOpened" :loading="batch.running" @click="chooseFolder">
        {{ batch.running ? '解析中…' : '选择文件夹批量解析' }}
      </el-button>
      <el-checkbox v-model="recursive" :disabled="batch.running">包含子文件夹</el-checkbox>
      <el-button :disabled="!batch.items.length" @click="onExportBatchCsv">批量结果导出</el-button>
      <el-button :disabled="!batch.items.length" @click="ledgerOpen = true">批量入库</el-button>
      <div class="view-switch" role="group" aria-label="结果视图">
        <button type="button" :class="{ active: resultView === 'table' }" :disabled="batch.running" @click="resultView = 'table'">表格</button>
        <button type="button" :class="{ active: resultView === 'map' }" :disabled="batch.running" @click="resultView = 'map'">地图</button>
      </div>
      <span class="batch-tip">仅扫描 JPG，自动忽略非红外照片（御 3T / FLIR 辐射原图）</span>
    </div>

    <div v-if="batch.running" class="batch-progress">
      <el-progress
        :percentage="batch.progress.total ? Math.round((batch.progress.done / batch.progress.total) * 100) : 0"
        :stroke-width="14"
      />
      <p class="progress-text">
        {{ batch.progress.done }} / {{ batch.progress.total }} — {{ batch.progress.file }}
      </p>
    </div>

    <template v-else>
      <div v-if="resultView === 'map' && batch.items.length" class="map-pane">
        <MapView :items="batch.items" />
      </div>
      <div v-else-if="!batch.items.length" class="batch-empty">
        <el-icon :size="44" color="#a8b3c1"><FolderOpened /></el-icon>
        <p v-if="batch.summary && batch.summary.skipped">
          未发现红外原图，已忽略 {{ batch.summary.skipped }} 张非红外照片
        </p>
        <p v-else>选择包含辐射原图的文件夹，批量解析测温</p>
      </div>
      <template v-else>
        <div v-if="batch.summary" class="batch-summary">
          <div class="sum-card"><b>{{ batch.summary.total }}</b><span>总数</span></div>
          <div class="sum-card ok"><b>{{ batch.summary.ok }}</b><span>成功</span></div>
          <div class="sum-card"><b>{{ batch.summary.skipped }}</b><span>已忽略</span></div>
          <div class="sum-card warn">
            <b>{{ overLimitCount }}</b><span>超限</span>
          </div>
        </div>

        <el-table v-if="batch.items.length" :data="pagedItems" size="small" stripe :height="tableHeight">
          <el-table-column type="index" width="52" label="#" :index="(i) => (page - 1) * pageSize + i + 1" />
          <el-table-column prop="file_name" label="文件名" min-width="220" show-overflow-tooltip />
          <el-table-column label="最高温" width="96">
            <template #default="{ row }">
              <span :class="{ hot: !row.error }">{{ row.error ? '—' : fmtTemp(row.max) }}</span>
            </template>
          </el-table-column>
          <el-table-column label="最低温" width="96">
            <template #default="{ row }">{{ row.error ? '—' : fmtTemp(row.min) }}</template>
          </el-table-column>
          <el-table-column label="温差ΔT" width="96">
            <template #default="{ row }">{{ row.error ? '—' : fmtDelta(row.delta_t) }}</template>
          </el-table-column>
          <el-table-column label="判定" width="88">
            <template #default="{ row }">
              <span v-if="row.error" class="err-text">失败</span>
              <span v-else class="level-tag" :style="{ background: levelInfo(row.level).color }">
                {{ levelText(row) }}
              </span>
            </template>
          </el-table-column>
          <el-table-column prop="path" label="路径" min-width="240" show-overflow-tooltip />
        </el-table>

        <div v-if="showPagination" class="batch-pagination">
          <el-pagination
            v-model:current-page="page"
            v-model:page-size="pageSize"
            :page-sizes="[50, 100, 200]"
            :total="batch.items.length"
            layout="total, sizes, prev, pager, next, jumper"
            size="small"
            background
            @size-change="page = 1"
          />
        </div>
      </template>
    </template>

    <BatchLedgerDialog v-model="ledgerOpen" :items="batch.items" />
  </div>
</template>

<style scoped>
.batch-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 14px;
}

.batch-toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
}

.batch-tip {
  font-size: 12px;
  color: var(--ip-text-muted);
}

.batch-progress {
  display: flex;
  align-items: center;
  gap: 16px;
}

.progress-text {
  margin: 0;
  font-size: 12px;
  color: var(--ip-text-secondary);
  white-space: nowrap;
}

.batch-summary {
  display: flex;
  gap: 12px;
}

.batch-pagination {
  display: flex;
  justify-content: flex-end;
}

.sum-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 8px 18px;
  border: 1px solid var(--ip-border);
  border-radius: 8px;
  background: var(--ip-surface);
}

.sum-card b {
  font-size: 18px;
}

.sum-card span {
  font-size: 11px;
  color: var(--ip-text-muted);
}

.sum-card.ok b { color: var(--ip-accent); }
.sum-card.bad b { color: var(--ip-danger); }
.sum-card.warn b { color: var(--ip-warning); }

.batch-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--ip-text-muted);
}

.hot {
  color: var(--ip-danger);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.err-text {
  color: var(--ip-text-muted);
}

.level-tag {
  padding: 2px 8px;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
}

.view-switch {
  display: inline-flex;
  padding: 3px;
  border: 1px solid var(--ip-border);
  border-radius: 8px;
  background: #f2f4f8;
}
.view-switch button {
  padding: 5px 14px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--ip-text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.18s ease;
}
.view-switch button:hover:not(:disabled) { color: var(--ip-primary); }
.view-switch button.active {
  background: var(--ip-surface);
  color: var(--ip-primary);
  font-weight: 600;
  box-shadow: var(--ip-shadow);
}
.view-switch button:disabled { cursor: not-allowed; opacity: 0.55; }
.map-pane { flex: 1; min-height: 0; }
</style>
