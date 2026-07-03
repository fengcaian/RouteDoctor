// 用 sharp 从内联 SVG 渲染一张 1024x1024 的应用图标源图。
// 用途：作为 `npx tauri icon` 的源图，覆盖旧的 PP 文字图标。
// 说明：这是占位图标（RouteDoctor 主题：三跳路径 + "RD" 字样 + 深蓝渐变），
//      正式发布前建议替换为设计师提供的作品。

import sharp from 'sharp'
import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const projectRoot = path.resolve(__dirname, '..')

const SIZE = 1024

// SVG 说明：
// - 圆角方形深蓝→深黑渐变作为背景
// - 三个节点（起点/中间/终点）+ 虚线连接，象征 traceroute 三跳路径
// - 底部大字 "RD"（RouteDoctor 品牌缩写）
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${SIZE}" height="${SIZE}" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#1e40af"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>
    <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="8" result="blur"/>
      <feMerge>
        <feMergeNode in="blur"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
  </defs>
  <rect x="0" y="0" width="1024" height="1024" rx="200" fill="url(#bg)"/>
  <g filter="url(#glow)">
    <line x1="240" y1="380" x2="512" y2="380" stroke="#38bdf8" stroke-width="18" stroke-linecap="round" stroke-dasharray="4,36"/>
    <line x1="512" y1="380" x2="784" y2="380" stroke="#38bdf8" stroke-width="18" stroke-linecap="round" stroke-dasharray="4,36"/>
    <circle cx="240" cy="380" r="58" fill="#38bdf8"/>
    <circle cx="512" cy="380" r="58" fill="#ffffff"/>
    <circle cx="784" cy="380" r="58" fill="#38bdf8"/>
  </g>
  <text x="512" y="820" font-family="Segoe UI, Arial Black, Arial, sans-serif"
        font-size="260" font-weight="900" fill="#ffffff"
        text-anchor="middle" letter-spacing="8">RD</text>
</svg>`

const outputPath = path.join(projectRoot, 'app-icon.png')

await sharp(Buffer.from(svg), { density: 300 })
  .resize(SIZE, SIZE)
  .png()
  .toFile(outputPath)

console.log('已生成占位图标:', outputPath)
console.log('接下来请运行：npx tauri icon app-icon.png')
