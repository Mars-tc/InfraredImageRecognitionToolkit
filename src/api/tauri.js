// Tauri 1 IPC 封装（@tauri-apps/api v1）
import { invoke } from '@tauri-apps/api/tauri'
import { open, save } from '@tauri-apps/api/dialog'
import { listen } from '@tauri-apps/api/event'

/** 打开辐射原图选择对话框，返回路径或 null */
export async function pickImage() {
  const selected = await open({
    multiple: false,
    filters: [{ name: '红外辐射原图', extensions: ['jpg', 'jpeg'] }]
  })
  if (typeof selected === 'string') return selected
  return null
}

/** 打开文件夹选择对话框，返回路径或 null */
export async function pickFolder() {
  const selected = await open({ directory: true })
  if (typeof selected === 'string') return selected
  return null
}

/** 打开保存文件对话框 */
export async function pickSave(defaultName, extensions = ['csv']) {
  return await save({
    defaultPath: defaultName,
    filters: [{ name: '导出文件', extensions }]
  })
}

/** F1：加载辐射原图，返回元数据 */
export function loadImage(path, distance = null) {
  return invoke('load_image', { path, distance })
}

/** 获取温度矩阵（base64 f32 小端）并解码为 Float32Array */
export async function getTemps() {
  const b64 = await invoke('get_temps')
  return base64ToFloat32(b64)
}

/** 当前图片尺寸 */
export function getDimensions() {
  return invoke('get_dimensions')
}

/** F7：修改发射率，返回新元数据 */
export function setEmissivity(value) {
  return invoke('set_emissivity', { value })
}

/** 修改测量距离，返回新元数据 */
export function setDistance(value) {
  return invoke('set_distance', { value })
}

/** F5/F10：ROI 统计（矩形/多边形/线段） */
export function roiStats(shape) {
  return invoke('roi_stats', { shape })
}

/** 订阅后端事件（批量进度等） */
export function onEvent(name, cb) {
  return listen(name, (e) => cb(e.payload))
}

/** F12：读取缺陷阈值 */
export function getSettings() {
  return invoke('get_settings')
}

/** F12：保存缺陷阈值 */
export function setSettings(payload) {
  return invoke('set_settings', payload)
}

/** F11：三相同部位 ΔT 比对 */
export function phaseCompare(entries) {
  return invoke('phase_compare', { entries })
}

/** F14：批量解析文件夹（进度经 onEvent('batch-progress')） */
export function batchParse(dir, recursive = true) {
  return invoke('batch_parse', { dir, recursive })
}

/** F8：导出当前图片温度矩阵 CSV */
export function exportCsv(path) {
  return invoke('export_csv', { path })
}

/** F14：批量解析结果导出 CSV */
export function exportBatchCsv(rows, path) {
  return invoke('export_batch_csv', { rows, path })
}

/** F8：导出当前图片带标注 PNG（rois: ROI 形状数组） */
export function exportAnnotated(path, rois = []) {
  return invoke('export_annotated', { path, rois })
}

/** F15：新增台账记录 */
export function ledgerAdd(input) {
  return invoke('ledger_add', { input })
}

/** F15：查询台账 */
export function ledgerQuery(keyword = '', level = null, limit = 200, offset = 0) {
  return invoke('ledger_query', { keyword, level, limit, offset })
}

/** F15：台账等级统计 */
export function ledgerStats() {
  return invoke('ledger_stats')
}

/** F15：更新台账记录 */
export function ledgerUpdate(id, input) {
  return invoke('ledger_update', { id, input })
}

/** F15：删除台账记录 */
export function ledgerDelete(id) {
  return invoke('ledger_delete', { id })
}

/** F16：导出 Word 缺陷报告 */
export function exportReport(ids, path, inspector, date, org) {
  return invoke('export_report', { ids, path, inspector, date, org })
}

/** base64 -> Float32Array（小端） */
export function base64ToFloat32(b64) {
  const bin = atob(b64)
  const len = bin.length
  const bytes = new Uint8Array(len)
  for (let i = 0; i < len; i++) bytes[i] = bin.charCodeAt(i)
  return new Float32Array(bytes.buffer, 0, len / 4)
}
