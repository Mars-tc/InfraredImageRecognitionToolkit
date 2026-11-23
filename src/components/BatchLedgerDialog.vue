<script setup>
// 批量解析结果批量入库对话框
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useLedgerStore } from '../stores/ledger.js'
import { buildBatchLedgerInputs } from '../utils/ledgerDraft.js'

const props = defineProps({
  modelValue: Boolean,
  items: { type: Array, default: () => [] }
})
const emit = defineEmits(['update:modelValue', 'saved'])

const ledger = useLedgerStore()
const towerNo = ref('')
const equipment = ref('')
const note = ref('')
const onlyOverLimit = ref(true)
const saving = ref(false)

const candidates = computed(() =>
  buildBatchLedgerInputs({
    items: props.items,
    towerNo: towerNo.value,
    equipment: equipment.value,
    note: note.value,
    onlyOverLimit: onlyOverLimit.value
  })
)

const successCount = computed(() => props.items.filter((it) => !it.error).length)
const overLimitCount = computed(() => props.items.filter((it) => !it.error && it.level >= 1).length)

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      towerNo.value = ''
      equipment.value = ''
      note.value = ''
      onlyOverLimit.value = true
    }
  }
)

async function onSubmit() {
  if (!towerNo.value.trim() && !equipment.value.trim()) {
    ElMessage.warning('请填写杆塔号或设备/部位')
    return
  }
  if (!candidates.value.length) {
    ElMessage.warning('当前筛选条件下没有可入库的记录')
    return
  }
  saving.value = true
  try {
    const n = await ledger.addMany(candidates.value)
    ElMessage.success(`已批量入库 ${n} 条记录`)
    emit('saved')
    emit('update:modelValue', false)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : e?.message || '批量入库失败')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="批量存入巡检台账"
    width="480px"
    @update:model-value="(v) => emit('update:modelValue', v)"
  >
    <el-form label-width="100px" label-position="left">
      <el-form-item label="杆塔号">
        <el-input v-model="towerNo" placeholder="本批次公共杆塔号（可留空）" />
      </el-form-item>
      <el-form-item label="设备/部位">
        <el-input v-model="equipment" placeholder="如：电缆接头（可留空）" />
      </el-form-item>
      <el-form-item label="备注">
        <el-input v-model="note" placeholder="附加到每条记录，可留空" />
      </el-form-item>
      <el-form-item label="入库范围">
        <el-radio-group v-model="onlyOverLimit">
          <el-radio :value="true">仅超限（{{ overLimitCount }}）</el-radio>
          <el-radio :value="false">全部成功项（{{ successCount }}）</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="预计入库">
        <span class="count">{{ candidates.length }} 条</span>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="onSubmit">批量入库</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.count {
  font-weight: 600;
  color: var(--ip-primary);
}
</style>
