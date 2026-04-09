<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { TreeChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { TracerouteResult, HopResult } from '@/types'
import { useTracerouteStore } from '@/stores'

use([TreeChart, TooltipComponent, CanvasRenderer])

const props = defineProps<{
  target: string
}>()

const traceStore = useTracerouteStore()

const result = computed<TracerouteResult | undefined>(() => traceStore.getResult(props.target))
const isRunning = computed(() => traceStore.isRunning(props.target))
const currentHop = computed(() => traceStore.getCurrentHop(props.target))

const hops = computed<HopResult[]>(() => result.value?.hops || [])

const treeData = computed(() => {
  if (!result.value) return null

  const hopsList = result.value.hops
  if (hopsList.length === 0) return null

  // Build tree structure with better layout
  const buildNode = (hop: HopResult) => ({
    name: `hop_${hop.hop_number}`,
    value: hop.avg_latency || 0,
    label: {
      show: true,
      formatter: `${hop.hop_number}`,
      position: 'inside',
      fontSize: 11,
      color: '#fff'
    },
    itemStyle: {
      color: !hop.ip ? '#F44336' : (hop.avg_latency && hop.avg_latency < 50 ? '#4CAF50' : '#FF9800')
    },
    children: []
  })

  // Build nodes
  const nodes = hopsList.map((hop) => buildNode(hop))

  // Build tree hierarchy
  for (let i = nodes.length - 1; i > 0; i--) {
    nodes[i - 1].children = [nodes[i]]
  }

  return {
    name: 'local',
    label: { show: true, formatter: '本机', position: 'inside', fontSize: 11, color: '#fff' },
    itemStyle: { color: '#2196F3' },
    children: nodes.length > 0 ? [nodes[0]] : []
  }
})

const chartOption = computed(() => ({
  backgroundColor: 'transparent',
  grid: {
    left: 10,
    right: 120,
    top: 10,
    bottom: 10
  },
  series: [
    {
      type: 'tree',
      data: treeData.value ? [treeData.value] : [],
      orient: 'LR',
      symbol: 'circle',
      symbolSize: 24,
      initialTreeDepth: -1,
      animationDuration: 300,
      animationDurationUpdate: 300,
      layout: 'orthogonal',
      leaves: {
        label: {
          position: 'right',
          verticalAlign: 'middle',
          align: 'left',
          distance: 8
        }
      },
      label: {
        position: 'inside',
        verticalAlign: 'middle',
        align: 'center',
        fontSize: 10,
        color: '#fff'
      },
      edgeShape: 'polyline',
      edgeForkPosition: '50%',
      roam: true,
      lineStyle: {
        color: '#555',
        width: 2
      },
      expandAndCollapse: false,
      nodeAlign: 'leaf'
    }
  ],
  tooltip: {
    trigger: 'item',
    backgroundColor: 'rgba(30, 30, 30, 0.95)',
    borderColor: '#555',
    textStyle: { color: '#fff', fontSize: 12 },
    formatter: (params: any) => {
      const hopNum = params.name?.replace('hop_', '')
      if (params.name === 'local') return '本机'
      const hop = result.value?.hops.find(h => h.hop_number.toString() === hopNum)
      if (!hop) return params.name
      const ip = hop.ip || 'Timeout'
      const latency = hop.avg_latency ? `${hop.avg_latency.toFixed(1)} ms` : '--'
      return `<div style="padding:4px;">
        <div><b>Hop ${hop.hop_number}</b></div>
        <div>IP: ${ip}</div>
        <div>延迟: ${latency}</div>
        <div>丢包: ${hop.packet_loss.toFixed(0)}%</div>
      </div>`
    }
  }
}))
</script>

<template>
  <div class="trace-graph">
    <!-- Loading State -->
    <div v-if="isRunning && hops.length === 0" class="loading-state">
      <div class="loading-spinner"></div>
      <p class="loading-text">正在初始化路由追踪...</p>
      <p class="loading-hint">正在解析目标地址并准备探测</p>
    </div>

    <!-- In Progress State -->
    <div v-else-if="isRunning && hops.length > 0" class="in-progress">
      <div class="progress-header">
        <div class="progress-info">
          <span class="status-badge running">追踪中</span>
          <span class="hop-progress">正在探测第 {{ currentHop }} 跳</span>
        </div>
        <div class="progress-dots">
          <span class="dot"></span>
          <span class="dot"></span>
          <span class="dot"></span>
        </div>
      </div>
    </div>

    <!-- Result Info -->
    <div v-if="result && result.hops.length > 0" class="trace-info">
      <div class="info-item">
        <span class="label">目标:</span>
        <span class="value">{{ result.target }}</span>
      </div>
      <div class="info-item">
        <span class="label">IP:</span>
        <span class="value">{{ result.target_ip }}</span>
      </div>
      <div class="info-item">
        <span class="label">跳数:</span>
        <span class="value">{{ result.hops.length }}</span>
      </div>
      <div v-if="result.completed" class="info-item">
        <span class="label status completed">已完成</span>
      </div>
    </div>

    <!-- Chart -->
    <v-chart
      v-if="treeData"
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; flex: 1; min-height: 300px;"
    />

    <!-- Empty State -->
    <div v-if="!isRunning && hops.length === 0" class="empty-state">
      <p>点击"开始追踪"进行路由追踪</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.trace-graph {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 12px;
  border: 1px solid var(--border-color);
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  gap: 12px;

  .loading-spinner {
    width: 40px;
    height: 40px;
    border: 3px solid var(--border-color);
    border-top-color: var(--accent-color);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  .loading-text {
    color: var(--text-primary);
    font-size: 14px;
    font-weight: 500;
    margin: 0;
  }

  .loading-hint {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0;
  }
}

.in-progress {
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border-color);

  .progress-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .progress-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status-badge {
    padding: 3px 8px;
    border-radius: 10px;
    font-size: 11px;
    font-weight: 500;

    &.running {
      background: rgba(33, 150, 243, 0.15);
      color: #64b5f6;
    }
  }

  .hop-progress {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .progress-dots {
    display: flex;
    gap: 4px;

    .dot {
      width: 6px;
      height: 6px;
      background: var(--accent-color);
      border-radius: 50%;
      animation: pulse 1.4s ease-in-out infinite;

      &:nth-child(2) { animation-delay: 0.2s; }
      &:nth-child(3) { animation-delay: 0.4s; }
    }
  }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes pulse {
  0%, 100% { opacity: 0.4; transform: scale(0.8); }
  50% { opacity: 1; transform: scale(1); }
}

.trace-info {
  display: flex;
  gap: 16px;
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border-color);
  flex-wrap: wrap;

  .info-item {
    display: flex;
    gap: 4px;

    .label {
      color: var(--text-muted);
      font-size: 11px;

      &.status.completed {
        background: rgba(76, 175, 80, 0.15);
        color: #81c784;
        padding: 2px 6px;
        border-radius: 6px;
      }
    }

    .value {
      color: var(--text-primary);
      font-size: 11px;
      font-weight: 500;
    }
  }
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
}
</style>