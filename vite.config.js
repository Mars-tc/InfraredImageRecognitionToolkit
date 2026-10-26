import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 会读取 vite dev server，端口必须与 tauri.conf.json 的 devPath 一致
export default defineConfig({
  plugins: [vue()],
  base: './',
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: '127.0.0.1'
  },
  build: {
    target: 'chrome100',
    chunkSizeWarningLimit: 2000
  },
  envPrefix: ['VITE_', 'TAURI_']
})
