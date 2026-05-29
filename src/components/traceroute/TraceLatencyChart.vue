<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'

const props = defineProps<{
  /** 选中要显示的跳号列表 */
  selectedHops: number[]
}>()

const { t } = useI18n()
const store = useContinuousTraceStore()

// ========== 配置参数 ==========
const POINT_GAP = 12                // 相邻点的横向间距（像素）
const LABEL_STEP = 10               // 每隔多少个数据点（按 seq）显示一个 X 轴刻度
const SLIDE_ANIM_MS = 220           // 新点滑入动画时长（ms）
const RIGHT_PAD = 16
const LEFT_PAD = 48
const TOP_PAD = 12
const BOTTOM_PAD = 26
const TIMEOUT_COLOR = '#F44336'

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

// ========== 数据快照 ==========
interface Frame {
  seq: number
  timestamp: number
}
// 当前所有可见跳的并集 seq 时间轴
const frames: Frame[] = []
// 每条跳号 → seq → latency（null 表示超时；undefined 表示该跳此 seq 无样本）
const hopValueMap = new Map<number, Map<number, number | null>>()
// 跳号 → 显示名（用于 tooltip / legend）
const hopNameMap = new Map<number, string>()

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
let viewportEnd = -1
let slideOffset = 0
let slideAnimStart = 0
let rafId: number | null = null
let needsRender = false

// ========== 鼠标交互 ==========
let dragging = false
let dragStartX = 0
let dragStartViewportEnd = 0
const hover = ref<{ x: number; frameIdx: number } | null>(null)

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
  scheduleRender()
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

// ========== 从 store 重建数据快照 ==========
function rebuildSnapshot(): { newMaxSeq: number; growBy: number } {
  // 收集选中跳的历史
  const histories = props.selectedHops
    .map(n => store.hopHistories.get(n))
    .filter((h): h is NonNullable<typeof h> => !!h && h.samples.length > 0)

  hopValueMap.clear()
  hopNameMap.clear()

  // 收集所有出现过的 seq
  const seqSet = new Set<number>()
  const seqToTime = new Map<number, number>()
  for (const h of histories) {
    const valueMap = new Map<number, number | null>()
    for (const s of h.samples) {
      seqSet.add(s.seq)
      if (!seqToTime.has(s.seq)) seqToTime.set(s.seq, s.timestamp)
      valueMap.set(s.seq, s.is_timeout ? null : s.latency_ms)
    }
    hopValueMap.set(h.hop_number, valueMap)
    hopNameMap.set(h.hop_number, `#${h.hop_number} ${h.ip}`)
  }

  const sortedSeqs = Array.from(seqSet).sort((a, b) => a - b)
  const oldLength = frames.length
  const oldLastSeq = oldLength > 0 ? frames[oldLength - 1].seq : -Infinity

  frames.length = 0
  for (const seq of sortedSeqs) {
    frames.push({ seq, timestamp: seqToTime.get(seq) ?? Date.now() })
  }

  totalCount.value = frames.length
  const newMaxSeq = frames.length > 0 ? frames[frames.length - 1].seq : -Infinity
  const growBy = oldLastSeq === -Infinity
    ? frames.length
    : Math.max(0, newMaxSeq - oldLastSeq)

  return { newMaxSeq, growBy }
}

// ========== 添加数据时的动画处理 ==========
function onSnapshotChanged(growBy: number) {
  if (frames.length === 0) {
    viewportEnd = -1
    slideOffset = 0
    return
  }
  if (isLiveMode.value) {
    viewportEnd = frames.length - 1
    if (growBy === 1) {
      slideOffset = POINT_GAP
      slideAnimStart = performance.now()
    } else {
      // 多帧或初始化：直接显示，不做动画
      slideOffset = 0
    }
  } else {
    // 历史模式：保持 viewportEnd 不超过最大
    if (viewportEnd >= frames.length) viewportEnd = frames.length - 1
  }
}

