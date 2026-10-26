<script setup>
// F15：巡检台账看板（SQLite CRUD + 筛选 + 统计卡片）
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Search, Document } from '@element-plus/icons-vue'
import { useLedgerStore } from '../stores/ledger.js'
import { DEFECT_LEVELS, levelInfo } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'
import ReportDialog from './ReportDialog.vue'

const ledger = useLedgerStore()
const kwInput = ref('')
const reportOpen = ref(false)
const selectedIds = ref([])

onMounted(() => ledger.refresh())

async function doSearch() {
  await ledger.setFilter(kwInput.value, ledger.levelFilter)
}

async function onLevelFilter(v) {
  ledger.levelFilter = v
  await ledger.setFilter(kwInput.value, v)
}

async function removeRow(row) {
  await ElMessageBox.confirm(`确认删除「${row.tower_no} ${row.equipment}」记录？`, '删除确认', {
    type: 'warning'
  })
  await ledger.remove(row.id)
  ElMessage.success('已删除')
}

function saveRow(row) {
  ledger
    .update(row.id, {
      tower_no: row.tower_no,
      equipment: row.equipment,
      phase: row.phase,
      image_path: row.image_path,
      max_temp: row.max_temp,
      delta_t: row.delta_t,
      level: row.level,
      note: row.note
    })
    .then(() => ElMessage.success('已保存'))
    .catch((e) => ElMessage.error(typeof e === 'string' ? e : '保存失败'))
}
</script>

<template>
  <div class="ledger-panel">
    <div class="ledger-toolbar">
      <el-input
        v-model="kwInput"
        placeholder="杆塔号 / 设备 / 备注"
        clearable
        style="width: 240px"
        :prefix-icon="Search"
        @keyup.enter="doSearch"
      />
      <el-select
        :model-value="ledger.levelFilter"
        placeholder="全部等级"
        clearable
        style="width: 120px"
        @change="onLevelFilter"
      >
        <el-option v-for="l in DEFECT_LEVELS" :key="l.value" :label="l.label" :value="l.value" />
      </el-select>
      <el-button type="primary" @click="doSearch">查询</el-button>
      <el-button :disabled="!selectedIds.length" :icon="Document" @click="reportOpen = true">
        生成缺陷报告（{{ selectedIds.length }}）
      </el-button>
    </div>

    <div class="ledger-stats">
      <div class="stat-card"><b>{{ ledger.levelCounts.reduce((a, b) => a + b, 0) }}</b><span>总数</span></div>
      <div v-for="l in DEFECT_LEVELS" :key="l.value" class="stat-card">
        <b :style="{ color: l.color }">{{ ledger.levelCounts[l.value] }}</b>
        <span>{{ l.label }}</span>
      </div>
    </div>

    <el-table
      v-loading="ledger.loading"
      :data="ledger.records"
      size="small"
      stripe
      height="calc(100% - 150px)"
      @selection-change="(rows) => (selectedIds = rows.map((r) => r.id))"
    >
      <el-table-column type="selection" width="42" />
      <el-table-column prop="tower_no" label="杆塔号" width="100" />
      <el-table-column prop="equipment" label="设备/部位" min-width="130" />
      <el-table-column prop="phase" label="相别" width="64" />
      <el-table-column label="最高温" width="90">
        <template #default="{ row }">{{ fmtTemp(row.max_temp) }}</template>
      </el-table-column>
      <el-table-column label="温差ΔT" width="90">
        <template #default="{ row }">{{ fmtDelta(row.delta_t) }}</template>
      </el-table-column>
      <el-table-column label="等级" width="96">
        <template #default="{ row }">
          <span class="level-tag" :style="{ background: levelInfo(row.level).color }">
            {{ levelInfo(row.level).label }}
          </span>
        </template>
      </el-table-column>
      <el-table-column prop="note" label="备注" min-width="150" show-overflow-tooltip />
      <el-table-column prop="created_at" label="录入时间" width="150" />
      <el-table-column label="操作" width="130" fixed="right">
        <template #default="{ row }">
          <el-button size="small" text type="primary" @click="saveRow(row)">保存</el-button>
          <el-button size="small" text type="danger" @click="removeRow(row)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <ReportDialog v-model="reportOpen" :ids="selectedIds" />
  </div>
</template>

<style scoped>
.ledger-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 14px;
}

.ledger-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ledger-stats {
  display: flex;
  gap: 12px;
}

.stat-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 8px 18px;
  border: 1px solid var(--ip-border);
  border-radius: 8px;
  background: var(--ip-surface);
}

.stat-card b {
  font-size: 18px;
}

.stat-card span {
  font-size: 11px;
  color: var(--ip-text-muted);
}

.level-tag {
  padding: 2px 8px;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
}
</style>
