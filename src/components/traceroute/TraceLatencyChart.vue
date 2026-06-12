<script setup lang="ts">
/**
 * 路径监控延迟折线图（时间轴版）
 *
 * 设计：X 轴是真实时间（毫秒时间戳），不是数组索引。
 * - 视口由 [viewStart, viewEnd]（毫秒）表示，实时模式下 viewEnd = now，
 *   viewStart = now - windowMs
 * - 任意采样点的 X 坐标 = plotLeft + (sample.timestamp - viewStart) / windowMs * plotWidth
 * - 时间轴均匀刻度（10s/30s/1m/5m/...），不再"按数据点数"打标签
 * - 这样无论 ticker 跳拍、网络抖动、中途暂停，曲线都按真实时间放置——
 *   PingPlotter / SmokePing / Wireshark IO Graph 等专业工具都是这种实现
 */
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'

const props = defineProps<{
  /** 选中要显示的跳号列表 */
  selectedHops: number[]
}>()

const { t } = useI18n()
const store = useContinuousTraceStore()

// ========== 配置 ==========
const RIGHT_PAD = 16
const LEFT_PAD = 48
const TOP_PAD = 12
const BOTTOM_PAD = 26
const TIMEOUT_COLOR = '#F44336'

// 默认窗口：2 分钟（最近 120 秒）。可通过滚轮缩放。
const DEFAULT_WINDOW_MS = 120_000
const MIN_WINDOW_MS = 10_000      // 最小窗口 10 秒
const MAX_WINDOW_MS = 24 * 3600_000 // 最大窗口 24 小时

// 实时模式下持续滚动的刷新频率：浏览器 RAF 大概 60fps
// 不需要每帧都重画整个 canvas，只在数据变化或时间推进 > 一帧像素时重画

// 调色板：固定按跳号映射颜色，与表格中的色点保持一致
const HOP_COLORS = [
  '#4CAF50', '#2196F3', '#FF9800', '#E91E63', '#9C27B0',
  '#00BCD4', '#FFC107', '#F44336', '#8BC34A', '#3F51B5'
]
function colorForHop(hopNumber: number): string {
  return HOP_COLORS[hopNumber % HOP_COLORS.length]
}

// 主题相关颜色（onMounted 时从 CSS 变量读取，主题切换时刷新）
let gridColor = 'rgba(127, 127, 127, 0.18)'
let axisColor = 'rgba(127, 127, 127, 0.5)'
let textColor = 'rgba(127, 127, 127, 0.85)'

// ========== 状态 ==========
const totalCount = ref(0)            // 当前显示的样本总数（仅做"采样: N"展示）
const isLiveMode = ref(true)

// 视口起止时间（毫秒）。实时模式下 viewEnd 持续等于 now。
const viewStartMs = ref(0)
const viewEndMs = ref(0)
const windowMs = ref(DEFAULT_WINDOW_MS)

// 跳号 → 显示名（用于 tooltip / legend）
const hopNameMap = new Map<number, string>()

// ========== Canvas ==========
const wrapperRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let cssWidth = 0
let cssHeight = 0
let dpr = 1
let resizeObserver: ResizeObserver | null = null
let themeObserver: MutationObserver | null = null

// 渲染循环
let rafId: number | null = null

// 鼠标交互
let dragging = false
let dragStartX = 0
let dragStartViewStartMs = 0
let dragStartViewEndMs = 0
const hover = ref<{
  x: number
  timestamp: number  // 鼠标命中的时间点（用于 tooltip 显示）
} | null>(null)

// ========== Resize / 主题色 ==========
function resizeCanvas() {
  const canvas = canvasRef.value
  const wrapper = wrapperRef.value
  if (!canvas || !wrapper) return
  const rect = wrapper.getBoundingClientRect()
  cssWidth = Math.max(0, Math.floor(rect.width))
  cssHeight = Math.max(0, Math.floor(rect.height))
  dpr = window.devicePixelRatio || 1
  canvas.width = Math.floor(cssWidth * dpr)
  canvas.height = Math.floor(cssHeight * dpr)
  canvas.style.width = `${cssWidth}px`
  canvas.style.height = `${cssHeight}px`
  if (ctx) {
    ctx.setTransform(1, 0, 0, 1, 0, 0)
    ctx.scale(dpr, dpr)
  }
}

