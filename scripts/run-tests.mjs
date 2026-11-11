#!/usr/bin/env node
// 统一测试入口（零新增依赖）。
//
// 用法: node scripts/run-tests.mjs [--rust|--frontend|--sample|--e2e]
//  无参数 = 全部通道；也可通过 package.json 的 pnpm test 调用。
//
// 关键设计：
// - 显式激活工具链：本机 cargo 在 ~/.cargo/bin，Node 由 NVM 管理，
//   系统 PATH 里可能只有旧版 node（16）且没有 cargo，故入口自行探测/自举。
// - Rust 通道走 src-tauri/backend-tests（不链接 tauri），避免依赖
//   webkit2gtk-4.0 等 GTK 开发库，在精简开发机/CI 上同样可跑。
import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const self = fileURLToPath(import.meta.url)
const root = join(dirname(self), '..')
const args = process.argv.slice(2)
const MIN_NODE_MAJOR = 18
const sep = process.platform === 'win32' ? ';' : ':'

const versionParts = (v) => v.replace(/^v/, '').split('.').map((n) => Number(n) || 0)
function compareVersion(a, b) {
  const pa = versionParts(a)
  const pb = versionParts(b)
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i]
  }
  return 0
}

// Node < 18 没有 node:test：从 NVM 目录挑一个可用版本重新执行自身。
function bootstrap() {
  if (Number(process.versions.node.split('.')[0]) >= MIN_NODE_MAJOR) return
  const nvmDir = process.env.NVM_DIR || join(homedir(), '.config', 'nvm')
  const versionsDir = join(nvmDir, 'versions', 'node')
  const node = (existsSync(versionsDir) ? readdirSync(versionsDir) : [])
    .filter((v) => compareVersion(v, `v${MIN_NODE_MAJOR}.0.0`) >= 0)
    .sort(compareVersion)
    .reverse()
    .map((v) => join(versionsDir, v, 'bin', 'node'))
    .find((p) => existsSync(p))
  if (!node) {
    console.error(`当前 Node ${process.versions.node} 不支持 node:test，且未在 ${versionsDir} 找到 ≥18 的版本`)
    console.error('请先通过 NVM 安装 Node 18+，或设置 NVM_DIR 指向 NVM 目录')
    process.exit(1)
  }
  const r = spawnSync(node, [self, ...args], { cwd: root, stdio: 'inherit', env: process.env })
  process.exit(r.status ?? 1)
}
bootstrap()

const env = {
  ...process.env,
  PATH: [
    dirname(process.execPath),
    join(homedir(), '.cargo', 'bin'),
    process.env.PATH || ''
  ].filter(Boolean).join(sep),
  IR_SAMPLE_JPG: join(root, 'DJI_20260804234634_0002_T.JPG'),
  IR_SAMPLE_DJI: join(root, 'scripts', 'samples', 'DJI_0001_R.jpg')
}

const testFiles = readdirSync(join(root, 'tests'))
  .filter((f) => f.endsWith('.test.js'))
  .sort()
  .map((f) => join('tests', f))

const channels = [
  {
    key: 'rust',
    name: 'Rust 后端单测 + 真实样本全链路',
    cmd: 'cargo',
    args: ['test', '--offline'],
    cwd: join(root, 'src-tauri', 'backend-tests')
  },
  {
    key: 'frontend',
    name: '前端逻辑单测',
    cmd: 'node',
    args: ['--test', ...testFiles],
    cwd: root
  },
  {
    key: 'sample',
    name: '真实样本基线校验',
    cmd: 'node',
    args: ['scripts/verify-sample.mjs'],
    cwd: root
  },
  {
    key: 'e2e',
    name: 'E2E 现状核查',
    cmd: 'node',
    args: ['scripts/check-e2e.mjs'],
    cwd: root
  }
]

const flags = args.filter((a) => a.startsWith('--'))
const unknown = flags.filter((a) => !channels.some((c) => `--${c.key}` === a))
if (unknown.length) {
  console.error(`未知参数: ${unknown.join(', ')}`)
  console.error(`可用: ${channels.map((c) => `--${c.key}`).join(' ')}（不带参数则全部执行）`)
  process.exit(1)
}
const selected = flags.length ? channels.filter((c) => flags.includes(`--${c.key}`)) : channels

console.log(`工作区: ${root}`)
console.log(`工具链: Node ${process.versions.node} / 执行通道 ${selected.length} 个\n`)

const results = []
for (const ch of selected) {
  console.log(`──── ${ch.name} ────`)
  const started = Date.now()
  const r = spawnSync(ch.cmd, ch.args, { cwd: ch.cwd, env, stdio: 'inherit' })
  const ok = !r.error && r.status === 0
  results.push({ name: ch.name, ok, ms: Date.now() - started, error: r.error?.message })
  if (r.error) console.error(`${ch.cmd} 启动失败: ${r.error.message}`)
}

console.log('\n════ 测试汇总 ════')
for (const r of results) {
  console.log(`${r.ok ? 'PASS' : 'FAIL'}  ${r.name} (${(r.ms / 1000).toFixed(1)}s)`)
  if (r.error) console.log(`      ${r.error}`)
}
const failed = results.filter((r) => !r.ok).length
console.log(`${results.length - failed}/${results.length} 通道通过`)
if (failed) process.exit(1)
console.log('全部验收通过 ✓')
