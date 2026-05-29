<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { LineChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, DataZoomComponent, LegendComponent, MarkLineComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { usePathMonitorStore } from '@/stores/pathMonitorStore'

use([LineChart, GridComponent, TooltipComponent, DataZoomComponent, LegendComponent, MarkLineComponent, CanvasRenderer])

const props = defineProps<{
  /** 选中显示的跳号集合 */
  selectedHops: number[]
}>()

const { t } = useI18n()
const pathStore = usePathMonitorStore()
const chartRef = ref<InstanceType<typeof VChart> | null>(null)

// ========== 配置 ==========
// 默认可视窗口（最近多少个采样点） —— 类似 preview.html 中的 MAX_VISIBLE_POINTS
const VISIBLE_WINDOW = 60
// 是否处于"实时模式"（自动跟随最新数据）
const isLiveMode = ref(true)
// 用户是否正在查看最新区段
let isUserAtLatest = true

// 调色板：为不同跳分配颜色
const HOP_COLORS = [
  '#4CAF50', // 绿
  '#2196F3', // 蓝
  '#FF9800', // 橙
  '#E91E63', // 粉
  '#9C27B0', // 紫
  '#00BCD4', // 青
  '#FFC107', // 黄
  '#F44336', // 红
  '#8BC34A', // 浅绿
  '#3F51B5'  // 靛蓝
]

function colorForHop(hopNumber: number): string {
  return HOP_COLORS[hopNumber % HOP_COLORS.length]
}

// ========== 计算图表数据 ==========
const chartData = computed(() => {
  const session = pathStore.session
  if (!session || props.selectedHops.length === 0) {
    return { xLabels: [] as string[], series: [] as any[], total: 0 }
  }

  // 选中的 hops
  const selected = session.hops.filter(h => props.selectedHops.includes(h.hopNumber))
  if (selected.length === 0) {
    return { xLabels: [] as string[], series: [] as any[], total: 0 }
  }

  // 以样本数最多的 hop 作为时间基准（正常情况下所有 hop 长度一致）
  const refHop = selected.reduce((a, b) => (a.samples.length >= b.samples.length ? a : b))
  const refSamples = refHop.samples

  // 时间标签：HH:mm:ss
  const xLabels = refSamples.map(s => {
    const d = new Date(s.timestamp)
    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`
  })

  // 每个选中 hop 一个 series
  const series = selected.map(hop => {
    const color = colorForHop(hop.hopNumber)
    // 按基准时间戳对齐：以参考 hop 的 timestamp 为索引去查找 hop 自身的 sample
    // 正常情况下两者长度一致，可直接 map
    let data: (number | null)[]
    if (hop.samples.length === refSamples.length) {
      data = hop.samples.map(s => s.latency)
    } else {
      // 容错：按 timestamp 匹配
      const map = new Map(hop.samples.map(s => [s.timestamp, s.latency]))
      data = refSamples.map(rs => (map.has(rs.timestamp) ? (map.get(rs.timestamp) as number | null) : null))
    }

    const ipLabel = hop.ip || '* * *'
    const name = `#${hop.hopNumber} ${ipLabel}`

    return {
      name,
      type: 'line',
      data,
      smooth: false,
      symbol: 'circle',
      symbolSize: 4,
      showSymbol: data.length <= 120, // 数据多时隐藏圆点，避免拥挤
      lineStyle: { color, width: 2 },
      itemStyle: { color },
      connectNulls: false,
      animation: false,
      // 超时点 (null) 自然断开 → 视觉提示丢包
    }
  })

  return { xLabels, series, total: refSamples.length }
})

// ========== 图表配置 ==========
const chartOption = computed(() => {
  const { xLabels, series, total } = chartData.value
  const labelInterval = total > 60 ? Math.floor(total / 8) : 4

  // 计算 dataZoom 范围：实时模式下右侧锁定
  let zoomStart = 0
  let zoomEnd = 100
  if (total > VISIBLE_WINDOW && isLiveMode.value) {
    zoomEnd = 100
    zoomStart = 100 - (VISIBLE_WINDOW / total) * 100
  } else if (total > VISIBLE_WINDOW && !isLiveMode.value) {
    // 非实时：保持上一次用户位置，由 dataZoom 事件维护，这里给默认
    zoomEnd = 100
    zoomStart = 100 - (VISIBLE_WINDOW / total) * 100
  }

  return {
    backgroundColor: 'transparent',
    grid: {
      left: 50,
      right: 20,
      top: 36,
      bottom: 50
    },
    legend: {
      show: series.length > 1,
      top: 4,
      left: 10,
      textStyle: { color: '#aaa', fontSize: 11 },
      itemWidth: 16,
      itemHeight: 8,
      icon: 'roundRect'
    },
    xAxis: {
      type: 'category',
      data: xLabels,
      boundaryGap: false,
      axisLine: { lineStyle: { color: '#444' } },
      axisLabel: {
        color: '#888',
        fontSize: 10,
        rotate: 30,
        margin: 10,
        interval: labelInterval,
        align: 'right'
      }
    },
    yAxis: {
      type: 'value',
      name: 'ms',
      nameTextStyle: { color: '#888', fontSize: 10 },
      axisLine: { lineStyle: { color: '#444' } },
      axisLabel: { color: '#888', fontSize: 10 },
      splitLine: { lineStyle: { color: '#333', opacity: 0.3 } },
      min: 0
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(20, 20, 20, 0.95)',
      borderColor: '#444',
      textStyle: { color: '#fff', fontSize: 12 },
      formatter: (params: any) => {
        if (!params || !Array.isArray(params) || params.length === 0) return ''
        const time = params[0]?.name || ''
        let html = `<div style="font-weight:600;margin-bottom:4px">${time}</div>`
        for (const p of params) {
          const v = p.value
          const valStr = v === null || v === undefined
            ? `<span style="color:#f44336">${t('pathLatency.timeout')}</span>`
            : `${(v as number).toFixed(1)} ms`
          html += `<div style="display:flex;align-items:center;gap:6px">
            <span style="display:inline-block;width:8px;height:8px;border-radius:2px;background:${p.color}"></span>
            <span style="flex:1">${p.seriesName}</span>
            <span style="font-weight:600">${valStr}</span>
          </div>`
        }
        return html
      }
    },
    dataZoom: [
      {
        type: 'inside',
        xAxisIndex: [0],
        start: zoomStart,
        end: zoomEnd,
        zoomOnMouseWheel: 'shift',
        moveOnMouseWheel: true,
        moveOnMouseMove: true,
        preventDefaultMouseMove: true,
        filterMode: 'none'
      },
      {
        type: 'slider',
        xAxisIndex: [0],
        start: zoomStart,
        end: zoomEnd,
        showDetail: false,
        brushSelect: false,
        borderColor: '#555',
        fillerColor: 'rgba(76, 175, 80, 0.15)',
        handleStyle: { color: '#4CAF50', borderColor: '#4CAF50' },
        textStyle: { color: '#888' },
        bottom: 4,
        height: 16,
        filterMode: 'none',
        show: total > VISIBLE_WINDOW
      }
    ],
    series
  }
})

// ========== 监听数据变化：实时模式下自动滚到最右 ==========
watch(
  () => pathStore.session?.totalSamples,
  () => {
    if (!isLiveMode.value) return
    nextTick(() => {
      const chart = chartRef.value
      if (!chart) return
      const { total } = chartData.value
      if (total <= VISIBLE_WINDOW) return
      const start = 100 - (VISIBLE_WINDOW / total) * 100
      ;(chart as any).setOption(
        {
          dataZoom: [
            { start, end: 100 },
            { start, end: 100 }
          ]
        },
        { lazyUpdate: true }
      )
    })
  }
)

// 当选中的跳变化时，重置为实时模式
watch(
  () => props.selectedHops.join(','),
  () => {
    isLiveMode.value = true
    isUserAtLatest = true
  }
)

// ========== 返回实时按钮 ==========
function backToLive() {
  isLiveMode.value = true
  isUserAtLatest = true
  const chart = chartRef.value
  if (!chart) return
  const { total } = chartData.value
  if (total <= VISIBLE_WINDOW) {
    ;(chart as any).setOption({
      dataZoom: [
        { start: 0, end: 100 },
        { start: 0, end: 100 }
      ]
    })
  } else {
    const start = 100 - (VISIBLE_WINDOW / total) * 100
    ;(chart as any).setOption({
      dataZoom: [
        { start, end: 100 },
        { start, end: 100 }
      ]
    })
  }
}

// ========== 监听 dataZoom 用户拖动 → 切换为历史模式 ==========
onMounted(() => {
  nextTick(() => {
    const chart = chartRef.value
    if (!chart) return
    const echartsInstance = (chart as any).chart || chart
    if (echartsInstance && typeof echartsInstance.on === 'function') {
      echartsInstance.on('datazoom', (params: any) => {
        const end = params.end ?? params.batch?.[0]?.end
        if (end === undefined) return
        const atLatest = end >= 99.5
        // 仅当用户主动拖离实时区域时退出实时
        if (!atLatest && isUserAtLatest) {
          isLiveMode.value = false
        } else if (atLatest && !isUserAtLatest) {
          isLiveMode.value = true
        }
        isUserAtLatest = atLatest
      })
    }
  })
})

onUnmounted(() => {
  // ECharts 组件销毁由 vue-echarts 自动处理
})

// ========== 是否有数据 ==========
const hasData = computed(() => chartData.value.total > 0)
</script>

<template>
  <div class="path-latency-chart">
    <!-- 顶部工具栏 -->
    <div class="chart-toolbar">
      <div class="status-group">
        <span class="status-dot" :class="{ live: isLiveMode }"></span>
        <span class="mode-text">
          {{ isLiveMode ? t('pathLatency.modeLive') : t('pathLatency.modeHistory') }}
        </span>
        <span class="count-text" v-if="hasData">
          {{ t('pathLatency.samples') }}: {{ chartData.total }}
        </span>
      </div>
      <button
        v-if="!isLiveMode && hasData"
        class="back-live-btn"
        @click="backToLive"
      >
        ⏵ {{ t('pathLatency.backToLive') }}
      </button>
    </div>

    <!-- 图表本体 -->
    <div class="chart-area">
      <v-chart
        v-if="hasData"
        ref="chartRef"
        :option="chartOption"
        :autoresize="true"
        style="width: 100%; height: 100%"
      />
      <div v-else class="empty">
        <p>{{ t('pathLatency.empty') }}</p>
        <p class="hint">{{ t('pathLatency.emptyHint') }}</p>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.path-latency-chart {
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

  &:hover {
    background: rgba(76, 175, 80, 0.25);
  }
}

.chart-area {
  flex: 1;
  min-height: 0;
  position: relative;
  padding: 4px;
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
