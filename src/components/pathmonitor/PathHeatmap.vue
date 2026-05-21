<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { HeatmapChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, VisualMapComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { usePathMonitorStore } from '@/stores/pathMonitorStore'

use([HeatmapChart, GridComponent, TooltipComponent, VisualMapComponent, DataZoomComponent, CanvasRenderer])

const { t } = useI18n()
const pathStore = usePathMonitorStore()
const chartRef = ref<InstanceType<typeof VChart> | null>(null)

// 可视窗口大小（显示最近多少个采样）
const VISIBLE_SAMPLES = 60

const chartOption = computed(() => {
  const session = pathStore.session
  if (!session || session.hops.length === 0) {
    return { series: [] }
  }

  const hops = session.hops
  const hopCount = hops.length

  // Y 轴：跳数标签
  const yLabels = hops.map(h => {
    if (h.ip) {
      return `${h.hopNumber}. ${h.ip}`
    }
    return `${h.hopNumber}. * * *`
  })

  // 收集所有时间点（取最近 VISIBLE_SAMPLES 个）
  const allTimestamps = new Set<number>()
  for (const hop of hops) {
    for (const sample of hop.samples) {
      allTimestamps.add(sample.timestamp)
    }
  }
  const sortedTimestamps = Array.from(allTimestamps).sort((a, b) => a - b)
  const visibleTimestamps = sortedTimestamps.slice(-VISIBLE_SAMPLES)

  // X 轴：时间标签
  const xLabels = visibleTimestamps.map(ts => {
    const d = new Date(ts)
    return `${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`
  })

  // 构建热力图数据 [x, y, value]
  // value: 延迟 ms，-1 表示超时
  const heatmapData: [number, number, number][] = []

  for (let hopIdx = 0; hopIdx < hopCount; hopIdx++) {
    const hop = hops[hopIdx]
    // 为每个可见时间点找到对应的采样
    for (let timeIdx = 0; timeIdx < visibleTimestamps.length; timeIdx++) {
      const ts = visibleTimestamps[timeIdx]
      const sample = hop.samples.find(s => s.timestamp === ts)
      if (sample) {
        heatmapData.push([timeIdx, hopIdx, sample.latency === null ? -1 : sample.latency])
      }
    }
  }

  return {
    backgroundColor: 'transparent',
    grid: {
      left: 120,
      right: 60,
      top: 10,
      bottom: 60
    },
    xAxis: {
      type: 'category',
      data: xLabels,
      axisLabel: {
        color: '#888',
        fontSize: 10,
        interval: Math.max(0, Math.floor(xLabels.length / 10) - 1),
        rotate: 30
      },
      axisLine: { lineStyle: { color: '#444' } },
      splitLine: { show: false }
    },
    yAxis: {
      type: 'category',
      data: yLabels,
      axisLabel: {
        color: '#888',
        fontSize: 10,
        width: 100,
        overflow: 'truncate'
      },
      axisLine: { lineStyle: { color: '#444' } },
      splitLine: { show: false }
    },
    visualMap: {
      min: 0,
      max: 200,
      calculable: true,
      orient: 'vertical',
      right: 0,
      top: 'center',
      itemHeight: 120,
      textStyle: { color: '#888', fontSize: 10 },
      inRange: {
        color: ['#4CAF50', '#8BC34A', '#CDDC39', '#FFEB3B', '#FF9800', '#f44336']
      },
      // -1 (超时) 用特殊颜色
      pieces: undefined
    },
    tooltip: {
      trigger: 'item',
      backgroundColor: 'rgba(30, 30, 30, 0.95)',
      borderColor: '#555',
      textStyle: { color: '#fff', fontSize: 12 },
      formatter: (params: any) => {
        const value = params.value[2]
        const hopLabel = yLabels[params.value[1]]
        const timeLabel = xLabels[params.value[0]]
        if (value === -1) {
          return `<b>${hopLabel}</b><br/>时间: ${timeLabel}<br/>状态: <span style="color:#f44336">超时</span>`
        }
        return `<b>${hopLabel}</b><br/>时间: ${timeLabel}<br/>延迟: ${value.toFixed(1)} ms`
      }
    },
    series: [{
      type: 'heatmap',
      data: heatmapData,
      emphasis: {
        itemStyle: {
          borderColor: '#fff',
          borderWidth: 1
        }
      },
      itemStyle: {
        borderRadius: 2
      }
    }]
  }
})
</script>

<template>
  <div class="path-heatmap">
    <v-chart
      v-if="pathStore.session && pathStore.session.hops.length > 0"
      ref="chartRef"
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; height: 100%"
    />
    <div v-else class="empty-heatmap">
      <p>{{ t('pathMonitor.heatmapEmpty') }}</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.path-heatmap {
  width: 100%;
  height: 100%;
  min-height: 300px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 12px;
  border: 1px solid var(--border-color);
  position: relative;
  min-width: 0;
  overflow: hidden;
}

.empty-heatmap {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-muted);
  font-size: 13px;
}
</style>