// ========== 重置 ==========
function reset() {
  frames.length = 0
  hopValueMap.clear()
  hopNameMap.clear()
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
  if (slideOffset > 0) {
    const elapsed = performance.now() - slideAnimStart
    const progress = Math.min(1, elapsed / SLIDE_ANIM_MS)
    const eased = 1 - Math.pow(1 - progress, 2)
    slideOffset = POINT_GAP * (1 - eased)
    if (progress < 1) needsRender = true
    else slideOffset = 0
  }
  if (needsRender) {
    needsRender = false
    draw()
  }
  if (slideOffset > 0) {
    rafId = requestAnimationFrame(renderFrame)
  }
}

// ========== 视口范围 ==========
function getVisibleCapacity(): number {
  const plotWidth = cssWidth - LEFT_PAD - RIGHT_PAD
  if (plotWidth <= 0) return 0
  return Math.floor(plotWidth / POINT_GAP) + 1
}

function getVisibleRange(): { startIdx: number; endIdx: number } {
  if (frames.length === 0 || viewportEnd < 0) {
    return { startIdx: 0, endIdx: -1 }
  }
  const capacity = getVisibleCapacity()
  const endIdx = Math.min(viewportEnd, frames.length - 1)
  const startIdx = Math.max(0, endIdx - capacity + 1)
  return { startIdx, endIdx }
}

