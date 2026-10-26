// 批量处理状态（F14）
import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '../api/tauri.js'

export const useBatchStore = defineStore('batch', () => {
  const running = ref(false)
  const progress = ref({ done: 0, total: 0, file: '' })
  const items = ref([])
  const summary = ref(null)
  const error = ref('')
  let unlisten = null

  async function start(dir, recursive) {
    if (running.value) return
    running.value = true
    error.value = ''
    items.value = []
    summary.value = null
    progress.value = { done: 0, total: 0, file: '' }
    // 订阅进度事件
    if (!unlisten) {
      unlisten = await api.onEvent('batch-progress', (p) => {
        progress.value = p
      })
    }
    try {
      const s = await api.batchParse(dir, recursive)
      items.value = s.items
      summary.value = { total: s.total, ok: s.ok, failed: s.failed }
    } catch (e) {
      error.value = typeof e === 'string' ? e : e?.message || String(e)
    } finally {
      running.value = false
    }
  }

  return { running, progress, items, summary, error, start }
})