function refreshThemeColors() {
  const wrapper = wrapperRef.value
  if (!wrapper) return
  const styles = getComputedStyle(wrapper)
  const muted = styles.getPropertyValue('--text-muted').trim()
  const border = styles.getPropertyValue('--border-color').trim()
  const primary = styles.getPropertyValue('--text-primary').trim()
  if (muted) textColor = muted
  if (border) gridColor = border
  if (primary) {
    const rgb = parseColorToRGB(primary)
    if (rgb) axisColor = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.45)`
  }
}

function parseColorToRGB(color: string): { r: number; g: number; b: number } | null {
  if (!color) return null
  const c = color.trim()
  if (c.startsWith('#')) {
    let hex = c.slice(1)
    if (hex.length === 3) hex = hex.split('').map(ch => ch + ch).join('')
    if (hex.length !== 6) return null
    const num = parseInt(hex, 16)
    if (Number.isNaN(num)) return null
    return { r: (num >> 16) & 0xff, g: (num >> 8) & 0xff, b: num & 0xff }
  }
  const m = c.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/)
  if (m) return { r: +m[1], g: +m[2], b: +m[3] }
  return null
}

// ========== 时间 → X 坐标映射 ==========
function plotLeft() { return LEFT_PAD }
function plotRight() { return cssWidth - RIGHT_PAD }
function plotTop() { return TOP_PAD }
function plotBottom() { return cssHeight - BOTTOM_PAD }
function plotWidth() { return plotRight() - plotLeft() }
function plotHeight() { return plotBottom() - plotTop() }

function timeToX(timestamp: number): number {
  const span = viewEndMs.value - viewStartMs.value
  if (span <= 0) return plotLeft()
  return plotLeft() + (timestamp - viewStartMs.value) / span * plotWidth()
}

function xToTime(x: number): number {
  const span = viewEndMs.value - viewStartMs.value
  if (span <= 0) return viewEndMs.value
  return viewStartMs.value + (x - plotLeft()) / plotWidth() * span
}

// ========== 实时滚动循环 ==========
// RAF 持续推进 viewEnd = now，在实时模式 + 监控运行中保持画面"流动"
// 停止监控后画面冻结在最后采样时刻，等下次开始监控自动恢复滚动
function tick() {
  rafId = requestAnimationFrame(tick)

  // 历史模式下任何情况都不要让 RAF 推进视口,否则会把刚对齐到数据范围的 viewStart/End
  // 立刻覆盖到当前时间,导致用户看历史会话却看到"未来"
  if (isLiveMode.value && store.isRunning && !store.isHistoricalView) {
    const now = Date.now()
    viewEndMs.value = now
    viewStartMs.value = now - windowMs.value
  }

  draw()
}

// ========== 选中跳同步：每次 selectedHops 变化时重建 hopNameMap + totalCount ==========
function rebuildMeta() {
  hopNameMap.clear()
  let total = 0
  for (const n of props.selectedHops) {
    const h = store.hopHistories.get(n)
    if (h) {
      hopNameMap.set(n, `#${n} ${h.ip}`)
      total += h.samples.length
    }
  }
  totalCount.value = total
}

// ========== Y 轴范围 ==========
function computeYAxis(): { yMax: number; tickStep: number } {
  let max = 0
  for (const n of props.selectedHops) {
    const h = store.hopHistories.get(n)
    if (!h) continue
    for (const s of h.samples) {
      if (s.timestamp < viewStartMs.value || s.timestamp > viewEndMs.value) continue
      if (s.is_timeout || s.latency_ms == null) continue
      if (s.latency_ms > max) max = s.latency_ms
    }
  }
  if (max < 50) max = 50
  const target = max * 1.1
  const tickStep = niceCeil(target / 4)
  const yMax = Math.ceil(target / tickStep) * tickStep
  return { yMax, tickStep }
}

function niceCeil(v: number): number {
  if (v <= 0) return 10
  const exp = Math.floor(Math.log10(v))
  const base = Math.pow(10, exp)
  const m = v / base
  let nice
  if (m <= 1) nice = 1
  else if (m <= 2) nice = 2
  else if (m <= 5) nice = 5
  else nice = 10
  return nice * base
}