// ========== Y 轴范围（取所有可见跳的最大延迟） ==========
function computeYAxis(startIdx: number, endIdx: number): { yMax: number; tickStep: number } {
  let max = 0
  for (let i = startIdx; i <= endIdx; i++) {
    const seq = frames[i].seq
    for (const valueMap of hopValueMap.values()) {
      const v = valueMap.get(seq)
      if (v != null && v > max) max = v
    }
  }
  if (max < 100) max = 100
  const niceMax = niceCeil(max * 1.15)
  const tickStep = niceCeil(niceMax / 5)
  const yMax = tickStep * 5
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

// ========== 绘制 ==========
function draw() {
  if (!ctx || cssWidth === 0 || cssHeight === 0) return
  const c = ctx
  c.clearRect(0, 0, cssWidth, cssHeight)

  const plotLeft = LEFT_PAD
  const plotRight = cssWidth - RIGHT_PAD
  const plotTop = TOP_PAD
  const plotBottom = cssHeight - BOTTOM_PAD
  const plotWidth = plotRight - plotLeft
  const plotHeight = plotBottom - plotTop
  if (plotWidth <= 0 || plotHeight <= 0) return

  const { startIdx, endIdx } = getVisibleRange()
  const hasData = endIdx >= startIdx
  const { yMax, tickStep } = hasData
    ? computeYAxis(startIdx, endIdx)
    : { yMax: 100, tickStep: 20 }

  // 1. 网格 + Y 轴标签
  c.strokeStyle = gridColor
  c.lineWidth = 1
  c.fillStyle = textColor
  c.font = '10px system-ui, -apple-system, sans-serif'
  c.textAlign = 'right'
  c.textBaseline = 'middle'
  for (let val = 0; val <= yMax; val += tickStep) {
    const y = plotBottom - (val / yMax) * plotHeight
    c.beginPath()
    c.moveTo(plotLeft, Math.round(y) + 0.5)
    c.lineTo(plotRight, Math.round(y) + 0.5)
    c.stroke()
    c.fillText(`${val}`, plotLeft - 6, y)
  }
  c.textAlign = 'left'
  c.textBaseline = 'top'
  c.fillText('ms', plotLeft - 24, 2)

  // 2. 坐标轴
  c.strokeStyle = axisColor
  c.beginPath()
  c.moveTo(plotLeft, plotBottom + 0.5)
  c.lineTo(plotRight, plotBottom + 0.5)
  c.moveTo(plotLeft + 0.5, plotTop)
  c.lineTo(plotLeft + 0.5, plotBottom)
  c.stroke()

  if (!hasData) return

  // 3. 折线（每个跳一条）
  const rightX = plotRight
  const offset = isLiveMode.value ? slideOffset : 0

  // 3.5 垂直网格线 + X 轴刻度位置（按 seq 锚定，每 LABEL_STEP 个点一个刻度）
  const labelIndices: number[] = []
  for (let i = startIdx; i <= endIdx; i++) {
    if (frames[i].seq % LABEL_STEP === 0) labelIndices.push(i)
  }
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

  c.save()
  c.beginPath()
  c.rect(plotLeft, plotTop - 2, plotWidth, plotHeight + 4)
  c.clip()

  // 按选中顺序绘制（保证颜色顺序与表格一致）
  for (const hopNumber of props.selectedHops) {
    const valueMap = hopValueMap.get(hopNumber)
    if (!valueMap) continue
    const color = colorForHop(hopNumber)

    // 折线
    c.strokeStyle = color
    c.lineWidth = 2
    c.lineJoin = 'round'
    c.lineCap = 'round'
    let pathStarted = false
    for (let i = startIdx; i <= endIdx; i++) {
      const seq = frames[i].seq
      const v = valueMap.get(seq)
      if (v === undefined || v === null) {
        pathStarted = false
        continue
      }
      const x = rightX - (endIdx - i) * POINT_GAP + offset
      const y = plotBottom - (v / yMax) * plotHeight
      if (!pathStarted) {
        c.beginPath()
        c.moveTo(x, y)
        pathStarted = true
      } else {
        c.lineTo(x, y)
      }
    }
    if (pathStarted) c.stroke()

    // 数据点（仅在间隔够大时绘制）
    if (POINT_GAP >= 8) {
      for (let i = startIdx; i <= endIdx; i++) {
        const seq = frames[i].seq
        const v = valueMap.get(seq)
        if (v === undefined) continue
        const x = rightX - (endIdx - i) * POINT_GAP + offset
        if (v === null) {
          // 超时点用 X 标记（用该跳颜色但带红框）
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
          const y = plotBottom - (v / yMax) * plotHeight
          c.fillStyle = color
          c.beginPath()
          c.arc(x, y, 2.5, 0, Math.PI * 2)
          c.fill()
        }
      }
    }
  }

  c.restore()

  // 4. X 轴时间标签（与上面的垂直网格线对齐）
  c.fillStyle = textColor
  c.textAlign = 'center'
  c.textBaseline = 'top'
  for (const i of labelIndices) {
    const f = frames[i]
    const x = rightX - (endIdx - i) * POINT_GAP + offset
    if (x < plotLeft - 1 || x > plotRight + 1) continue
    const d = new Date(f.timestamp)
    const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
    c.fillText(time, x, plotBottom + 4)
  }

  // 5. 实时模式右沿虚线
  if (isLiveMode.value && frames.length > 0) {
    c.strokeStyle = 'rgba(76, 175, 80, 0.25)'
    c.lineWidth = 1
    c.setLineDash([3, 3])
    c.beginPath()
    c.moveTo(rightX + 0.5, plotTop)
    c.lineTo(rightX + 0.5, plotBottom)
    c.stroke()
    c.setLineDash([])
  }

  // 6. Hover 高亮
  const h = hover.value
  if (h && h.frameIdx >= startIdx && h.frameIdx <= endIdx) {
    c.strokeStyle = 'rgba(127, 127, 127, 0.4)'
    c.lineWidth = 1
    c.setLineDash([2, 3])
    c.beginPath()
    c.moveTo(h.x + 0.5, plotTop)
    c.lineTo(h.x + 0.5, plotBottom)
    c.stroke()
    c.setLineDash([])
    // 每条跳在 hover 处放大点
    const seq = frames[h.frameIdx].seq
    for (const hopNumber of props.selectedHops) {
      const valueMap = hopValueMap.get(hopNumber)
      const v = valueMap?.get(seq)
      if (v == null) continue
      const y = plotBottom - (v / yMax) * plotHeight
      const color = colorForHop(hopNumber)
      c.fillStyle = '#fff'
      c.beginPath()
      c.arc(h.x, y, 4, 0, Math.PI * 2)
      c.fill()
      c.fillStyle = color
      c.beginPath()
      c.arc(h.x, y, 2.5, 0, Math.PI * 2)
      c.fill()
    }
  }
}

function pad2(n: number): string {
  return n.toString().padStart(2, '0')
}

// ========== 鼠标交互 ==========
function onMouseDown(e: MouseEvent) {
  if (frames.length === 0) return
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
    const movePoints = Math.round(dx / POINT_GAP)
    let newEnd = dragStartViewportEnd - movePoints
    const capacity = getVisibleCapacity()
    const minEnd = Math.min(frames.length - 1, capacity - 1)
    newEnd = Math.max(minEnd, Math.min(frames.length - 1, newEnd))
    viewportEnd = newEnd
    if (newEnd >= frames.length - 1) {
      isLiveMode.value = true
    } else {
      isLiveMode.value = false
      slideOffset = 0
    }
    scheduleRender()
    return
  }

  const { startIdx, endIdx } = getVisibleRange()
  if (endIdx < startIdx) {
    if (hover.value !== null) {
      hover.value = null
      scheduleRender()
    }
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
  const x = rightX - (endIdx - bestIdx) * POINT_GAP + offset
  hover.value = { x, frameIdx: bestIdx }
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
  if (frames.length === 0) return
  const delta = e.deltaY !== 0 ? e.deltaY : e.deltaX
  if (delta === 0) return
  e.preventDefault()
  const movePoints = delta > 0 ? -2 : 2
  let newEnd = viewportEnd + movePoints
  const capacity = getVisibleCapacity()
  const minEnd = Math.min(frames.length - 1, capacity - 1)
  newEnd = Math.max(minEnd, Math.min(frames.length - 1, newEnd))
  viewportEnd = newEnd
  if (newEnd >= frames.length - 1) {
    isLiveMode.value = true
  } else {
    isLiveMode.value = false
    slideOffset = 0
  }
  scheduleRender()
}

function backToLive() {
  isLiveMode.value = true
  viewportEnd = frames.length - 1
  slideOffset = 0
  scheduleRender()
}

// ========== Tooltip 内容 ==========
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
  if (!h || h.frameIdx < 0 || h.frameIdx >= frames.length) {
    return { time: '', rows: [] as TooltipRow[] }
  }
  const f = frames[h.frameIdx]
  const d = new Date(f.timestamp)
  const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
  const rows: TooltipRow[] = []
  for (const hopNumber of props.selectedHops) {
    const valueMap = hopValueMap.get(hopNumber)
    if (!valueMap) continue
    const v = valueMap.get(f.seq)
    if (v === undefined) continue
    rows.push({
      hop: hopNumber,
      name: hopNameMap.get(hopNumber) ?? `#${hopNumber}`,
      color: colorForHop(hopNumber),
      value: v === null ? '' : `${v.toFixed(1)} ms`,
      timeout: v === null
    })
  }
  return { time, rows }
})

