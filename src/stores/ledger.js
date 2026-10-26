// 巡检台账状态（F15）
import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '../api/tauri.js'

export const useLedgerStore = defineStore('ledger', () => {
  const records = ref([])
  const levelCounts = ref([0, 0, 0, 0])
  const keyword = ref('')
  const levelFilter = ref(null)
  const loading = ref(false)
  const error = ref('')

  async function refresh() {
    loading.value = true
    error.value = ''
    try {
      records.value = await api.ledgerQuery(keyword.value, levelFilter.value, 200, 0)
      levelCounts.value = await api.ledgerStats()
    } catch (e) {
      error.value = typeof e === 'string' ? e : e?.message || String(e)
    } finally {
      loading.value = false
    }
  }

  async function add(input) {
    const id = await api.ledgerAdd(input)
    await refresh()
    return id
  }

  async function update(id, input) {
    await api.ledgerUpdate(id, input)
    await refresh()
  }

  async function remove(id) {
    await api.ledgerDelete(id)
    await refresh()
  }

  function setFilter(keyword_, level_) {
    keyword.value = keyword_
    levelFilter.value = level_
    return refresh()
  }

  return {
    records, levelCounts, keyword, levelFilter, loading, error,
    refresh, add, update, remove, setFilter
  }
})
