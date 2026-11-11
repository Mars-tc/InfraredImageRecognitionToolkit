#!/usr/bin/env node
// 本机构建入口：统一 Node / pnpm / cargo 的路径与版本自举，
// 提供环境自检、依赖安装、开发运行、本机调试包构建与测试转发。
//
// 用法：
//   node scripts/build-local.mjs env      环境自检（并提示 UOS20 交付限制）
//   node scripts/build-local.mjs install  安装前端依赖
//   node scripts/build-local.mjs dev      启动 Tauri 开发模式
//   node scripts/build-local.mjs build    构建本机调试包（不可交付 UOS20）
//   node scripts/build-local.mjs test     转发到 scripts/run-tests.mjs
//   node scripts/build-local.mjs clean    清理 dist 与 release 产物
//
// 说明：npm 上的 @tauri-apps/cli 1.x 只发布到 1.6.3，而 Cargo.lock 锁定的
// tauri crate 为 1.8.3，两者主次版本不一致时 CLI 会直接拒绝执行；
// 因此优先使用 cargo 版 tauri CLI（cargo install tauri-cli --version 1.8.3 --locked）。
//
// 注意：本机编译产物依赖本机 glibc（开发机为 2.38），
// 无法安装到 UOS20（Debian 10 底层，glibc 2.28）；
// 交付给 UOS20 的 deb / AppImage 请使用 GitHub Actions 流水线产出。
import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, rmSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const self = fileURLToPath(import.meta.url)
const root = join(dirname(self), '..')
const MIN_NODE_MAJOR = 18
const sep = process.platform === 'win32' ? ';' : ':'
const env = { ...process.env, COREPACK_ENABLE_DOWNLOAD_PROMPT: '0' }

function versionParts(v) {
  return String(v).replace(/^v/, '').split('.').map((n) => Number(n) || 0)
}

function compareVersion(a, b) {
  const pa = versionParts(a)
  const pb = versionParts(b)
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i]
  }
  return 0
}

/** Vite 8 的 engines 要求：node ^20.19.0 || >=22.12.0 */
function satisfiesVite(version) {
  const [maj = 0, min = 0] = versionParts(version)
  if (maj >= 22) return maj > 22 || min >= 12
  return maj === 20 && min >= 19
}

/** Node 不满足 Vite 8 要求时，从 NVM 目录挑一个可用版本重新执行自身。 */
function bootstrapNode() {
  if (satisfiesVite(process.versions.node)) return
  const candidates = []
  const bases = [process.env.NVM_DIR, join(homedir(), '.config', 'nvm'), join(homedir(), '.nvm')]
  for (const base of bases) {
    if (!base) continue
    const versionsDir = join(base, 'versions', 'node')
    if (!existsSync(versionsDir)) continue
    for (const name of readdirSync(versionsDir)) {
      const bin = join(versionsDir, name, 'bin', 'node')
      if (existsSync(bin)) candidates.push({ bin, version: name })
    }
  }
  const pick = (list) => list.sort((a, b) => compareVersion(b.version, a.version))[0]
  const preferred = pick(candidates.filter((c) => satisfiesVite(c.version)))
  const fallback = pick(candidates.filter((c) => compareVersion(c.version, `${MIN_NODE_MAJOR}.0.0`) >= 0))
  const chosen = preferred || fallback
  if (!chosen) {
    console.error(`当前 Node ${process.versions.node} 不满足 Vite 8 要求（^20.19 || >=22.12），且未找到可用版本`)
    process.exit(1)
  }
  if (!preferred) {
    console.warn(`未找到满足 Vite 8 要求的 Node，回退到 ${chosen.version}（前端构建可能失败）`)
  }
  const res = spawnSync(chosen.bin, [self, ...process.argv.slice(2)], { cwd: root, stdio: 'inherit', env })
  process.exit(res.status ?? 1)
}
bootstrapNode()

/** 把 ~/.cargo/bin 加入子进程 PATH，避免每条命令都手动 export。 */
function withCargoPath() {
  const cargoBin = join(homedir(), '.cargo', 'bin')
  if (existsSync(cargoBin) && !env.PATH.split(sep).includes(cargoBin)) {
    env.PATH = `${cargoBin}${sep}${env.PATH}`
  }
}
withCargoPath()

/** package.json 中声明的 pnpm 版本。 */
function declaredPnpmVersion() {
  try {
    const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
    const matched = /^pnpm@(.+)$/.exec(pkg.packageManager || '')
    return matched ? matched[1] : null
  } catch {
    return null
  }
}

/** 优先用 PATH 中且版本匹配的 pnpm；否则回退 corepack（按 packageManager 自动准备版本）。 */
function resolvePnpm() {
  const want = declaredPnpmVersion()
  const direct = spawnSync('pnpm', ['--version'], { encoding: 'utf8', env })
  if (direct.status === 0) {
    const version = (direct.stdout || '').trim()
    if (!want || version === want) return { cmd: 'pnpm', args: [] }
  }
  const corepack = spawnSync('corepack', ['pnpm', '--version'], { encoding: 'utf8', env })
  if (corepack.status === 0) return { cmd: 'corepack', args: ['pnpm'] }
  return null
}

