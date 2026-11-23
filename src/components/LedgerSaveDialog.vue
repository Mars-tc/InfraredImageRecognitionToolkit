<script setup>
// 分析页「存入台账」对话框
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useLedgerStore } from '../stores/ledger.js'
import { DEFECT_LEVELS } from '../utils/defects.js'
import { fmtTemp, fmtDelta } from '../utils/format.js'

const props = defineProps({
  modelValue: Boolean,
  draft: { type: Object, default: null }
})
const emit = defineEmits(['update:modelValue', 'saved'])

const ledger = useLedgerStore()
const form = ref(emptyForm())
const saving = ref(false)

function emptyForm() {
  return {
    tower_no: '',
    equipment: '',
    phase: '',
    image_path: '',
    max_temp: 0,
    delta_t: 0,
    level: 0,
    note: ''
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) form.value = { ...emptyForm(), ...(props.draft || {}) }
  }
)

async function onSubmit() {
  if (!form.value.tower_no.trim() && !form.value.equipment.trim()) {
    ElMessage.warning('请填写杆塔号或设备/部位')
    return
  }
  saving.value = true
  try {
    await ledger.add({ ...form.value })
    ElMessage.success('已存入巡检台账')
    emit('saved')
    emit('update:modelValue', false)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : e?.message || '存入台账失败')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="存入巡检台账"
    width="460px"
    @update:model-value="(v) => emit('update:modelValue', v)"
  >
    <el-form label-width="90px" label-position="left">
      <el-form-item label="杆塔号">
        <el-input v-model="form.tower_no" placeholder="如：10kV-测试线-012" />
      </el-form-item>
      <el-form-item label="设备/部位">
        <el-input v-model="form.equipment" placeholder="如：隔离开关 A 相" />
      </el-form-item>
      <el-form-item label="相别">
        <el-select v-model="form.phase" placeholder="可不填" clearable style="width: 120px">
          <el-option label="A相" value="A" />
          <el-option label="B相" value="B" />
          <el-option label="C相" value="C" />
          <el-option label="N相" value="N" />
        </el-select>
      </el-form-item>
      <el-form-item label="缺陷等级">
        <el-select v-model="form.level" style="width: 140px">
          <el-option v-for="l in DEFECT_LEVELS" :key="l.value" :label="l.label" :value="l.value" />
        </el-select>
      </el-form-item>
      <el-form-item label="最高温 / ΔT">
        <span class="metric">{{ fmtTemp(form.max_temp) }} / {{ fmtDelta(form.delta_t) }}</span>
      </el-form-item>
      <el-form-item label="备注">
        <el-input v-model="form.note" type="textarea" :rows="2" placeholder="处理意见或现场说明" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSubmit">存入台账</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.metric {
  font-variant-numeric: tabular-nums;
  color: var(--ip-text-secondary);
}
</style>
