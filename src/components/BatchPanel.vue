<script setup>
// F14：批量处理面板（文件夹遍历 + 进度 + 结果表）
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { FolderOpened } from '@element-plus/icons-vue'
import { useBatchStore } from '../stores/batch.js'
import { useSettingsStore } from '../stores/settings.js'
import { levelInfo } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'
import { pickFolder, pickSave, exportBatchCsv } from '../api/tauri.js'

const batch = useBatchStore()
const settings = useSettingsStore()
const recursive = ref(true)

async function chooseFolder() {
  const dir = await pickFolder()
  if (!dir) return
  settings.load()
  await batch.start(dir, recursive.value)
  if (batch.error) {
    ElMessage.error(batch.error)
  } else {
    ElMessage.success(`解析完成：成功 ${batch.summary.ok} 张，失败 ${batch.summary.failed} 张`)
  }
}

async function onExportBatchCsv() {
  const path = await pickSave('批量解析结果.csv', ['csv'])
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
      <span class="batch-tip">仅扫描 JPG（御 3T / FLIR 辐射原图）</span>
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

    <div v-if="batch.summary" class="batch-summary">
      <div class="sum-card"><b>{{ batch.summary.total }}</b><span>总数</span></div>
      <div class="sum-card ok"><b>{{ batch.summary.ok }}</b><span>成功</span></div>
      <div class="sum-card bad"><b>{{ batch.summary.failed }}</b><span>失败</span></div>
      <div class="sum-card warn">
        <b>{{ batch.items.filter((i) => !i.error && i.level >= 1).length }}</b><span>超限</span>
      </div>
    </div>

    <el-table v-if="batch.items.length" :data="batch.items" size="small" stripe height="calc(100% - 170px)">
      <el-table-column type="index" width="52" label="#" />
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

    <div v-else-if="!batch.running" class="batch-empty">
      <el-icon :size="44" color="#a8b3c1"><FolderOpened /></el-icon>
      <p>选择包含辐射原图的文件夹，批量解析测温</p>
    </div>
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
</style>
