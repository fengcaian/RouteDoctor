<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { LineChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { PingResult } from '@/types'
import { usePingStore } from '@/stores'

use([LineChart, GridComponent, TooltipComponent, DataZoomComponent, CanvasRenderer])

const props = defineProps<{
  target: string
}>()

const pingStore = usePingStore()

// ========== 配置参数 ==========
const MAX_DATA_POINTS = 720      // 最大数据点数（12 分钟 * 60 秒）
const VISIBLE_WINDOW = 120       // 默认可视窗口大小（120 个数据点 ≈ 2 分钟）

// ========== 数据缓冲 ==========
// 使用数组按顺序存储数据
const dataTimes = ref<string[]>([])  // 时间标签数组（按顺序）
const dataValues = ref<(number | null)[]>([])  // 对应的延迟值
const totalCount = ref(0)

const chartRef = ref<InstanceType<typeof VChart> | null>(null)

// 用于防抖：避免 setOption 重入
let rafId: number | null = null
let pendingUpdate = false
// 跟踪用户是否正在查看最新数据（用于决定是否自动滚动）
let isUserAtLatest = true

// ========== 初始化图表配置 ==========
// 使用时间轴模式：X 轴始终显示固定时间范围，数据从右向左移动
const chartOption = ref({
  backgroundColor: 'transparent',
  grid: {
    left: 50,
    right: 15,
    top: 15,
    bottom: 50
  },
  xAxis: {
    type: 'category',
    data: [] as string[],
    boundaryGap: false,
    axisLine: {
      lineStyle: { color: '#444' }
    },
    axisLabel: {
      color: '#888',
      fontSize: 10,
      rotate: 30,
      margin: 10,
      interval: 0
    }
  },
  yAxis: {
    type: 'value',
    name: 'ms',
    nameTextStyle: {
      color: '#888'
    },
    axisLine: {
      lineStyle: { color: '#444' }
    },
    axisLabel: {
      color: '#888',
      fontSize: 10
    },
    splitLine: {
      lineStyle: { color: '#333', opacity: 0.3 }
    }
  },
  series: [{
    name: 'Latency',
    type: 'line',
    data: [] as (number | null)[],
    smooth: false,
    symbol: 'circle',
    symbolSize: 3,
    lineStyle: {
      color: '#4CAF50',
      width: 1
    },
    itemStyle: {
      color: '#4CAF50'
    },
    connectNulls: false,
    animation: false,
    animationDuration: 0,
    animationDurationUpdate: 0
  }],
  dataZoom: [
    {
      type: 'inside',       // 支持鼠标滚轮 / 触摸板左右滚动
      xAxisIndex: [0],
      start: 0,
      end: 100,
      zoomOnMouseWheel: 'shift',  // Shift+滚轮缩放，普通滚轮左右平移
      moveOnMouseWheel: true,
      moveOnMouseMove: false,
      preventDefaultMouseMove: true,
      filterMode: 'none'          // 不过滤数据，保持完整数据集
    },
    {
      type: 'slider',       // 底部滑块，方便快速定位
      xAxisIndex: [0],
      start: 0,
      end: 100,
      showDetail: true,
      brushSelect: false,
      borderColor: '#555',
      fillerColor: 'rgba(76, 175, 80, 0.15)',
      handleStyle: { color: '#4CAF50', borderColor: '#4CAF50' },
      textStyle: { color: '#888' },
      bottom: 5,
      height: 18,
      filterMode: 'none',
      show: false  // 初始隐藏，有数据后显示
    }
  ],
  tooltip: {
    trigger: 'axis',
    backgroundColor: 'rgba(30, 30, 30, 0.9)',
    borderColor: '#444',
    textStyle: { color: '#fff' },
    formatter: (params: any) => {
      if (!params || !Array.isArray(params) || params.length === 0) return ''
      const p = params[0]
      if (!p) return ''
      // 空数据点或空标签不显示 tooltip
      if (p.value === null || p.value === undefined || !p.name || p.name === '') {
        return ''
      }
      return `${p.name}<br/>Latency: ${p.value.toFixed(1)} ms`
    }
  }
})

// ========== 核心：添加新数据 ==========
function addData(result: PingResult) {
  const date = new Date(result.timestamp)
  const timeStr = `${date.getHours().toString().padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}:${date.getSeconds().toString().padStart(2, '0')}`

  // 追加数据
  dataTimes.value.push(timeStr)
  dataValues.value.push(result.is_timeout ? null : result.latency_ms)

  // 超出最大数据点数时移除最旧的
  if (dataTimes.value.length > MAX_DATA_POINTS) {
    dataTimes.value.shift()
    dataValues.value.shift()
  }

  totalCount.value++

  // 更新图表
  scheduleUpdate()
}

// ========== 更新图表（rAF 防抖，避免 setOption 重入）==========
function scheduleUpdate() {
  pendingUpdate = true
  if (rafId !== null) return  // 已有待执行的帧，不重复注册
  rafId = requestAnimationFrame(() => {
    rafId = null
    if (!pendingUpdate) return
    pendingUpdate = false
    flushChartUpdate()
  })
}

function flushChartUpdate() {
  const chart = chartRef.value
  if (!chart || dataTimes.value.length === 0) return

  const total = dataTimes.value.length

  // 每 60 个数据点显示一个 X 轴刻度（约 1 分钟间隔）
  const labelInterval = 59

  // 计算可视窗口的百分比范围
  let zoomStart: number
  let zoomEnd: number

  if (total <= VISIBLE_WINDOW) {
    // 数据量不足一个窗口，显示全部
    zoomStart = 0
    zoomEnd = 100
  } else if (isUserAtLatest) {
    // 用户在查看最新数据，自动滚动到最右
    zoomEnd = 100
    zoomStart = 100 - (VISIBLE_WINDOW / total * 100)
  } else {
    // 用户手动滚动了，不改变当前位置
    return updateDataOnly(chart, labelInterval)
  }

  chart.setOption({
    xAxis: {
      type: 'category',
      data: dataTimes.value,
      boundaryGap: false,
      axisLine: { lineStyle: { color: '#444' } },
      axisLabel: {
        color: '#888',
        fontSize: 10,
        rotate: 30,
        margin: 15,
        interval: labelInterval,
        align: 'right'
      }
    },
    series: [{
      data: dataValues.value,
      type: 'line',
      smooth: false,
      symbol: 'circle',
      symbolSize: 3,
      lineStyle: { color: '#4CAF50', width: 1 },
      itemStyle: { color: '#4CAF50' },
      connectNulls: false,
      animation: false
    }],
    dataZoom: [
      { start: zoomStart, end: zoomEnd },
      { show: true, start: zoomStart, end: zoomEnd }
    ]
  })
}

// 仅更新数据，不改变用户当前的滚动位置
function updateDataOnly(chart: any, labelInterval: number) {
  chart.setOption({
    xAxis: {
      data: dataTimes.value,
      axisLabel: { interval: labelInterval }
    },
    yAxis: {},
    series: [{ data: dataValues.value, type: 'line' }],
    dataZoom: [
      {},       // inside：不传 start/end，保持用户位置
      { show: true }  // slider：保持可见
    ]
  })
}

// ========== 重置 ==========
function reset() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
  pendingUpdate = false
  isUserAtLatest = true

  dataTimes.value = []
  dataValues.value = []
  totalCount.value = 0

  const chart = chartRef.value
  if (chart) {
    chart.setOption({
      xAxis: {
        data: [],
        axisLabel: { interval: 59 }
      },
      yAxis: {},
      series: [{ data: [], type: 'line' }],
      dataZoom: [
        { start: 0, end: 100 },
        { show: false, start: 0, end: 100 }
      ]
    })
  }
}

