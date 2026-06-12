<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PingResult } from '@/types'
import { usePingStore } from '@/stores'

// ========== Props / Stores / i18n ==========
const props = defineProps<{
  target: string
}>()

const pingStore = usePingStore()
const { t } = useI18n()

// ========== 配置参数 ==========
const MAX_DATA_POINTS = 720         // 最多保留 720 个点
const POINT_GAP = 12                // 相邻点的横向间距（像素）
const LABEL_STEP = 10               // 每隔多少个数据点（按 seq）显示一个 X 轴刻度
const SLIDE_ANIM_MS = 220           // 新点滑入动画时长（ms）
const RIGHT_PAD = 16                // 绘图区右侧内边距
const LEFT_PAD = 48                 // 给 Y 轴标签留出的左侧内边距
const TOP_PAD = 12                  // 顶部内边距
const BOTTOM_PAD = 26               // 底部 X 轴时间标签高度
const LINE_COLOR = '#4CAF50'        // 折线/正常点颜色
const TIMEOUT_COLOR = '#F44336'     // 超时点颜色

// 主题相关颜色（onMounted 时从 CSS 变量读取，主题切换时刷新）
let gridColor = 'rgba(127, 127, 127, 0.18)'
let axisColor = 'rgba(127, 127, 127, 0.5)'
let textColor = 'rgba(127, 127, 127, 0.85)'

// ========== 数据 ==========
interface Sample {
  seq: number
  timestamp: number
  latency: number | null
  isTimeout: boolean
}

const samples: Sample[] = []
const totalCount = ref(0)
const isLiveMode = ref(true)

// ========== Canvas 与尺寸 ==========
const wrapperRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let cssWidth = 0
let cssHeight = 0
let dpr = 1
let resizeObserver: ResizeObserver | null = null
let themeObserver: MutationObserver | null = null

// ========== 视口与动画 ==========
// viewportEnd: 当前可视区域最右侧对应的数据索引（包含），实时模式下等于 samples.length - 1
let viewportEnd = -1
// 平滑滚动的额外像素偏移（新点滑入动画时使用，0 表示动画结束）
let slideOffset = 0
let slideAnimStart = 0
// 渲染循环
let rafId: number | null = null
let needsRender = false

// ========== 鼠标交互 ==========
let dragging = false
let dragStartX = 0
let dragStartViewportEnd = 0
const hover = ref<{ x: number; y: number; sample: Sample } | null>(null)

// ========== 工具：保证 canvas 尺寸与 DPR 匹配 ==========
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
  scheduleRender()
}