// ========== 时间轴刻度 ==========
// 根据当前窗口大小选择"漂亮"的刻度间隔（秒），保持 5-10 个刻度
function computeTimeTickStep(): number {
  const span = viewEndMs.value - viewStartMs.value
  // 候选刻度（毫秒）：1s, 2s, 5s, 10s, 15s, 30s, 1m, 2m, 5m, 10m, 30m, 1h, 2h, 6h, 12h
  const candidates = [
    1_000, 2_000, 5_000, 10_000, 15_000, 30_000,
    60_000, 2 * 60_000, 5 * 60_000, 10 * 60_000, 30 * 60_000,
    3600_000, 2 * 3600_000, 6 * 3600_000, 12 * 3600_000
  ]
  // 目标：6-10 个刻度
  const idealStep = span / 7
  for (const c of candidates) {
    if (c >= idealStep) return c
  }
  return candidates[candidates.length - 1]
}

/** 判断两个时间戳是否在同一天（按本地时区） */
function isSameDay(a: number, b: number): boolean {
  const da = new Date(a)
  const db = new Date(b)
  return da.getFullYear() === db.getFullYear()
    && da.getMonth() === db.getMonth()
    && da.getDate() === db.getDate()
}

/** 判断时间戳是否是今天 */
function isToday(ts: number): boolean {
  return isSameDay(ts, Date.now())
}

/** 把时间戳格式化成轴标签（窗口短显示秒，长显示日期）。
 * 当视图跨天或不在今天时，自动加上 MM-DD 前缀方便辨识历史数据日期。 */
