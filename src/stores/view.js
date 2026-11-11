// 顶层视图状态：地图弹窗「在图像分析中打开」需要跨组件切换视图
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useImageStore } from './image.js'

export const useViewStore = defineStore('view', () => {
  const activeView = ref('analysis')

  /** 加载指定原图并切到「图像分析」页；加载错误由 image store 展示，仍切换以便查看原因 */
  async function openInAnalysis(path) {
    if (!path) return
    await useImageStore().load(path)
    activeView.value = 'analysis'
  }

  return { activeView, openInAnalysis }
})
