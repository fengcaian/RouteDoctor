// 跨平台 Npcap SDK 安装入口
//
// 在 Windows 上调用 src-tauri/scripts/setup-npcap-sdk.ps1 下载并解压 SDK。
// 在 macOS/Linux 上直接跳过（Npcap 是 Windows 专属，pcap crate 在那些平台用
// 系统的 libpcap，由包管理器提供）。
//
// 由 package.json 的 "tauri:build" / "tauri:dev" 脚本调用，做到一行命令完成构建。

import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import process from 'node:process'

const platform = process.platform
const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const RepoRoot = path.resolve(__dirname, '..')

if (platform !== 'win32') {
  console.log(`[setup-npcap-sdk] 当前平台 ${platform}，跳过 Npcap SDK（仅 Windows 需要）`)
  process.exit(0)
}

// Windows 下调用 PowerShell 脚本
const Ps1 = path.join(RepoRoot, 'src-tauri', 'scripts', 'setup-npcap-sdk.ps1')

console.log(`[setup-npcap-sdk] 调用 ${Ps1}`)

const child = spawn(
  'powershell',
  [
    '-NoProfile',
    '-ExecutionPolicy', 'Bypass',
    '-File', Ps1,
  ],
  { stdio: 'inherit' }
)

child.on('exit', (code) => {
  process.exit(code ?? 1)
})

child.on('error', (err) => {
  console.error('[setup-npcap-sdk] 启动 PowerShell 失败:', err)
  process.exit(1)
})