function formatTickLabel(ts: number, step: number): string {
  const d = new Date(ts)
  const pad = (n: number) => n.toString().padStart(2, '0')
  // 窗口跨天 或 数据不是今天 → 在标签里加日期前缀
  const needDate = !isSameDay(viewStartMs.value, viewEndMs.value) || !isToday(ts)
  const datePrefix = needDate ? `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` : ''

  if (step >= 24 * 3600_000) {
    // 跨天：纯日期 MM-DD
    return `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  } else if (step >= 3600_000) {
    // 小时级：[MM-DD ]HH:MM
    return `${datePrefix}${pad(d.getHours())}:${pad(d.getMinutes())}`
  } else {
    // 分钟/秒级：[MM-DD ]HH:MM:SS
    return `${datePrefix}${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  }
}

// ========== 绘制 ==========
function draw() {
  if (!ctx || cssWidth === 0 || cssHeight === 0) return
  const c = ctx
  c.clearRect(0, 0, cssWidth, cssHeight)

  const pL = plotLeft()
  const pR = plotRight()
  const pT = plotTop()
  const pB = plotBottom()
  const pW = plotWidth()
  const pH = plotHeight()
  if (pW <= 0 || pH <= 0) return

  const { yMax, tickStep } = computeYAxis()

  // 1. Y 轴网格 + 标签
  c.strokeStyle = gridColor
  c.lineWidth = 1
  c.fillStyle = textColor
  c.font = '10px system-ui, -apple-system, sans-serif'
  c.textAlign = 'right'
  c.textBaseline = 'middle'
  for (let val = 0; val <= yMax; val += tickStep) {
    const y = pB - (val / yMax) * pH
    c.beginPath()
    c.moveTo(pL, Math.round(y) + 0.5)
    c.lineTo(pR, Math.round(y) + 0.5)
    c.stroke()
    c.fillText(`${val}`, pL - 6, y)
  }
  c.textAlign = 'left'
  c.textBaseline = 'top'
  c.fillText('ms', 4, 4)

  // 2. 坐标轴
  c.strokeStyle = axisColor
  c.beginPath()
  c.moveTo(pL, pB + 0.5)
  c.lineTo(pR, pB + 0.5)
  c.moveTo(pL + 0.5, pT)
  c.lineTo(pL + 0.5, pB)
  c.stroke()

  // 3. X 轴时间刻度（按真实时间均匀分布）
  const timeStep = computeTimeTickStep()
  // 找到第一个 >= viewStart 且能被 timeStep 整除的时间点（"对齐到整点"）
  const firstTick = Math.ceil(viewStartMs.value / timeStep) * timeStep
  c.strokeStyle = gridColor
  c.fillStyle = textColor
  c.textAlign = 'center'
  c.textBaseline = 'top'
  for (let t = firstTick; t <= viewEndMs.value; t += timeStep) {
    const x = timeToX(t)
    if (x < pL - 1 || x > pR + 1) continue
    const xR = Math.round(x) + 0.5
    c.beginPath()
    c.moveTo(xR, pT)
    c.lineTo(xR, pB)
    c.stroke()
    c.fillText(formatTickLabel(t, timeStep), x, pB + 4)
  }

  // 4. 折线（每条选中的跳）
  c.save()
  c.beginPath()
  c.rect(pL, pT - 2, pW, pH + 4)
  c.clip()

  // 当样本之间时间差超过这个阈值就不连线（表示丢拍/数据缺失）
  // 用每跳实际探测间隔的 2 倍作为阈值——前端不知道间隔，按 store 中的 maxSamplesPerHop / windowMs
  // 这里采用一个保守经验值：超过 5 秒的"间隔"就不连线
  // TODO：可以从 store 读 pingInterval 计算更精准的阈值
  const GAP_THRESHOLD_MS = 5_000

  for (const hopNumber of props.selectedHops) {
    const hop = store.hopHistories.get(hopNumber)
    if (!hop || hop.samples.length === 0) continue
    const color = colorForHop(hopNumber)

    // 找到视口内的样本范围（samples 已按 timestamp 升序排列）
    const samples = hop.samples
    // 二分找起点（视口左边界），但为了画丢线判断需要前后各多取 1 个
    let lo = 0, hi = samples.length
    while (lo < hi) {
      const mid = (lo + hi) >>> 1
      if (samples[mid].timestamp < viewStartMs.value) lo = mid + 1
      else hi = mid
    }
    const startIdx = Math.max(0, lo - 1)
    let endIdx = startIdx
    while (endIdx < samples.length && samples[endIdx].timestamp <= viewEndMs.value) {
      endIdx++
    }
    // endIdx 现在指向 viewEnd 之后第一个样本（或末尾）；多取一个用于绘制超出右边界的连线
    if (endIdx < samples.length) endIdx++

    // 绘折线
    c.strokeStyle = color
    c.lineWidth = 2
    c.lineJoin = 'round'
    c.lineCap = 'round'
    let pathStarted = false
    let lastTs = 0
    for (let i = startIdx; i < endIdx; i++) {
      const s = samples[i]
      if (s.is_timeout || s.latency_ms == null) {
        pathStarted = false
        continue
      }
      // 时间间隔超阈值则断线
      if (pathStarted && s.timestamp - lastTs > GAP_THRESHOLD_MS) {
        if (pathStarted) c.stroke()
        pathStarted = false
      }
      const x = timeToX(s.timestamp)
      const y = pB - (s.latency_ms / yMax) * pH
      if (!pathStarted) {
        c.beginPath()
        c.moveTo(x, y)
        pathStarted = true
      } else {
        c.lineTo(x, y)
      }
      lastTs = s.timestamp
    }
    if (pathStarted) c.stroke()

    // 数据点 —— 仅在每点占 >=8 像素时画，避免太密
    const pixelPerSample = pW / Math.max(1, endIdx - startIdx)
    if (pixelPerSample >= 6) {
      for (let i = startIdx; i < endIdx; i++) {
        const s = samples[i]
        const x = timeToX(s.timestamp)
        if (x < pL - 4 || x > pR + 4) continue
        if (s.is_timeout || s.latency_ms == null) {
          // 超时点：底部 X 标记
          const y = pB - 2
          c.strokeStyle = TIMEOUT_COLOR
          c.lineWidth = 1.5
          c.beginPath()
          c.moveTo(x - 3, y - 3)
          c.lineTo(x + 3, y + 3)
          c.moveTo(x + 3, y - 3)
          c.lineTo(x - 3, y + 3)
          c.stroke()
        } else {
          const y = pB - (s.latency_ms / yMax) * pH
          c.fillStyle = color
          c.beginPath()
          c.arc(x, y, 2.5, 0, Math.PI * 2)
          c.fill()
        }
      }
    }
  }

  c.restore()

  // 5. 实时模式右沿虚线
  if (isLiveMode.value) {
    c.strokeStyle = 'rgba(76, 175, 80, 0.25)'
    c.lineWidth = 1
    c.setLineDash([3, 3])
    c.beginPath()
    c.moveTo(pR + 0.5, pT)
    c.lineTo(pR + 0.5, pB)
    c.stroke()
    c.setLineDash([])
  }

  // 6. Hover 高亮：在 hover 时间点画一根竖线 + 各跳放大点
  const h = hover.value
  if (h && h.timestamp >= viewStartMs.value && h.timestamp <= viewEndMs.value) {
    const hx = timeToX(h.timestamp)
    c.strokeStyle = 'rgba(127, 127, 127, 0.4)'
    c.lineWidth = 1
    c.setLineDash([2, 3])
    c.beginPath()
    c.moveTo(hx + 0.5, pT)
    c.lineTo(hx + 0.5, pB)
    c.stroke()
    c.setLineDash([])
    // 每跳找最近的样本
    for (const hopNumber of props.selectedHops) {
      const hop = store.hopHistories.get(hopNumber)
      if (!hop) continue
      const s = findNearestSample(hop.samples, h.timestamp)
      if (!s || s.is_timeout || s.latency_ms == null) continue
      const x = timeToX(s.timestamp)
      const y = pB - (s.latency_ms / yMax) * pH
      const color = colorForHop(hopNumber)
      c.fillStyle = '#fff'
      c.beginPath()
      c.arc(x, y, 4, 0, Math.PI * 2)
      c.fill()
      c.fillStyle = color
      c.beginPath()
      c.arc(x, y, 2.5, 0, Math.PI * 2)
      c.fill()
    }
  }
}

/** samples 已按 timestamp 升序，二分找最接近 ts 的样本 */
function findNearestSample(samples: ReadonlyArray<{ timestamp: number; latency_ms: number | null; is_timeout: boolean }>, ts: number) {
  if (samples.length === 0) return null
  let lo = 0, hi = samples.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >>> 1
    if (samples[mid].timestamp < ts) lo = mid + 1
    else hi = mid
  }
  // lo 指向第一个 >= ts 的；与前一个比较谁更近
  if (lo === 0) return samples[0]
  const a = samples[lo - 1]
  const b = samples[lo]
  return Math.abs(a.timestamp - ts) <= Math.abs(b.timestamp - ts) ? a : b
}

