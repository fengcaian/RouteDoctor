<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { LineChart } from 'echarts/charts'
import { GridComponent, TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { PingResult } from '@/types'
import { usePingStore } from '@/stores'

use([LineChart, GridComponent, TooltipComponent, CanvasRenderer])

const props = defineProps<{
  target: string
}>()

const pingStore = usePingStore()

// ========== 配置参数 ==========
const TICK_COUNT = 10            // X 轴固定刻度数
const MAX_DATA_POINTS = 200      // 最大数据点数

// ========== 数据缓冲 ==========
// 使用数组按顺序存储数据
const dataTimes = ref<string[]>([])  // 时间标签数组（按顺序）
const dataValues = ref<(number | null)[]>([])  // 对应的延迟值
const totalCount = ref(0)

const chartRef = ref<InstanceType<typeof VChart> | null>(null)

// 用于防抖：避免 setOption 重入
let rafId: number | null = null
let pendingUpdate = false

// ========== 初始化图表配置 ==========
// 使用时间轴模式：X 轴始终显示固定时间范围，数据从右向左移动
const chartOption = ref({
  backgroundColor: 'transparent',
  grid: {
    left: 50,
    right: 15,
    top: 15,
    bottom: 40
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
      width: 2
    },
    itemStyle: {
      color: '#4CAF50'
    },
    connectNulls: true,
    animation: false,
    animationDuration: 0,
    animationDurationUpdate: 0
  }],
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

  // 滑动窗口模式：只显示最近的数据
  const displayDataCount = Math.min(dataTimes.value.length, TICK_COUNT)

  // 截取最新的 displayDataCount 个数据点
  const visibleTimes = dataTimes.value.slice(-displayDataCount)
  const visibleValues = dataValues.value.slice(-displayDataCount)

  // 填充空位确保 X 轴稳定
  const xAxisLabels: string[] = []
  const seriesData: (number | null)[] = []

  const emptySlots = TICK_COUNT - visibleTimes.length

  // 左侧填充空位
  for (let i = 0; i < emptySlots; i++) {
    xAxisLabels.push('')
    seriesData.push(null)
  }

  xAxisLabels.push(...visibleTimes)
  seriesData.push(...visibleValues)

  // 使用完整配置避免合并问题
  chart.setOption({
    xAxis: {
      type: 'category',
      data: xAxisLabels,
      boundaryGap: false,
      axisLine: { lineStyle: { color: '#444' } },
      axisLabel: {
        color: '#888',
        fontSize: 10,
        rotate: 30,
        margin: 15,
        interval: 0,
        align: 'right'
      }
    },
    yAxis: {
      type: 'value',
      name: 'ms',
      nameTextStyle: { color: '#888' },
      axisLine: { lineStyle: { color: '#444' } },
      axisLabel: { color: '#888', fontSize: 10 },
      splitLine: { lineStyle: { color: '#333', opacity: 0.3 } }
    },
    series: [{
      data: seriesData,
      type: 'line',
      smooth: false,
      symbol: 'circle',
      symbolSize: 3,
      lineStyle: { color: '#4CAF50', width: 2 },
      itemStyle: { color: '#4CAF50' },
      connectNulls: true,
      animation: false
    }]
  })
}

// ========== 重置 ==========
function reset() {
  // 取消待执行的更新帧
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
  pendingUpdate = false

  dataTimes.value = []
  dataValues.value = []
  totalCount.value = 0

  const chart = chartRef.value
  if (chart) {
    chart.setOption({
      xAxis: {
        type: 'category',
        data: [],
        boundaryGap: false,
        axisLine: { lineStyle: { color: '#444' } },
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
        nameTextStyle: { color: '#888' },
        axisLine: { lineStyle: { color: '#444' } },
        axisLabel: { color: '#888', fontSize: 10 },
        splitLine: { lineStyle: { color: '#333', opacity: 0.3 } }
      },
      series: [{
        data: [],
        type: 'line',
        smooth: false,
        symbol: 'circle',
        symbolSize: 3,
        lineStyle: { color: '#4CAF50', width: 2 },
        itemStyle: { color: '#4CAF50' },
        connectNulls: true,
        animation: false
      }]
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
      <p>等待数据...</p>
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