const hasData = computed(() => totalCount.value > 0)

// ========== 监听 store 数据变化 ==========
// 用所有选中跳样本数之和作为变更信号（hopHistories 是 Map，每次 addHopResult 会重建引用）
const totalSamples = computed(() => {
  let s = 0
  for (const n of props.selectedHops) {
    const h = store.hopHistories.get(n)
    if (h) s += h.samples.length
  }
  return s
})

watch(totalSamples, () => {
  const { growBy } = rebuildSnapshot()
  onSnapshotChanged(growBy)
  scheduleRender()
})

// 选中跳变化时回到实时
watch(
  () => props.selectedHops.join(','),
  () => {
    isLiveMode.value = true
    viewportEnd = -1
    slideOffset = 0
    hover.value = null
    const { growBy } = rebuildSnapshot()
    // 选择切换不做滑入动画
    if (frames.length > 0) {
      viewportEnd = frames.length - 1
    }
    void growBy
    slideOffset = 0
    scheduleRender()
  }
)

// ========== 生命周期 ==========
onMounted(() => {
  const canvas = canvasRef.value
  if (canvas) ctx = canvas.getContext('2d')
  resizeCanvas()
  refreshThemeColors()
  if (wrapperRef.value && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => resizeCanvas())
    resizeObserver.observe(wrapperRef.value)
  }
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
  // 初始加载已有数据
  rebuildSnapshot()
  if (frames.length > 0) {
    viewportEnd = frames.length - 1
  }
  scheduleRender()
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

// 暴露给模板
const legendItems = computed(() => {
  return props.selectedHops
    .filter(n => hopValueMap.has(n) || true) // 即使无数据也显示
    .map(n => ({
      hop: n,
      color: colorForHop(n),
      name: hopNameMap.get(n) ?? `#${n}`
    }))
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