// ========== 鼠标交互 ==========
function onMouseDown(e: MouseEvent) {
  dragging = true
  dragStartX = e.clientX
  dragStartViewStartMs = viewStartMs.value
  dragStartViewEndMs = viewEndMs.value
  hover.value = null
}

function onMouseMove(e: MouseEvent) {
  const canvas = canvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const localX = e.clientX - rect.left
  const localY = e.clientY - rect.top

  if (dragging) {
    const dx = e.clientX - dragStartX
    // 把像素拖动量换算成时间偏移：拖动一个 plotWidth 等于一个 windowMs
    const pW = plotWidth()
    if (pW <= 0) return
    const timeShift = -dx / pW * windowMs.value
    viewStartMs.value = dragStartViewStartMs + timeShift
    viewEndMs.value = dragStartViewEndMs + timeShift
    // 进入历史模式
    if (viewEndMs.value < Date.now() - 200) {
      isLiveMode.value = false
    } else {
      // 拖回最右等于实时
      isLiveMode.value = true
    }
    return
  }

  // 没拖动：处理 hover
  const pL = plotLeft()
  const pR = plotRight()
  if (localX < pL || localX > pR || localY < TOP_PAD || localY > cssHeight - BOTTOM_PAD) {
    if (hover.value !== null) hover.value = null
    return
  }
  hover.value = {
    x: localX,
    timestamp: xToTime(localX)
  }
}

function onMouseUp() {
  dragging = false
}

function onMouseLeave() {
  dragging = false
  if (hover.value !== null) hover.value = null
}

