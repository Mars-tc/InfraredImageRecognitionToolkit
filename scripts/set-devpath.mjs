// 临时把 tauri.conf.json 的 devPath 改为带 ?img= 样本参数的地址（真机自验用）
// 用法: node scripts/set-devpath.mjs set|reset [样本路径]
import { readFileSync, writeFileSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const confPath = join(root, 'src-tauri', 'tauri.conf.json')
const mode = process.argv[2] || 'set'
const DEFAULT_DEV_PATH = 'http://127.0.0.1:5173'

let sample = process.argv[3]
if (!sample) {
  const guess = join(root, 'DJI_20260804234634_0002_T.JPG')
  if (existsSync(guess)) sample = guess
}
if (!sample) {
  console.error('未找到样本，请传入路径')
  process.exit(1)
}

const conf = JSON.parse(readFileSync(confPath, 'utf8'))
if (mode === 'set') {
  conf.build.devPath = `${DEFAULT_DEV_PATH}/?img=${encodeURIComponent(sample)}`
  console.log('devPath =>', conf.build.devPath)
} else {
  conf.build.devPath = DEFAULT_DEV_PATH
  console.log('devPath =>', conf.build.devPath)
}
writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n')
