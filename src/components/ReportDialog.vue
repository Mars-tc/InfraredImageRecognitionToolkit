<script setup>
// F16：Word 缺陷报告导出对话框
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useLedgerStore } from '../stores/ledger.js'
import { pickSave, exportReport } from '../api/tauri.js'

const props = defineProps({
  modelValue: Boolean,
  ids: { type: Array, default: () => [] }
})
const emit = defineEmits(['update:modelValue'])

const ledger = useLedgerStore()
const inspector = ref('')
const org = ref('配电运维班')
const date = ref(new Date().toISOString().slice(0, 10))
const exporting = ref(false)

watch(
  () => props.modelValue,
  (open) => {
    if (open && !ledger.records.length) ledger.refresh()
  }
)

async function onExport() {
  if (!props.ids.length) {
    ElMessage.warning('请先勾选要导出的记录')
    return
  }
  const name = `红外检测缺陷报告_${date.value}.docx`
  const path = await pickSave(name, ['docx'])
  if (!path) return
  exporting.value = true
  try {
    await exportReport(props.ids, path, inspector.value || '未填写', date.value, org.value)
    ElMessage.success('报告已生成')
    emit('update:modelValue', false)
  } catch (e) {
    ElMessage.error(typeof e === 'string' ? e : e?.message || '导出失败')
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="导出巡检缺陷报告（Word）"
    width="420px"
    @update:model-value="(v) => emit('update:modelValue', v)"
  >
    <el-form label-width="90px" label-position="left">
      <el-form-item label="检测单位">
        <el-input v-model="org" placeholder="如：配电运维班" />
      </el-form-item>
      <el-form-item label="检测人">
        <el-input v-model="inspector" placeholder="检测人姓名" />
      </el-form-item>
      <el-form-item label="检测日期">
        <el-input v-model="date" placeholder="YYYY-MM-DD" />
      </el-form-item>
      <el-form-item label="记录数">
        <span>{{ ids.length }} 条（在台账表格中勾选）</span>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :loading="exporting" @click="onExport">生成报告</el-button>
    </template>
  </el-dialog>
</template>
