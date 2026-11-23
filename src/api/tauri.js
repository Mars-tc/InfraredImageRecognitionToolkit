// Tauri 1 IPC 封装（@tauri-apps/api v1）
import { invoke } from '@tauri-apps/api/tauri'
import { open, save } from '@tauri-apps/api/dialog'
import { listen } from '@tauri-apps/api/event'

/**
 * IPC 超时包装：后端进程已退出（如 dev server 被关闭后残留的旧窗口）时，
 * invoke 的 Promise 会永久挂起，表现为“按钮点了没反应”。
 * 这里对非长任务命令加超时，把静默挂起变成明确报错。
 */
const LONG_TASKS = new Set(['batch_parse', 'export_report'])
const DEFAULT_TIMEOUT_MS = 15000
const LONG_TIMEOUT_MS = 10 * 60 * 1000

function withTimeout(promise, ms) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(
        new Error(
          '后端未响应（超过 ' +
            Math.round(ms / 1000) +
            ' 秒）：当前窗口可能已失效（后端进程已退出），请关闭本窗口并重新启动应用'
        )
      )
    }, ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      }
    )
  })
}

/** 带超时的 invoke：普通命令 15s，长任务（批量解析/报告生成）10min */
export function safeInvoke(cmd, args) {
  const ms = LONG_TASKS.has(cmd) ? LONG_TIMEOUT_MS : DEFAULT_TIMEOUT_MS
  return withTimeout(invoke(cmd, args), ms)
}

/** 应用启动探活：2s 内后端无响应则明确提示（捕获“旧窗口/宿主已死”场景） */
export async function probeBackend() {
  try {
    await withTimeout(invoke('get_settings'), 2000)
    return true
  } catch (e) {
    const msg = typeof e === 'string' ? e : e?.message || String(e)
    if (msg.includes('后端未响应')) {
      throw new Error('后端未连接：窗口与后端进程失联，请关闭窗口后重新启动应用（tauri dev 或先启动 dev server 再打开 exe）')
    }
    return true // 后端在，但该命令报错：不视为失联
  }
}

/** 统一包装原生对话框调用：失败时抛出可读错误，避免按钮“点了没反应” */
async function callDialog(label, fn, options) {
  try {
    return await fn(options)
  } catch (e) {
    const msg = typeof e === 'string' ? e : e?.message || String(e)
    throw new Error(`${label}失败：${msg}`)
  }
}

/** 打开辐射原图选择对话框，返回路径或 null */
export async function pickImage() {
  const selected = await callDialog('打开文件对话框', open, {
    multiple: false,
    filters: [{ name: '红外辐射原图', extensions: ['jpg', 'jpeg'] }]
  })
  if (typeof selected === 'string') return selected
  return null
}

/** 打开文件夹选择对话框，返回路径或 null */
export async function pickFolder() {
  const selected = await callDialog('打开文件夹对话框', open, { directory: true })
  if (typeof selected === 'string') return selected
  return null
}

/** 打开 CSV 文件选择对话框，返回路径或 null */
export async function pickCsv() {
  const selected = await callDialog('打开 CSV 文件对话框', open, {
    multiple: false,
    filters: [{ name: 'CSV 文件', extensions: ['csv'] }]
  })
  return typeof selected === 'string' ? selected : null
}

/** 打开保存文件对话框 */
export async function pickSave(defaultName, extensions = ['csv']) {
  return await callDialog('打开保存对话框', save, {
    defaultPath: defaultName,
    filters: [{ name: '导出文件', extensions }]
  })
}

/** F1：加载辐射原图，返回元数据 */
export function loadImage(path, distance = null) {
  return safeInvoke('load_image', { path, distance })
}

/** 获取温度矩阵（base64 f32 小端）并解码为 Float32Array */
export async function getTemps() {
  const b64 = await invoke('get_temps')
  return base64ToFloat32(b64)
}

/** 当前图片尺寸 */
export function getDimensions() {
  return safeInvoke('get_dimensions')
}

