<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { HeatmapChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, VisualMapComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { useContinuousTrace, useContinuousTraceListener } from '@/composables/useContinuousTrace'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'
import { useToast } from '@/composables/useToast'
import type { ContinuousTraceHopResult, PathDiscovered } from '@/composables/useContinuousTrace'

use([HeatmapChart, GridComponent, TooltipComponent, VisualMapComponent, DataZoomComponent, CanvasRenderer])

const { t } = useI18n()
const store = useContinuousTraceStore()
const toast = useToast()
const { startContinuousTrace, stopContinuousTrace } = useContinuousTrace()

const targetInput = ref('google.com')
const pingInterval = ref(2000)
const maxHops = ref(30)

// 监听事件
useContinuousTraceListener(
  (data: PathDiscovered) => {
    store.setPath(data)
    toast.success(`路径发现完成：${data.hops.length} 跳，开始持续监控`)
  },
  (result: ContinuousTraceHopResult) => {
    store.addHopResult(result)
  },
  (errorMsg: string) => {
    toast.error(errorMsg)
    store.stopMonitoring()
  },
  (_target: string) => {
    store.stopMonitoring()
  }
)

// 热力图配置
const heatmapOption = computed(() => {
  const { times, hops, data } = store.getHeatmapData()

  if (data.length === 0) {
    return { series: [] }
  }

  // 动态计算 visualMap 的 max 值：取实际数据的 P95 或最大值的 1.2 倍
  const validValues = data
    .map(d => d[2])
    .filter((v): v is number => v !== null && v > 0)
    .sort((a, b) => a - b)

  let dynamicMax = 50 // 最小 50ms
  if (validValues.length > 0) {
    // 取 P95 值，避免极端值拉伸色阶
    const p95Index = Math.floor(validValues.length * 0.95)
    const p95Value = validValues[p95Index] || validValues[validValues.length - 1]
    dynamicMax = Math.max(dynamicMax, Math.ceil(p95Value * 1.3))
  }

  return {
    backgroundColor: 'transparent',
    tooltip: {
      position: 'top',
      backgroundColor: 'rgba(30, 30, 30, 0.95)',
      borderColor: '#555',
      textStyle: { color: '#fff', fontSize: 12 },
      formatter: (params: any) => {
        const value = params.value[2]
        const hopLabel = hops[params.value[1]] || ''
        const time = times[params.value[0]] || ''
        if (value === null || value === -1) {
          return `${hopLabel}<br/>${time}<br/>超时`
        }
        return `${hopLabel}<br/>${time}<br/>延迟: ${value.toFixed(1)} ms`
      }
    },
    grid: {
      left: 160,
      right: 60,
      top: 10,
      bottom: 60
    },
    xAxis: {
      type: 'category',
      data: times,
      splitArea: { show: false },
      axisLabel: {
        color: '#888',
        fontSize: 9,
        interval: Math.max(0, Math.floor(times.length / 8) - 1),
        rotate: 45
      },
      axisLine: { lineStyle: { color: '#444' } }
    },
    yAxis: {
      type: 'category',
      data: hops,
      splitArea: { show: false },
      axisLabel: {
        color: '#888',
        fontSize: 10,
        width: 140,
        overflow: 'truncate'
      },
      axisLine: { lineStyle: { color: '#444' } }
    },
    dataZoom: [
      {
        type: 'slider',
        xAxisIndex: 0,
        bottom: 5,
        height: 16,
        start: Math.max(0, 100 - (30 / Math.max(times.length, 1)) * 100),
        end: 100,
        borderColor: '#555',
        fillerColor: 'rgba(76, 175, 80, 0.15)',
        handleStyle: { color: '#4CAF50', borderColor: '#4CAF50' },
        textStyle: { color: '#888', fontSize: 9 },
        show: times.length > 30
      },
      {
        type: 'inside',
        xAxisIndex: 0,
        zoomOnMouseWheel: 'shift',
        moveOnMouseWheel: true
      }
    ],
    visualMap: {
      min: 0,
      max: dynamicMax,
      calculable: true,
      orient: 'vertical',
      right: 0,
      top: 'center',
      itemHeight: 120,
      textStyle: { color: '#888' },
      inRange: {
        color: ['#2196F3', '#4CAF50', '#8BC34A', '#FFEB3B', '#FF9800', '#f44336']
      },
      formatter: (value: number) => `${Math.round(value)}ms`
    },
    series: [{
      name: 'Latency',
      type: 'heatmap',
      data: data.map(d => [d[0], d[1], d[2] === null ? -1 : d[2]]),
      label: { show: false },
      emphasis: {
        itemStyle: {
          shadowBlur: 10,
          shadowColor: 'rgba(0, 0, 0, 0.5)'
        }
      },
      itemStyle: {
        borderWidth: 1,
        borderColor: 'rgba(0,0,0,0.1)'
      }
    }]
  }
})

// 每跳统计表格数据
const hopStats = computed(() => {
  return store.hops
    .filter(h => h.ip)
    .map(h => ({
      ...h,
      stats: store.getHopStats(h.hop_number)
    }))
})

async function handleStart() {
  if (!targetInput.value.trim()) return
  store.startMonitoring(targetInput.value.trim())
  try {
    await startContinuousTrace(
      targetInput.value.trim(),
      maxHops.value,
      3000,
      pingInterval.value
    )
  } catch (e: any) {
    toast.error(`启动失败: ${typeof e === 'string' ? e : e.message || '未知错误'}`)
    store.stopMonitoring()
  }
}

async function handleStop() {
  try {
    await stopContinuousTrace(store.target)
  } catch (e) {
    // ignore
  }
  store.stopMonitoring()
}
</script>

<template>
  <div class="continuous-trace-view">
    <div class="view-header">
      <div>
        <h2>{{ t('continuousTrace.title') }}</h2>
        <p class="subtitle">{{ t('continuousTrace.subtitle') }}</p>
      </div>
    </div>

    <!-- 配置区 -->
    <div class="config-section">
      <div class="config-row">
        <div class="config-field target-field">
          <label class="config-label">{{ t('common.target') }}</label>
          <input
            v-model="targetInput"
            type="text"
            class="config-input"
            :placeholder="t('common.targetPlaceholder')"
            :disabled="store.isRunning"
            @keyup.enter="handleStart"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('continuousTrace.interval') }}</label>
          <input
            v-model.number="pingInterval"
            type="number"
            class="config-input"
            min="1000"
            max="10000"
            step="500"
            :disabled="store.isRunning"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.maxHops') }}</label>
          <input
            v-model.number="maxHops"
            type="number"
            class="config-input"
            min="5"
            max="64"
            :disabled="store.isRunning"
          />
        </div>
        <div class="config-field action-field">
          <label class="config-label">&nbsp;</label>
          <button
            v-if="!store.isRunning"
            class="start-btn"
            @click="handleStart"
            :disabled="!targetInput.trim()"
          >
            {{ t('continuousTrace.start') }}
          </button>
          <button
            v-else
            class="stop-btn"
            @click="handleStop"
          >
            {{ t('continuousTrace.stop') }}
          </button>
        </div>
      </div>
    </div>

    <!-- 发现中状态 -->
    <div v-if="store.isDiscovering" class="discovering-state">
      <div class="discovering-spinner"></div>
      <span>{{ t('continuousTrace.discovering') }}</span>
    </div>

    <!-- 热力图 -->
    <div v-if="store.hops.length > 0" class="heatmap-section">
      <h3 class="section-title">{{ t('continuousTrace.heatmapTitle') }}</h3>
      <div class="heatmap-container">
        <v-chart
          :option="heatmapOption"
          :autoresize="true"
          style="width: 100%; height: 100%"
        />
      </div>
    </div>

    <!-- 每跳统计表格 -->
    <div v-if="hopStats.length > 0" class="stats-section">
      <h3 class="section-title">{{ t('continuousTrace.statsTitle') }}</h3>
      <table class="stats-table">
        <thead>
          <tr>
            <th>#</th>
            <th>IP</th>
            <th>{{ t('ping.avg') }}</th>
            <th>{{ t('ping.min') }}</th>
            <th>{{ t('ping.max') }}</th>
            <th>{{ t('traceroute.lossRate') }}</th>
            <th>{{ t('continuousTrace.samples') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="hop in hopStats" :key="hop.hop_number">
            <td>{{ hop.hop_number }}</td>
            <td class="mono">{{ hop.ip }}</td>
            <td :class="getLatencyClass(hop.stats.avg)">{{ hop.stats.avg > 0 ? `${hop.stats.avg.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.stats.min > 0 ? `${hop.stats.min.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.stats.max > 0 ? `${hop.stats.max.toFixed(1)} ms` : '--' }}</td>
            <td :class="{ 'loss-high': hop.stats.loss > 5 }">{{ hop.stats.loss.toFixed(1) }}%</td>
            <td class="muted">{{ hop.stats.count }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 空状态 -->
    <div v-if="!store.isRunning && store.hops.length === 0 && !store.isDiscovering" class="empty-state">
      <div class="empty-icon">🗺️</div>
      <p>{{ t('continuousTrace.emptyHint') }}</p>
    </div>
  </div>
</template>

<script lang="ts">
function getLatencyClass(avg: number): string {
  if (avg === 0) return ''
  if (avg < 50) return 'good'
  if (avg < 100) return 'medium'
  return 'bad'
}
</script>

<style lang="scss" scoped>
.continuous-trace-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  min-width: 0;
}

.view-header {
  h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }
  .subtitle {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
}

.config-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 12px 16px;
}

.config-row {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  flex-wrap: wrap;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 4px;

  &.target-field { flex: 1; min-width: 200px; }
  &.action-field { width: auto; }
}

.config-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.config-input {
  padding: 8px 10px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
  color: var(--text-primary);
  font-size: 13px;

  &:focus {
    outline: none;
    border-color: var(--accent-color);
  }

  &:disabled {
    opacity: 0.6;
  }
}

.start-btn {
  padding: 8px 20px;
  background: var(--accent-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) { background: var(--accent-color-hover); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
}

.stop-btn {
  padding: 8px 20px;
  background: var(--error-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;

  &:hover { background: var(--error-color-hover); }
}

.discovering-state {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  color: var(--text-secondary);
  font-size: 13px;

  .discovering-spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border-color);
    border-top-color: var(--accent-color);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 12px 0;
}

.heatmap-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.heatmap-container {
  width: 100%;
  height: 300px;
  min-width: 0;
}

.stats-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.stats-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 12px;

  th {
    padding: 8px 10px;
    text-align: left;
    color: var(--text-muted);
    font-weight: 600;
    border-bottom: 1px solid var(--border-color);
  }

  td {
    padding: 6px 10px;
    color: var(--text-primary);
    border-bottom: 1px solid var(--border-color);

    &.mono { font-family: monospace; font-size: 11px; }
    &.muted { color: var(--text-muted); }
    &.good { color: #4CAF50; }
    &.medium { color: #FF9800; }
    &.bad { color: #f44336; }
    &.loss-high { color: #f44336; font-weight: 600; }
  }

  tr:hover td { background: var(--hover-bg); }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  min-height: 200px;
  color: var(--text-muted);
  gap: 8px;

  .empty-icon { font-size: 40px; opacity: 0.5; }
  p { font-size: 13px; margin: 0; }
}
</style>
