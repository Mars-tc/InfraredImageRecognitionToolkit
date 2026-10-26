<script setup>
// F5/F10：ROI 统计面板（max/min/avg/ΔT + F12 定级 + F11 三相告警）
import { Warning } from '@element-plus/icons-vue'
import { useRoiStore } from '../stores/roi.js'
import { useSettingsStore } from '../stores/settings.js'
import { levelInfo } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'

const roiStore = useRoiStore()
const settingsStore = useSettingsStore()
settingsStore.load()

const KIND_LABEL = { rect: '矩形', poly: '多边形', line: '线段' }

function removeRoi(id) {
  roiStore.removeRoi(id)
}
</script>

<template>
  <div class="roi-panel">
    <div class="roi-head">
      <span class="roi-title">ROI 统计</span>
      <el-button v-if="roiStore.rois.length" size="small" text type="danger" @click="roiStore.clearAll">清空</el-button>
    </div>

    <p v-if="!roiStore.rois.length" class="roi-empty">
      选择「矩形 / 多边形 / 线段」工具在画布上绘制测温区域
    </p>

    <div v-else class="roi-list">
      <div
        v-for="roi in roiStore.rois"
        :key="roi.id"
        class="roi-item"
        :class="{ active: roiStore.selectedId === roi.id }"
        @click="roiStore.selectRoi(roi.id)"
      >
        <div class="roi-item-head">
          <span class="roi-dot" :style="{ background: roi.color }" />
          <span class="roi-name">{{ roi.label }}</span>
          <span class="roi-kind">{{ KIND_LABEL[roi.kind] }}</span>
          <el-select
            :model-value="roi.phase"
            size="small"
            style="width: 64px"
            placeholder="相别"
            @change="(v) => roiStore.setPhase(roi.id, v)"
            @click.stop
          >
            <el-option label="A相" value="A" />
            <el-option label="B相" value="B" />
            <el-option label="C相" value="C" />
            <el-option label="N相" value="N" />
          </el-select>
          <el-button size="small" text type="danger" @click.stop="removeRoi(roi.id)">删除</el-button>
        </div>
        <div v-if="roi.stats" class="roi-stats">
          <div class="stat"><span>最高</span><b class="hot">{{ fmtTemp(roi.stats.max) }}</b></div>
          <div class="stat"><span>最低</span><b>{{ fmtTemp(roi.stats.min) }}</b></div>
          <div class="stat"><span>平均</span><b>{{ fmtTemp(roi.stats.avg) }}</b></div>
          <div class="stat"><span>温差ΔT</span><b class="delta">{{ fmtDelta(roi.stats.delta_t) }}</b></div>
        </div>
        <div v-if="roi.verdict" class="verdict" :style="{ borderColor: levelInfo(roi.verdict.level).color }">
          <span class="level-tag" :style="{ background: levelInfo(roi.verdict.level).color }">
            {{ levelInfo(roi.verdict.level).label }}
          </span>
          <span class="verdict-reason">{{ roi.verdict.reason }}</span>
        </div>
        <p v-else-if="roi.error" class="roi-err">{{ roi.error }}</p>
        <p v-else class="roi-err">统计中…</p>
      </div>
    </div>

    <div v-if="roiStore.phaseAlert" class="phase-alert">
      <el-icon><Warning /></el-icon>
      <span>{{ roiStore.phaseAlert.reason }}</span>
    </div>
  </div>
</template>

<style scoped>
.roi-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.roi-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.roi-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--ip-text);
}

.roi-empty {
  margin: 0;
  font-size: 12px;
  color: var(--ip-text-muted);
  line-height: 1.6;
}

.roi-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 300px;
  overflow-y: auto;
}

.roi-item {
  padding: 8px 10px;
  border: 1px solid var(--ip-border);
  border-radius: 6px;
  cursor: pointer;
  transition: border-color 0.15s ease;
}

.roi-item:hover {
  border-color: var(--ip-info);
}

.roi-item.active {
  border-color: var(--ip-primary);
  background: rgba(0, 91, 172, 0.04);
}

.roi-item-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.roi-dot {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex: none;
}

.roi-name {
  font-size: 12px;
  font-weight: 600;
}

.roi-kind {
  font-size: 11px;
  color: var(--ip-text-muted);
  margin-right: auto;
}

.roi-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2px 10px;
  margin-top: 6px;
  font-size: 12px;
}

.stat {
  display: flex;
  justify-content: space-between;
}

.stat span {
  color: var(--ip-text-secondary);
}

.stat b {
  font-variant-numeric: tabular-nums;
}

.stat .hot {
  color: var(--ip-danger);
}

.stat .delta {
  color: var(--ip-warning);
}

.roi-err {
  margin: 4px 0 0;
  font-size: 11px;
  color: var(--ip-text-muted);
}

.verdict {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  padding: 3px 6px;
  border: 1px solid var(--ip-border);
  border-radius: 4px;
}

.level-tag {
  flex: none;
  padding: 1px 6px;
  border-radius: 3px;
  color: #fff;
  font-size: 11px;
  font-weight: 600;
}

.verdict-reason {
  font-size: 11px;
  color: var(--ip-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.phase-alert {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border: 1px solid var(--ip-warning);
  background: rgba(242, 183, 5, 0.08);
  border-radius: 6px;
  color: var(--ip-warning);
  font-size: 12px;
}
</style>
