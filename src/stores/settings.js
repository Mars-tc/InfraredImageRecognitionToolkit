// 缺陷阈值设置（F12）
import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '../api/tauri.js'

export const useSettingsStore = defineStore('settings', () => {
  const thresholds = ref({ joint_temp: 70, delta_t: 10, phase_delta_t: 5 })
  const loaded = ref(false)

  async function load() {
    if (loaded.value) return
    try {
      thresholds.value = await api.getSettings()
      loaded.value = true
    } catch (e) {
      console.error('读取阈值失败', e)
    }
  }

  async function save(next) {
    thresholds.value = await api.setSettings(next)
    return thresholds.value
  }

  return { thresholds, loaded, load, save }
})