function onWheel(e: WheelEvent) {
  // 仅在按住 Ctrl/Cmd 时才缩放图表;否则放行,让外层容器正常滚动页面。
  // 这与 Figma/Google Maps 的交互约定一致,避免用户想滚动页面时被图表"吃掉"滚轮事件。
  if (!e.ctrlKey && !e.metaKey) return

  e.preventDefault()
  const delta = e.deltaY !== 0 ? e.deltaY : e.deltaX
  if (delta === 0) return

  // 滚轮缩放：以鼠标 x 处的时间为锚点缩放窗口
  const canvas = canvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const localX = e.clientX - rect.left
  const anchorTime = xToTime(localX)

  const factor = delta > 0 ? 1.25 : 0.8  // 向下滚 = 放大窗口（看更多时间），向上滚 = 缩小
  const newWindow = Math.max(MIN_WINDOW_MS, Math.min(MAX_WINDOW_MS, windowMs.value * factor))
  if (newWindow === windowMs.value) return

  // 缩放后让 anchorTime 仍位于鼠标处
  const ratioFromStart = (anchorTime - viewStartMs.value) / windowMs.value
  windowMs.value = newWindow
  viewStartMs.value = anchorTime - ratioFromStart * windowMs.value
  viewEndMs.value = viewStartMs.value + windowMs.value

  // 如果右边界已经接近现在，回到实时
  if (viewEndMs.value >= Date.now() - 200) {
    isLiveMode.value = true
  } else {
    isLiveMode.value = false
  }
}

function backToLive() {
  isLiveMode.value = true
  // viewStart/viewEnd 由 RAF tick 自动更新
}

// ========== Tooltip ==========
const tooltipStyle = computed(() => {
  const h = hover.value
  if (!h) return { display: 'none' } as const
  const showLeft = h.x > cssWidth / 2
  return {
    display: 'block',
    left: showLeft ? `${h.x - 12}px` : `${h.x + 12}px`,
    top: `${TOP_PAD + 8}px`,
    transform: showLeft ? 'translate(-100%, 0)' : 'none'
  } as const
})

interface TooltipRow {
  hop: number
  name: string
  color: string
  value: string
  timeout: boolean
}

const tooltipData = computed(() => {
  const h = hover.value
  if (!h) return { time: '', rows: [] as TooltipRow[] }
  const d = new Date(h.timestamp)
  const pad = (n: number) => n.toString().padStart(2, '0')
  // 非今天的数据自动加日期前缀,方便查看历史会话时分辨日期
  const datePrefix = isToday(h.timestamp)
    ? ''
    : `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} `
  const time = `${datePrefix}${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  const rows: TooltipRow[] = []
  for (const hopNumber of props.selectedHops) {
    const hop = store.hopHistories.get(hopNumber)
    if (!hop) continue
    const s = findNearestSample(hop.samples, h.timestamp)
    if (!s) continue
    // 太远的样本（超过 windowMs/plotWidth × 20 像素）不算命中
    if (Math.abs(s.timestamp - h.timestamp) > windowMs.value / Math.max(1, plotWidth()) * 20) continue
    rows.push({
      hop: hopNumber,
      name: hopNameMap.get(hopNumber) ?? `#${hopNumber}`,
      color: colorForHop(hopNumber),
      value: s.is_timeout || s.latency_ms == null ? '' : `${s.latency_ms.toFixed(1)} ms`,
      timeout: s.is_timeout || s.latency_ms == null
    })
  }
  return { time, rows }
})

const hasData = computed(() => totalCount.value > 0)

const legendItems = computed(() => {
  return props.selectedHops.map(n => ({
    hop: n,
    color: colorForHop(n),
    name: hopNameMap.get(n) ?? `#${n}`
  }))
})

// ========== 监听 store / props 变化 ==========
// store 数据变化时，更新 totalCount 和 hopNameMap（只是元数据）
const totalSamplesSignal = computed(() => {
  let s = 0
  for (const n of props.selectedHops) {
    const h = store.hopHistories.get(n)
    if (h) s += h.samples.length
  }
  return s
})
watch(totalSamplesSignal, () => {
  rebuildMeta()
})

