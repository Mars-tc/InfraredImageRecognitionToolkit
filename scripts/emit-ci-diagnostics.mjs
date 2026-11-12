#!/usr/bin/env node
// CI 失败诊断：把构建日志发布为一个 check run。
// 公开仓库的 Actions 原始日志需要登录才能查看，而 check run 的 output.text 可匿名读取，
// 便于在 CI 失败时直接定位错误（例如构建命令在启动阶段就退出）。
//
// 用法（在 workflow 的 if: failure() 步骤中）：
//   GITHUB_TOKEN=${{ github.token }} node scripts/emit-ci-diagnostics.mjs /tmp/diagnostics.log
import { existsSync, readFileSync } from 'node:fs'

const logPath = process.argv[2] || '/tmp/diagnostics.log'
const token = process.env.GITHUB_TOKEN
const repo = process.env.GITHUB_REPOSITORY
const sha = process.env.GITHUB_SHA

if (!token || !repo || !sha) {
  console.error('缺少 GITHUB_TOKEN / GITHUB_REPOSITORY / GITHUB_SHA')
  process.exit(1)
}

const log = existsSync(logPath)
  ? readFileSync(logPath, 'utf8')
  : `（诊断日志不存在：${logPath}）`
const tail = log.slice(-20000)

const payload = {
  name: 'build-failure-diagnostics',
  head_sha: sha,
  status: 'completed',
  conclusion: 'neutral',
  output: {
    title: '构建失败诊断',
    summary: '构建 deb 步骤失败时的环境与输出',
    text: `\`\`\`\n${tail}\n\`\`\``
  }
}

const res = await fetch(`https://api.github.com/repos/${repo}/check-runs`, {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify(payload)
})

if (!res.ok) {
  console.error(`创建 check run 失败: ${res.status} ${await res.text()}`)
  process.exit(1)
}
console.log('构建失败诊断已发布为 check run')
