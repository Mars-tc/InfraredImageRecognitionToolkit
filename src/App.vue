<script setup>
import { ref, computed } from 'vue'
import { Setting } from '@element-plus/icons-vue'
import AnalysisView from './components/AnalysisView.vue'
import BatchPanel from './components/BatchPanel.vue'
import LedgerPanel from './components/LedgerPanel.vue'
import SettingsDialog from './components/SettingsDialog.vue'
import { useViewStore } from './stores/view.js'

const view = useViewStore()
const settingsOpen = ref(false)

const views = [
  { key: 'analysis', label: '图像分析', icon: 'Picture' },
  { key: 'batch', label: '批量处理', icon: 'Files' },
  { key: 'ledger', label: '巡检台账', icon: 'Notebook' }
]

const viewTitle = computed(() => views.find((v) => v.key === view.activeView)?.label ?? '')
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand">
        <div class="brand-logo">IR</div>
        <div class="brand-text">
          <span class="brand-name">红外照片分析工具</span>
          <span class="brand-sub">@Mars-GTC</span>
        </div>
      </div>
      <nav class="app-nav">
        <button
          v-for="v in views"
          :key="v.key"
          type="button"
          class="nav-btn"
          :class="{ active: view.activeView === v.key }"
          @click="view.activeView = v.key"
        >
          <el-icon><component :is="v.icon" /></el-icon>
          <span>{{ v.label }}</span>
        </button>
      </nav>
      <div class="app-header-right">
        <button type="button" class="nav-btn" @click="settingsOpen = true">
          <el-icon><Setting /></el-icon>
          <span>阈值设置</span>
        </button>
        <span class="env-badge">离线环境可用</span>
      </div>
    </header>
    <SettingsDialog v-model="settingsOpen" />

    <main class="app-main">
      <AnalysisView v-show="view.activeView === 'analysis'" class="view-pane" />
      <BatchPanel v-show="view.activeView === 'batch'" class="view-pane" />
      <LedgerPanel v-show="view.activeView === 'ledger'" class="view-pane" />
    </main>

    <footer class="app-status">
      <span>{{ viewTitle }}</span>
      <span class="status-dot" /> 就绪
    </footer>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.app-header {
  display: flex;
  align-items: center;
  gap: 24px;
  height: 56px;
  padding: 0 20px;
  background: var(--ip-surface);
  border-bottom: 1px solid var(--ip-border);
  box-shadow: var(--ip-shadow);
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand-logo {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 8px;
  background: linear-gradient(135deg, var(--ip-primary), var(--ip-accent));
  color: #fff;
  font-weight: 700;
  font-size: 13px;
  letter-spacing: 0.5px;
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.25;
}

.brand-name {
  font-size: 15px;
  font-weight: 600;
}

.brand-sub {
  font-size: 11px;
  color: var(--ip-text-muted);
}

.app-nav {
  display: flex;
  gap: 4px;
  margin-left: 8px;
}

.nav-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--ip-text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.nav-btn:hover {
  background: #eef2f7;
  color: var(--ip-primary);
}

.nav-btn.active {
  background: rgba(0, 91, 172, 0.1);
  color: var(--ip-primary);
  font-weight: 600;
}

.app-header-right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 12px;
}

.env-badge {
  padding: 4px 12px;
  border-radius: 999px;
  background: rgba(0, 166, 80, 0.1);
  color: var(--ip-accent);
  font-size: 12px;
  font-weight: 500;
}

.app-main {
  flex: 1;
  min-height: 0;
  padding: 16px;
}

.view-pane {
  height: 100%;
  background: var(--ip-surface);
  border: 1px solid var(--ip-border);
  border-radius: var(--ip-radius);
  box-shadow: var(--ip-shadow);
}

.pane-placeholder {
  display: grid;
  place-items: center;
  gap: 12px;
  height: 100%;
  color: var(--ip-text-muted);
}

.app-status {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 16px;
  background: var(--ip-surface);
  border-top: 1px solid var(--ip-border);
  color: var(--ip-text-secondary);
  font-size: 12px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ip-accent);
}
</style>
