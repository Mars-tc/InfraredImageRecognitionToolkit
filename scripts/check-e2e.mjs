// E2E 现状核查（只读）：
// 1. 核对 main.rs 注册命令、e2e-cdp.mjs 的 mock 分支、src/api/tauri.js 封装三者一致性；
// 2. 探测本机浏览器；无 Chrome/Chromium 时明确记为受限项，不视为测试失败。
import { readFileSync, existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import assert from 'node:assert/strict'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => readFileSync(join(root, p), 'utf8')

const mainSrc = read('src-tauri/src/main.rs')
const e2eSrc = read('scripts/e2e-cdp.mjs')
const apiSrc = read('src/api/tauri.js')

// main.rs 形如 .invoke_handler(tauri::generate_handler![ ... ])
const handler = mainSrc.match(/generate_handler!\[([\s\S]*?)\]/)
assert.ok(handler, '未找到 invoke_handler')
const commands = handler[1]
  .split(',')
  .map((s) => s.trim())
  .filter((s) => /^[a-z_][a-z0-9_]*$/.test(s))

const mocked = new Set(
  [...e2eSrc.matchAll(/cmd === '([a-z_]+)'/g)].map((m) => m[1])
)
const wrapped = new Set(
  [...apiSrc.matchAll(/invoke\('([a-z_]+)'/g)].map((m) => m[1])
)

// 19 → +image_extrema（画布温度标注）→ +get_thumbnail（离线地图缩略图）
assert.equal(commands.length, 21, 'main.rs 注册命令数变化，请同步更新核查脚本')
for (const cmd of commands) {
  assert.ok(wrapped.has(cmd), `前端缺少命令封装: ${cmd}`)
}
for (const cmd of wrapped) {
  assert.ok(commands.includes(cmd), `前端封装了未注册的命令: ${cmd}`)
}
for (const cmd of mocked) {
  assert.ok(commands.includes(cmd), `mock 实现了未注册的命令: ${cmd}`)
}

const missing = commands.filter((c) => !mocked.has(c))
console.log(`IPC 契约 OK: 注册 ${commands.length} / 前端封装 ${wrapped.size} / E2E mock ${mocked.size}`)
if (missing.length) {
  console.log(`E2E mock 未覆盖（当前 A1-A3 验收不涉及）: ${missing.join(', ')}`)
}

const candidates = [
  process.env.CHROME_PATH,
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
  '/opt/google/chrome/chrome'
].filter(Boolean)
const chrome = candidates.find((p) => existsSync(p))
if (chrome) {
  console.log(`浏览器可用: ${chrome}`)
  console.log('提示: scripts/e2e-cdp.mjs 需另开终端启动 pnpm dev 后再执行')
} else {
  console.log('E2E SKIP: 本机未找到 Chrome/Chromium，且脚本默认路径为 Windows。')
  console.log('          CDP 验收按计划记为环境受限项；设置 CHROME_PATH 可在有浏览器的机器复现。')
}
