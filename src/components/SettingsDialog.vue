<script setup>
// F12：缺陷阈值设置对话框
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useSettingsStore } from '../stores/settings.js'

const props = defineProps({ modelValue: Boolean })
const emit = defineEmits(['update:modelValue'])

const settings = useSettingsStore()
const form = ref({ joint_temp: 70, delta_t: 10, phase_delta_t: 5 })
const saving = ref(false)

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      settings.load()
      form.value = { ...settings.thresholds }
    }
  }
)

async function onSave() {
  saving.value = true
  try {
    await settings.save({ ...form.value })
    ElMessage.success('阈值已保存')
    emit('update:modelValue', false)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : e?.message || '保存失败')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="缺陷判定阈值"
    width="420px"
    @update:model-value="(v) => emit('update:modelValue', v)"
  >
    <el-form label-width="120px" label-position="left">
      <el-form-item label="接头温度阈值">
        <el-input-number v-model="form.joint_temp" :min="0" :max="500" :step="5" style="width: 160px" />
        <span class="unit">℃</span>
      </el-form-item>
      <el-form-item label="温差阈值 ΔT">
        <el-input-number v-model="form.delta_t" :min="0" :max="200" :step="1" style="width: 160px" />
        <span class="unit">℃</span>
      </el-form-item>
      <el-form-item label="相间温差阈值">
        <el-input-number v-model="form.phase_delta_t" :min="0" :max="200" :step="1" style="width: 160px" />
        <span class="unit">℃</span>
      </el-form-item>
    </el-form>
    <div class="level-legend">
      <div class="legend-row"><span class="dot other" />其他：未超阈值</div>
      <div class="legend-row"><span class="dot minor" />一般：最高温 ≥ 接头阈值 或 ΔT ≥ 温差阈值</div>
      <div class="legend-row"><span class="dot major" />重大：超过 1.5 倍接头阈值 或 2 倍温差</div>
      <div class="legend-row"><span class="dot critical" />紧急：超过 2 倍接头阈值 或 3 倍温差</div>
    </div>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSave">保存</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.unit {
  margin-left: 8px;
  color: var(--ip-text-secondary);
  font-size: 12px;
}

.level-legend {
  margin-top: 8px;
  padding: 10px 12px;
  background: var(--ip-bg);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.legend-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--ip-text-secondary);
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex: none;
}

.dot.other { background: #8b98a9; }
.dot.minor { background: #f2b705; }
.dot.major { background: #f57c00; }
.dot.critical { background: #e02020; }
</style>