// 从 CSS 变量与计算样式中读取主题色，使坐标轴在明/暗主题下都清晰可见
function refreshThemeColors() {
  const wrapper = wrapperRef.value
  if (!wrapper) return
  const styles = getComputedStyle(wrapper)
  const muted = styles.getPropertyValue('--text-muted').trim()
  const border = styles.getPropertyValue('--border-color').trim()
  const primary = styles.getPropertyValue('--text-primary').trim()
  if (muted) textColor = muted
  if (border) gridColor = border
  // 轴线颜色：取主文本色的弱化版本，保证在亮/暗主题下都有足够对比
  if (primary) {
    const rgb = parseColorToRGB(primary)
    if (rgb) axisColor = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.45)`
  }
}

// 解析 #RRGGBB / #RGB / rgb()/rgba() 为 RGB 分量
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

// ========== 计算可视点数量 ==========
function getVisibleCapacity(): number {
  const plotWidth = cssWidth - LEFT_PAD - RIGHT_PAD
  if (plotWidth <= 0) return 0
  // 可见点数：让最右侧点正好落在 cssWidth - RIGHT_PAD 上
  return Math.floor(plotWidth / POINT_GAP) + 1
}

// ========== 添加数据 ==========
function addData(result: PingResult) {
  samples.push({
    seq: result.seq,
    timestamp: result.timestamp,
    latency: result.is_timeout ? null : result.latency_ms,
    isTimeout: result.is_timeout
  })

  // 维持最大长度
  if (samples.length > MAX_DATA_POINTS) {
    const removeCount = samples.length - MAX_DATA_POINTS
    samples.splice(0, removeCount)
    if (!isLiveMode.value) {
      // 历史模式下保持 viewportEnd 对应的样本不变
      viewportEnd = Math.max(-1, viewportEnd - removeCount)
    }
  }

  totalCount.value = samples.length

  if (isLiveMode.value) {
    viewportEnd = samples.length - 1
    // 启动滑入动画：新点从右侧外面滑入
    slideOffset = POINT_GAP
    slideAnimStart = performance.now()
  }

  scheduleRender()
}

// ========== 重置 ==========
function reset() {
  samples.length = 0
  totalCount.value = 0
  viewportEnd = -1
  slideOffset = 0
  isLiveMode.value = true
  hover.value = null
  scheduleRender()
}

// ========== 渲染调度 ==========
function scheduleRender() {
  needsRender = true
  if (rafId !== null) return
  rafId = requestAnimationFrame(renderFrame)
}

function renderFrame() {
  rafId = null
  // 推进滑动动画
  if (slideOffset > 0) {
    const elapsed = performance.now() - slideAnimStart
    const progress = Math.min(1, elapsed / SLIDE_ANIM_MS)
    // ease-out
    const eased = 1 - Math.pow(1 - progress, 2)
    slideOffset = POINT_GAP * (1 - eased)
    if (progress < 1) {
      // 持续动画
      needsRender = true
    } else {
      slideOffset = 0
    }
  }

  if (needsRender) {
    needsRender = false
    draw()
  }

  if (slideOffset > 0) {
    rafId = requestAnimationFrame(renderFrame)
  }
}

// ========== 计算 Y 轴范围 ==========
function getVisibleRange(): { startIdx: number; endIdx: number } {
  if (samples.length === 0 || viewportEnd < 0) {
    return { startIdx: 0, endIdx: -1 }
  }
  const capacity = getVisibleCapacity()
  const endIdx = Math.min(viewportEnd, samples.length - 1)
  const startIdx = Math.max(0, endIdx - capacity + 1)
  return { startIdx, endIdx }
}

function computeYAxis(startIdx: number, endIdx: number): { yMin: number; yMax: number; tickStep: number } {
  let min = Infinity
  let max = -Infinity
  for (let i = startIdx; i <= endIdx; i++) {
    const v = samples[i]?.latency
    if (v != null) {
      if (v > max) max = v
      if (v < min) min = v
    }
  }
  // 没有有效数据时给一个默认范围
  if (!isFinite(min) || !isFinite(max)) {
    return { yMin: 0, yMax: 100, tickStep: 20 }
  }

  // 上下各留 20% padding，并保证至少展示 10ms 的跨度（避免完全一条直线时除零）
  const range = Math.max(max - min, 10)
  const padding = range * 0.2
  let yMin = Math.max(0, min - padding)
  let yMax = max + padding

  // 选 nice 的刻度间距，并把 yMin/yMax 对齐到刻度边界
  const span = yMax - yMin
  const tickStep = niceCeil(span / 5)
  yMin = Math.floor(yMin / tickStep) * tickStep
  yMax = Math.ceil(yMax / tickStep) * tickStep
  // 确保至少有 4 段（5 个刻度），保证图表纵向利用度
  while ((yMax - yMin) / tickStep < 4) yMax += tickStep

  return { yMin, yMax, tickStep }
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

// ========== 绘制主流程 ==========
function draw() {
  if (!ctx || cssWidth === 0 || cssHeight === 0) return
  const c = ctx
  c.clearRect(0, 0, cssWidth, cssHeight)

  // 绘图区
  const plotLeft = LEFT_PAD
  const plotRight = cssWidth - RIGHT_PAD
  const plotTop = TOP_PAD
  const plotBottom = cssHeight - BOTTOM_PAD
  const plotWidth = plotRight - plotLeft
  const plotHeight = plotBottom - plotTop

  if (plotWidth <= 0 || plotHeight <= 0) return

  const { startIdx, endIdx } = getVisibleRange()
  const hasData = endIdx >= startIdx
  const { yMin, yMax, tickStep } = hasData
    ? computeYAxis(startIdx, endIdx)
    : { yMin: 0, yMax: 100, tickStep: 20 }
  const ySpan = yMax - yMin || 1

  // ===== 1. 网格线 + Y 轴标签 =====
  c.strokeStyle = gridColor
  c.lineWidth = 1
  c.fillStyle = textColor
  c.font = '10px system-ui, -apple-system, sans-serif'
  c.textAlign = 'right'
  c.textBaseline = 'middle'

  for (let val = yMin; val <= yMax + 0.001; val += tickStep) {
    const y = plotBottom - ((val - yMin) / ySpan) * plotHeight
    c.beginPath()
    c.moveTo(plotLeft, Math.round(y) + 0.5)
    c.lineTo(plotRight, Math.round(y) + 0.5)
    c.stroke()
    // 刻度数字：整数显示无小数，非整数保留 1 位小数（避免 tickStep 为 2.5 等浮点 step 时显示混乱）
    const label = Number.isInteger(val) ? `${val}` : val.toFixed(1)
    c.fillText(label, plotLeft - 6, y)
  }

  // Y 轴单位（ms）— 放在左上角独立位置，水平上远离刻度数字（数字在 plotLeft-6 处右对齐），
  // 垂直上位于最大刻度数字之上，确保两者不重叠
  c.textAlign = 'left'
  c.textBaseline = 'top'
  c.fillStyle = textColor
  c.fillText('ms', 4, 4)

  // ===== 2. 坐标轴 =====
  c.strokeStyle = axisColor
  c.lineWidth = 1
  c.beginPath()
  // X 轴
  c.moveTo(plotLeft, plotBottom + 0.5)
  c.lineTo(plotRight, plotBottom + 0.5)
  // Y 轴
  c.moveTo(plotLeft + 0.5, plotTop)
  c.lineTo(plotLeft + 0.5, plotBottom)
  c.stroke()

  if (!hasData) {
    return
  }

  // ===== 3. 计算每个可见样本的 x 坐标 =====
  // 最右侧的可视位置（即 endIdx 这个点的位置，未应用 slideOffset 时）
  const rightX = plotRight
  // 应用滑动偏移（实时模式下新点从右外滑入，整体向左滑）
  const offset = isLiveMode.value ? slideOffset : 0

  // ===== 3.5 垂直网格线 + X 轴刻度位置（按 seq 锚定，每 LABEL_STEP 个点一个刻度） =====
  // 跟随数据滑动，避免传送带移动时刻度抖动
  const labelIndices: number[] = []
  for (let i = startIdx; i <= endIdx; i++) {
    if (samples[i].seq % LABEL_STEP === 0) labelIndices.push(i)
  }
  // 数据点过少时，至少显示首尾两个刻度
  if (labelIndices.length === 0 && endIdx >= startIdx) {
    labelIndices.push(endIdx)
    if (endIdx - startIdx >= LABEL_STEP) labelIndices.unshift(startIdx)
  }

  c.strokeStyle = gridColor
  c.lineWidth = 1
  for (const i of labelIndices) {
    const x = rightX - (endIdx - i) * POINT_GAP + offset
    if (x < plotLeft - 1 || x > plotRight + 1) continue
    const xRounded = Math.round(x) + 0.5
    c.beginPath()
    c.moveTo(xRounded, plotTop)
    c.lineTo(xRounded, plotBottom)
    c.stroke()
  }

  // 用 path 来 clip，避免数据画到 plot 外
  c.save()
  c.beginPath()
  c.rect(plotLeft, plotTop - 2, plotWidth, plotHeight + 4)
  c.clip()

  // ===== 4. 折线 =====
  c.strokeStyle = LINE_COLOR
  c.lineWidth = 2
  c.lineJoin = 'round'
  c.lineCap = 'round'

  let pathStarted = false
  for (let i = startIdx; i <= endIdx; i++) {
    const s = samples[i]
    if (s.isTimeout || s.latency == null) {
      pathStarted = false  // 超时点断开折线
      continue
    }
    const x = rightX - (endIdx - i) * POINT_GAP + offset
    const y = plotBottom - ((s.latency - yMin) / ySpan) * plotHeight
    if (!pathStarted) {
      c.beginPath()
      c.moveTo(x, y)
      pathStarted = true
    } else {
      c.lineTo(x, y)
    }
  }
  if (pathStarted) c.stroke()

  // ===== 5. 数据点 =====
  // 仅在可见点不太密时画点（避免太密）
  const drawDots = POINT_GAP >= 8
  if (drawDots) {
    for (let i = startIdx; i <= endIdx; i++) {
      const s = samples[i]
      const x = rightX - (endIdx - i) * POINT_GAP + offset
      if (s.isTimeout || s.latency == null) {
        // 超时画 X 标记
        const y = plotBottom - 2
        c.strokeStyle = TIMEOUT_COLOR
        c.lineWidth = 1.5
        c.beginPath()
        c.moveTo(x - 3, y - 3)
        c.lineTo(x + 3, y + 3)
        c.moveTo(x + 3, y - 3)
        c.lineTo(x - 3, y + 3)
        c.stroke()
      } else {
        const y = plotBottom - ((s.latency - yMin) / ySpan) * plotHeight
        c.fillStyle = LINE_COLOR
        c.beginPath()
        c.arc(x, y, 2.5, 0, Math.PI * 2)
        c.fill()
      }
    }
  }

  c.restore()

  // ===== 6. X 轴时间标签（与上面的垂直网格线对齐） =====
  c.fillStyle = textColor
  c.textAlign = 'center'
  c.textBaseline = 'top'
  for (const i of labelIndices) {
    const s = samples[i]
    const x = rightX - (endIdx - i) * POINT_GAP + offset
    if (x < plotLeft - 1 || x > plotRight + 1) continue
    const d = new Date(s.timestamp)
    const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
    c.fillText(time, x, plotBottom + 4)
  }

  // ===== 7. 实时模式时右侧"前沿"竖向高亮线 =====
  if (isLiveMode.value && samples.length > 0) {
    c.strokeStyle = 'rgba(76, 175, 80, 0.25)'
    c.lineWidth = 1
    c.setLineDash([3, 3])
    c.beginPath()
    c.moveTo(rightX + 0.5, plotTop)
    c.lineTo(rightX + 0.5, plotBottom)
    c.stroke()
    c.setLineDash([])
  }

  // ===== 8. Hover 提示线 =====
  const h = hover.value
  if (h) {
    c.strokeStyle = 'rgba(255, 255, 255, 0.25)'
    c.lineWidth = 1
    c.setLineDash([2, 3])
    c.beginPath()
    c.moveTo(h.x + 0.5, plotTop)
    c.lineTo(h.x + 0.5, plotBottom)
    c.stroke()
    c.setLineDash([])
    if (!h.sample.isTimeout && h.sample.latency != null) {
      const y = plotBottom - ((h.sample.latency - yMin) / ySpan) * plotHeight
      c.fillStyle = '#fff'
      c.beginPath()
      c.arc(h.x, y, 4, 0, Math.PI * 2)
      c.fill()
      c.fillStyle = LINE_COLOR
      c.beginPath()
      c.arc(h.x, y, 2.5, 0, Math.PI * 2)
      c.fill()
    }
  }
}

function pad2(n: number): string {
  return n.toString().padStart(2, '0')
}

// ========== 鼠标事件 ==========
function onMouseDown(e: MouseEvent) {
  if (samples.length === 0) return
  dragging = true
  dragStartX = e.clientX
  dragStartViewportEnd = viewportEnd
  hover.value = null
  scheduleRender()
}

function onMouseMove(e: MouseEvent) {
  const canvas = canvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const localX = e.clientX - rect.left
  const localY = e.clientY - rect.top

  if (dragging) {
    const dx = e.clientX - dragStartX
    // 拖动：每 POINT_GAP 像素 = 1 个数据点
    const movePoints = Math.round(dx / POINT_GAP)
    let newEnd = dragStartViewportEnd - movePoints
    const capacity = getVisibleCapacity()
    const minEnd = Math.min(samples.length - 1, capacity - 1)
    newEnd = Math.max(minEnd, Math.min(samples.length - 1, newEnd))
    viewportEnd = newEnd
    // 进入历史模式（除非用户拖回最右）
    if (newEnd >= samples.length - 1) {
      isLiveMode.value = true
    } else {
      isLiveMode.value = false
      slideOffset = 0
    }
    scheduleRender()
    return
  }

  // 没有拖动：处理 hover
  const { startIdx, endIdx } = getVisibleRange()
  if (endIdx < startIdx) {
    hover.value = null
    return
  }
  const plotLeft = LEFT_PAD
  const plotRight = cssWidth - RIGHT_PAD
  if (localX < plotLeft || localX > plotRight || localY < TOP_PAD || localY > cssHeight - BOTTOM_PAD) {
    if (hover.value !== null) {
      hover.value = null
      scheduleRender()
    }
    return
  }
  // 找最近的点
  const offset = isLiveMode.value ? slideOffset : 0
  const rightX = plotRight
  let bestIdx = endIdx
  let bestDist = Infinity
  for (let i = startIdx; i <= endIdx; i++) {
    const x = rightX - (endIdx - i) * POINT_GAP + offset
    const d = Math.abs(x - localX)
    if (d < bestDist) {
      bestDist = d
      bestIdx = i
    }
  }
  if (bestDist > POINT_GAP / 2 + 4) {
    if (hover.value !== null) {
      hover.value = null
      scheduleRender()
    }
    return
  }
  const s = samples[bestIdx]
  const x = rightX - (endIdx - bestIdx) * POINT_GAP + offset
  hover.value = { x, y: localY, sample: s }
  scheduleRender()
}

function onMouseUp() {
  dragging = false
}

function onMouseLeave() {
  dragging = false
  if (hover.value !== null) {
    hover.value = null
    scheduleRender()
  }
}

function onWheel(e: WheelEvent) {
  // 仅在按住 Ctrl/Cmd 时才平移视图;否则放行,让外层容器正常滚动页面。
  // 这与 Figma/Google Maps 的交互约定一致,避免用户想滚动页面时被图表"吃掉"滚轮事件。
  if (!e.ctrlKey && !e.metaKey) return
  if (samples.length === 0) return
  // 滚轮平移视图
  const delta = e.deltaY !== 0 ? e.deltaY : e.deltaX
  if (delta === 0) return
  e.preventDefault()
  // 向上滚动 -> 看历史（viewportEnd 减小）
  const movePoints = delta > 0 ? -2 : 2  // 向下滚动看更新的（向右）
  let newEnd = viewportEnd + movePoints
  const capacity = getVisibleCapacity()
  const minEnd = Math.min(samples.length - 1, capacity - 1)
  newEnd = Math.max(minEnd, Math.min(samples.length - 1, newEnd))
  viewportEnd = newEnd
  if (newEnd >= samples.length - 1) {
    isLiveMode.value = true
  } else {
    isLiveMode.value = false
    slideOffset = 0
  }
  scheduleRender()
}

function backToLive() {
  isLiveMode.value = true
  viewportEnd = samples.length - 1
  slideOffset = 0
  scheduleRender()
}

// ========== Tooltip 计算 ==========
const tooltipStyle = computed(() => {
  const h = hover.value
  if (!h) return { display: 'none' } as const
  // 鼠标右侧或左侧显示
  const showLeft = h.x > cssWidth / 2
  return {
    display: 'block',
    left: showLeft ? `${h.x - 12}px` : `${h.x + 12}px`,
    top: `${Math.max(8, h.y - 12)}px`,
    transform: showLeft ? 'translate(-100%, 0)' : 'none'
  } as const
})

const tooltipText = computed(() => {
  const h = hover.value
  if (!h) return { time: '', value: '', timeout: false }
  const s = h.sample
  const d = new Date(s.timestamp)
  const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
  if (s.isTimeout || s.latency == null) {
    return { time, value: '', timeout: true }
  }
  return { time, value: `${s.latency.toFixed(1)} ms`, timeout: false }
})

// ========== 监听 store 变化 ==========
let lastSeq = -1
const unsubscribe = pingStore.$subscribe(() => {
  const results = pingStore.getResults(props.target)
  if (results.length === 0) {
    reset()
    lastSeq = -1
    return
  }
  const newResults = results.filter(r => r.seq > lastSeq)
  if (newResults.length === 0) return
  for (const r of newResults) {
    addData(r)
    if (r.seq > lastSeq) lastSeq = r.seq
  }
})

// 监听 target 变化
watch(() => props.target, () => {
  reset()
  lastSeq = -1
  nextTick(() => {
    const results = pingStore.getResults(props.target)
    if (results.length > 0) {
      for (const r of results) {
        addData(r)
        if (r.seq > lastSeq) lastSeq = r.seq
      }
    }
  })
})

// ========== 生命周期 ==========
onMounted(() => {
  const canvas = canvasRef.value
  if (canvas) {
    ctx = canvas.getContext('2d')
  }
  // 初始尺寸 + 主题色
  resizeCanvas()
  refreshThemeColors()
  // 监听容器尺寸变化
  if (wrapperRef.value && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => resizeCanvas())
    resizeObserver.observe(wrapperRef.value)
  }
  // 监听主题切换（document.documentElement 的 data-theme 属性变化）
  if (typeof MutationObserver !== 'undefined') {
    themeObserver = new MutationObserver(() => {
      refreshThemeColors()
      scheduleRender()
    })
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme', 'class']
    })
  }
  // 加载已有数据
  const results = pingStore.getResults(props.target)
  if (results.length > 0) {
    for (const r of results) {
      addData(r)
      if (r.seq > lastSeq) lastSeq = r.seq
    }
  } else {
    scheduleRender()
  }
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
  unsubscribe()
})
</script>

<template>
  <div class="ping-chart">
    <!-- 顶部状态条 -->
    <div class="chart-toolbar">
      <div class="status-group">
        <span class="status-dot" :class="{ live: isLiveMode }"></span>
        <span class="mode-text">
          {{ isLiveMode ? t('traceLatency.modeLive') : t('traceLatency.modeHistory') }}
        </span>
        <span v-if="totalCount > 0" class="count-text">
          {{ t('traceLatency.samples') }}: {{ totalCount }}
        </span>
      </div>
      <button
        v-if="!isLiveMode && totalCount > 0"
        class="back-live-btn"
        @click="backToLive"
      >
        ⏵ {{ t('traceLatency.backToLive') }}
      </button>
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
        <div class="tooltip-time">{{ tooltipText.time }}</div>
        <div v-if="tooltipText.timeout" class="tooltip-timeout">
          {{ t('ping.timeoutText') }}
        </div>
        <div v-else class="tooltip-value">
          <span class="dot"></span>
          <span>{{ t('ping.latency') }}: {{ tooltipText.value }}</span>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="totalCount === 0" class="empty-chart">
        <p>{{ t('ping.noData') }}</p>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-chart {
  width: 100%;
  height: 280px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
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

  .tooltip-value {
    display: flex;
    align-items: center;
    gap: 6px;

    .dot {
      width: 8px;
      height: 8px;
      border-radius: 2px;
      background: #4CAF50;
    }
  }

  .tooltip-timeout {
    color: #F44336;
    font-weight: 600;
  }
}

.empty-chart {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 12px;
  pointer-events: none;
}
</style>