// ========== 监听 store 变化 ==========
let lastSeq = -1
const unsubscribe = pingStore.$subscribe(() => {
  const results = pingStore.getResults(props.target)
  if (results.length === 0) {
    // Store was cleared (Clear Results button clicked), reset the chart
    reset()
    lastSeq = -1
    return
  }

  const newResults = results.filter(r => r.seq > lastSeq)
  if (newResults.length === 0) return

  newResults.forEach(r => {
    addData(r)
    lastSeq = Math.max(lastSeq, r.seq)
  })
})

// 监听 target 变化
watch(() => props.target, () => {
  reset()
  lastSeq = -1
})

// ========== 生命周期 ==========
onMounted(() => {
  nextTick(() => {
    // 监听 dataZoom 事件，判断用户是否在查看最新数据
    const chart = chartRef.value
    if (chart) {
      // 获取底层 echarts 实例来绑定事件
      const echartsInstance = (chart as any).chart || chart
      if (echartsInstance && typeof echartsInstance.on === 'function') {
        echartsInstance.on('datazoom', (params: any) => {
          const end = params.end ?? params.batch?.[0]?.end
          if (end !== undefined) {
            isUserAtLatest = end >= 99.5
          }
        })
      }
    }

    const results = pingStore.getResults(props.target)
    if (results.length > 0) {
      results.forEach(r => {
        addData(r)
        lastSeq = Math.max(lastSeq, r.seq)
      })
    }
  })
})

onUnmounted(() => {
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
  unsubscribe()
})
</script>

<template>
  <div class="ping-chart">
    <v-chart
      ref="chartRef"
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; height: 100%"
    />
    <div v-if="totalCount === 0" class="empty-chart">
      <p>暂无数据</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-chart {
  width: 100%;
  height: 280px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 12px;
  border: 1px solid var(--border-color);
  position: relative;
}

.empty-chart {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  color: var(--text-muted);
  font-size: 12px;
  pointer-events: none;
}
</style>