/** 离线地图点位弹窗：按路径生成红外原图缩略图 data URL（按需懒加载） */
export function getThumbnail(path) {
  return invoke('get_thumbnail', { path })
}

/** F7：修改发射率，返回新元数据 */
export function setEmissivity(value) {
  return safeInvoke('set_emissivity', { value })
}

/** 修改测量距离，返回新元数据 */
export function setDistance(value) {
  return safeInvoke('set_distance', { value })
}

/** F5/F10：ROI 统计（矩形/多边形/线段） */
export function roiStats(shape) {
  return safeInvoke('roi_stats', { shape })
}

/** 线段 ROI 沿路径的温度剖面 */
export function lineProfile(shape) {
  return safeInvoke('line_profile', { shape })
}

/** 整张图的最高温 / 最低温及其像素坐标（画布温度标注用） */
export function imageExtrema() {
  return invoke('image_extrema')
}

/** 订阅后端事件（批量进度等） */
export function onEvent(name, cb) {
  return listen(name, (e) => cb(e.payload))
}

/** F12：读取缺陷阈值 */
export function getSettings() {
  return safeInvoke('get_settings')
}

/** F12：保存缺陷阈值 */
export function setSettings(payload) {
  return safeInvoke('set_settings', payload)
}

/** F11：三相同部位 ΔT 比对 */
export function phaseCompare(entries) {
  return safeInvoke('phase_compare', { entries })
}

/** F14：批量解析文件夹（进度经 onEvent('batch-progress')） */
export function batchParse(dir, recursive = true) {
  return safeInvoke('batch_parse', { dir, recursive })
}

/** F8：导出当前图片温度矩阵 CSV */
export function exportCsv(path) {
  return safeInvoke('export_csv', { path })
}

/** F14：批量解析结果导出 CSV */
export function exportBatchCsv(rows, path) {
  return safeInvoke('export_batch_csv', { rows, path })
}

/** F8：导出当前图片带标注 PNG（rois: ROI 形状数组，marks: 温度标注） */
export function exportAnnotated(path, rois = [], marks = []) {
  return safeInvoke('export_annotated', { path, rois, marks })
}

/** F15：新增台账记录 */
export function ledgerAdd(input) {
  return safeInvoke('ledger_add', { input })
}

/** F15：批量新增台账记录 */
export function ledgerAddMany(inputs) {
  return safeInvoke('ledger_add_many', { inputs })
}

/** F15：导出台账 CSV */
export function exportLedgerCsv(path) {
  return safeInvoke('export_ledger_csv', { path })
}

/** F15：导入台账 CSV */
export function importLedgerCsv(path) {
  return safeInvoke('import_ledger_csv', { path })
}

/** F15：查询台账 */
export function ledgerQuery(keyword = '', level = null, limit = 200, offset = 0) {
  return safeInvoke('ledger_query', { keyword, level, limit, offset })
}

/** F15：台账等级统计 */
export function ledgerStats() {
  return safeInvoke('ledger_stats')
}

/** F15：更新台账记录 */
export function ledgerUpdate(id, input) {
  return safeInvoke('ledger_update', { id, input })
}

/** F15：删除台账记录 */
export function ledgerDelete(id) {
  return safeInvoke('ledger_delete', { id })
}

/** F16：导出 Word 缺陷报告（withImages 时附缺陷图片） */
export function exportReport(ids, path, inspector, date, org, withImages = false) {
  return safeInvoke('export_report', { ids, path, inspector, date, org, withImages })
}

/** base64 -> Float32Array（小端） */
export function base64ToFloat32(b64) {
  const bin = atob(b64)
  const len = bin.length
  // f32 每点占 4 字节：长度不对齐时 Float32Array 会静默截断，
  // 导致温度矩阵比图片尺寸短，必须显式报错而不是产出半截数据。
  if (len % 4 !== 0) throw new Error(`温度矩阵字节数异常: ${len}（应为 4 的倍数）`)
  const bytes = new Uint8Array(len)
  for (let i = 0; i < len; i++) bytes[i] = bin.charCodeAt(i)
  return new Float32Array(bytes.buffer, 0, len / 4)
}