// 选中跳变化时，回到实时模式（历史模式下保持视口对齐到数据范围，不切回实时）
watch(
  () => props.selectedHops.join(','),
  () => {
    if (!store.isHistoricalView) {
      isLiveMode.value = true
    }
    hover.value = null
    rebuildMeta()
  }
)

// 历史会话加载时（loadHistoricalSession 改了 hopHistories），把视口缩放到刚好
// 覆盖整段会话数据，让用户一眼看到完整范围。
//
// 监听 [isHistoricalView, loadedSessionId] 组合:
//   - 进入/退出历史模式 → isHistoricalView 变化触发
//   - 历史模式下切换另一个历史会话 → isHistoricalView 始终 true,但 loadedSessionId 变化触发
// 单独监听 isHistoricalView 会漏掉"历史→历史"的切换(同值不触发)。
//
// flush:'post' 确保在 DOM/响应式更新完成后再读 hopHistories,避免拿到中间状态。
watch(
  () => [store.isHistoricalView, store.loadedSessionId] as const,
  ([isHist]) => {
    if (isHist) {
      // 扫描所有跳的样本，找出全局最早 / 最晚时间点
      let minTs = Infinity
      let maxTs = 0
      for (const hop of store.hopHistories.values()) {
        if (hop.samples.length === 0) continue
        const first = hop.samples[0].timestamp
        const last = hop.samples[hop.samples.length - 1].timestamp
        if (first < minTs) minTs = first
        if (last > maxTs) maxTs = last
      }

      if (maxTs > 0 && minTs < Infinity) {
        // 两边各留 5% 余量，避免曲线贴边
        const span = Math.max(MIN_WINDOW_MS, maxTs - minTs)
        const padding = span * 0.05
        windowMs.value = Math.min(MAX_WINDOW_MS, span + padding * 2)
        viewStartMs.value = minTs - padding
        viewEndMs.value = viewStartMs.value + windowMs.value
        isLiveMode.value = false
      }
    } else {
      // 退出历史视图时回到默认窗口大小，恢复实时模式
      windowMs.value = DEFAULT_WINDOW_MS
      isLiveMode.value = true
    }
    rebuildMeta()
  },
  { flush: 'post' }
)

// ========== 生命周期 ==========
onMounted(() => {
  const canvas = canvasRef.value
  if (canvas) ctx = canvas.getContext('2d')
  resizeCanvas()
  refreshThemeColors()

  // 初始化视口
  // 如果挂载时已经处于历史视图(组件可能因 v-if 重新挂载),不要把视口推到当前时间,
  // 而是直接对齐到数据范围,避免画面闪一下"未来时间"再被 watcher 拨回。
  if (store.isHistoricalView) {
    let minTs = Infinity
    let maxTs = 0
    for (const hop of store.hopHistories.values()) {
      if (hop.samples.length === 0) continue
      const first = hop.samples[0].timestamp
      const last = hop.samples[hop.samples.length - 1].timestamp
      if (first < minTs) minTs = first
      if (last > maxTs) maxTs = last
    }
    if (maxTs > 0 && minTs < Infinity) {
      const span = Math.max(MIN_WINDOW_MS, maxTs - minTs)
      const padding = span * 0.05
      windowMs.value = Math.min(MAX_WINDOW_MS, span + padding * 2)
      viewStartMs.value = minTs - padding
      viewEndMs.value = viewStartMs.value + windowMs.value
      isLiveMode.value = false
    } else {
      const now = Date.now()
      viewEndMs.value = now
      viewStartMs.value = now - windowMs.value
    }
  } else {
    const now = Date.now()
    viewEndMs.value = now
    viewStartMs.value = now - windowMs.value
  }

  if (wrapperRef.value && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => resizeCanvas())
    resizeObserver.observe(wrapperRef.value)
  }
  if (typeof MutationObserver !== 'undefined') {
    themeObserver = new MutationObserver(() => refreshThemeColors())
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme', 'class']
    })
  }

  rebuildMeta()
  // 启动渲染循环
  rafId = requestAnimationFrame(tick)
})

onUnmounted(() => {
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (themeObserver) {
    themeObserver.disconnect()
    themeObserver = null
  }
})
</script>