/**
 * tauri CLI 解析：优先 cargo 版（与 tauri crate 版本一致），
 * 回退到 npm 版 pnpm tauri（版本不匹配时会被 CLI 拒绝）。
 */
function resolveTauriCli() {
  const cargoCli = spawnSync('cargo', ['tauri', '--version'], { encoding: 'utf8', env })
  if (cargoCli.status === 0) {
    const version = (cargoCli.stdout || '').trim().split('\n')[0]
    return { cmd: 'cargo', args: ['tauri'], version, source: 'cargo' }
  }
  const pnpm = resolvePnpm()
  if (pnpm) {
    const npmCli = spawnSync(pnpm.cmd, [...pnpm.args, 'tauri', '--version'], { encoding: 'utf8', env })
    if (npmCli.status === 0) {
      const version = (npmCli.stdout || '').trim().split('\n')[0]
      return { cmd: pnpm.cmd, args: [...pnpm.args, 'tauri'], version, source: 'npm' }
    }
  }
  return null
}

function run(cmd, args, options = {}) {
  const res = spawnSync(cmd, args, { cwd: root, stdio: 'inherit', env, ...options })
  if (res.error) {
    console.error(`执行失败：${cmd} ${args.join(' ')}（${res.error.message}）`)
    process.exit(1)
  }
  return res.status ?? 1
}

function pnpmOrExit() {
  const pnpm = resolvePnpm()
  if (!pnpm) {
    const want = declaredPnpmVersion()
    console.error('未找到可用的 pnpm。请先执行其一：')
    console.error(`  corepack enable pnpm        # 按 package.json 的 packageManager 自动准备 pnpm@${want || '指定版本'}（首次需联网）`)
    console.error(`  npm i -g pnpm@${want || '指定版本'}`)
    process.exit(1)
  }
  return pnpm
}

function pnpmRun(args) {
  const pnpm = pnpmOrExit()
  return run(pnpm.cmd, [...pnpm.args, ...args])
}

function tauriRun(args) {
  const cli = resolveTauriCli()
  if (!cli) {
    console.error('未找到 tauri CLI。请执行其一：')
    console.error('  cargo install tauri-cli --version 1.8.3 --locked   # 与 Cargo.lock 中的 tauri crate 版本一致（推荐）')
    console.error('  npm i -g @tauri-apps/cli@1.6.3                  # 与 tauri crate 1.8.3 不匹配，可能被拒绝')
    process.exit(1)
  }
  return run(cli.cmd, [...cli.args, ...args])
}

function probe(label, cmd, args) {
  const res = spawnSync(cmd, args, { encoding: 'utf8', env })
  if (res.status !== 0) {
    console.log(`${label}：未检测到`)
    return null
  }
  const out = `${res.stdout || ''}${res.stderr || ''}`.trim().split('\n')[0]
  console.log(`${label}：${out}`)
  return out
}

function showEnv() {
  console.log('=== 本机构建环境自检 ===')
  probe('Node', process.execPath, ['-v'])
  const pnpm = resolvePnpm()
  console.log(`pnpm：${pnpm ? `${pnpm.cmd} ${pnpm.args.join(' ')}`.trim() : '未检测到（可用 corepack enable pnpm）'}`)
  probe('cargo', 'cargo', ['--version'])
  probe('rustc', 'rustc', ['--version'])
  const cli = resolveTauriCli()
  console.log(`tauri CLI：${cli ? `${cli.version}（${cli.source}）` : '未检测到（建议 cargo install tauri-cli --version 1.8.3 --locked）'}`)
  probe('glibc', 'ldd', ['--version'])
  probe('webkit2gtk-4.0', 'pkg-config', ['--modversion', 'webkit2gtk-4.0'])
  console.log('')
  console.log('提示：本机构建产物依赖本机 glibc，不能安装到 UOS20（Debian 10，glibc 2.28）；')
  console.log('      交付给 UOS20 的 deb / AppImage 请使用 GitHub Actions 流水线产出。')
}

function clean() {
  for (const dir of [join(root, 'dist'), join(root, 'src-tauri', 'target', 'release')]) {
    if (existsSync(dir)) {
      rmSync(dir, { recursive: true, force: true })
      console.log(`已删除 ${dir}`)
    }
  }
}

const [command = 'env', ...rest] = process.argv.slice(2)

switch (command) {
  case 'env':
    showEnv()
    break
  case 'install':
    process.exit(pnpmRun(['install', ...rest]))
    break
  case 'dev':
    process.exit(tauriRun(['dev', ...rest]))
    break
  case 'build':
    console.warn('注意：本机产物不能交付 UOS20（glibc 不匹配），仅用于本机运行与调试。')
    process.exit(tauriRun(['build', ...rest]))
    break
  case 'test':
    process.exit(run(process.execPath, [join(root, 'scripts', 'run-tests.mjs'), ...rest]))
    break
  case 'clean':
    clean()
    break
  default:
    console.error(`未知命令：${command}`)
    console.error('可用命令：env | install | dev | build | test | clean')
    process.exit(1)
}