<template>
  <div class="trace-latency-chart">
    <!-- 顶部状态条 -->
    <div class="chart-toolbar">
      <div class="status-group">
        <span class="status-dot" :class="{ live: isLiveMode }"></span>
        <span class="mode-text">
          {{ isLiveMode ? t('traceLatency.modeLive') : t('traceLatency.modeHistory') }}
        </span>
        <span class="count-text" v-if="hasData">
          {{ t('traceLatency.samples') }}: {{ totalCount }}
        </span>
      </div>
      <button
        v-if="!isLiveMode && hasData"
        class="back-live-btn"
        @click="backToLive"
      >
        ⏵ {{ t('traceLatency.backToLive') }}
      </button>
    </div>

    <!-- 图例 -->
    <div v-if="legendItems.length > 1" class="chart-legend">
      <div
        v-for="item in legendItems"
        :key="item.hop"
        class="legend-item"
      >
        <span class="legend-dot" :style="{ background: item.color }"></span>
        <span class="legend-name">{{ item.name }}</span>
      </div>
    </div>

    <!-- Canvas 区域 -->
    <div
      ref="wrapperRef"
      class="canvas-wrapper"
      :title="t('traceLatency.wheelHint')"
      @mousedown="onMouseDown"
      @mousemove="onMouseMove"
      @mouseup="onMouseUp"
      @mouseleave="onMouseLeave"
      @wheel="onWheel"
    >
      <canvas ref="canvasRef" class="chart-canvas" />

      <!-- Tooltip -->
      <div class="tooltip" :style="tooltipStyle">
        <div class="tooltip-time">{{ tooltipData.time }}</div>
        <div
          v-for="row in tooltipData.rows"
          :key="row.hop"
          class="tooltip-row"
        >
          <span class="dot" :style="{ background: row.color }"></span>
          <span class="name">{{ row.name }}</span>
          <span v-if="row.timeout" class="timeout">{{ t('traceLatency.timeout') }}</span>
          <span v-else class="value">{{ row.value }}</span>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="!hasData" class="empty">
        <p>{{ t('traceLatency.empty') }}</p>
        <p class="hint">{{ t('traceLatency.emptyHint') }}</p>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.trace-latency-chart {
  width: 100%;
  height: 100%;
  min-height: 320px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.chart-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.1);
  flex-shrink: 0;
}

.status-group {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  color: var(--text-muted);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);

  &.live {
    background: #4CAF50;
    box-shadow: 0 0 6px rgba(76, 175, 80, 0.6);
    animation: pulse 1.4s infinite;
  }
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.mode-text {
  font-weight: 600;
  color: var(--text-primary);
}

.count-text {
  font-size: 11px;
}

.back-live-btn {
  padding: 4px 12px;
  border: 1px solid #4CAF50;
  background: rgba(76, 175, 80, 0.12);
  color: #4CAF50;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(76, 175, 80, 0.25); }
}

.chart-legend {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border-color);
  font-size: 11px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.legend-dot {
  width: 10px;
  height: 4px;
  border-radius: 2px;
}

.canvas-wrapper {
  flex: 1;
  min-height: 0;
  position: relative;
  cursor: grab;
  user-select: none;

  &:active {
    cursor: grabbing;
  }
}

.chart-canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.tooltip {
  position: absolute;
  pointer-events: none;
  background: rgba(20, 20, 20, 0.95);
  border: 1px solid #444;
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 12px;
  color: #fff;
  white-space: nowrap;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  z-index: 10;

  .tooltip-time {
    font-weight: 600;
    margin-bottom: 4px;
    font-size: 11px;
    color: #ccc;
  }

  .tooltip-row {
    display: flex;
    align-items: center;
    gap: 6px;
    line-height: 1.6;

    .dot {
      width: 8px;
      height: 8px;
      border-radius: 2px;
    }

    .name {
      flex: 1;
      margin-right: 8px;
    }

    .value {
      font-weight: 600;
    }

    .timeout {
      color: #F44336;
      font-weight: 600;
    }
  }
}

.empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 13px;
  gap: 4px;

  .hint {
    font-size: 11px;
    opacity: 0.7;
  }
}
</style>
