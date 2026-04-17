<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { LineChart, BarChart } from 'echarts/charts'
import { GridComponent, TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { useBandwidthStore } from '@/stores'

use([LineChart, BarChart, GridComponent, TooltipComponent, CanvasRenderer])

const bandwidthStore = useBandwidthStore()

const history = computed(() => bandwidthStore.history.slice(-20))

const chartOption = computed(() => ({
  backgroundColor: 'transparent',
  grid: {
    left: 40,
    right: 15,
    top: 30,
    bottom: 30
  },
  xAxis: {
    type: 'category',
    data: history.value.map((_, i) => `Test ${i + 1}`),
    axisLine: {
      lineStyle: { color: 'var(--border-color)' }
    },
    axisLabel: {
      color: 'var(--text-muted)',
      fontSize: 10
    }
  },
  yAxis: {
    type: 'value',
    name: 'Mbps',
    nameTextStyle: {
      color: 'var(--text-muted)'
    },
    axisLine: {
      lineStyle: { color: 'var(--border-color)' }
    },
    axisLabel: {
      color: 'var(--text-muted)',
      fontSize: 10
    },
    splitLine: {
      lineStyle: { color: 'var(--border-color)', opacity: 0.3 }
    }
  },
  series: [
    {
      name: 'Download',
      type: 'bar',
      data: history.value.map(h => h.download_speed_mbps),
      itemStyle: {
        color: '#4CAF50',
        borderRadius: [4, 4, 0, 0]
      }
    },
    {
      name: 'Upload',
      type: 'bar',
      data: history.value.map(h => h.upload_speed_mbps),
      itemStyle: {
        color: '#2196F3',
        borderRadius: [4, 4, 0, 0]
      }
    }
  ],
  legend: {
    data: ['Download', 'Upload'],
    textStyle: { color: 'var(--text-muted)' },
    top: 0
  },
  tooltip: {
    trigger: 'axis',
    backgroundColor: 'rgba(30, 30, 30, 0.9)',
    borderColor: 'var(--border-color)',
    textStyle: { color: 'var(--text-primary)' },
    formatter: (params: any) => {
      const download = params[0]?.value || 0
      const upload = params[1]?.value || 0
      return `Download: ${download.toFixed(1)} Mbps<br/>Upload: ${upload.toFixed(1)} Mbps`
    }
  }
}))
</script>

<template>
  <div class="speed-chart">
    <v-chart
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; height: 100%"
    />
    <div v-if="history.length === 0" class="empty-state">
      <p>No speed test history</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.speed-chart {
  width: 100%;
  height: 220px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 12px;
  border: 1px solid var(--border-color);
  position: relative;
}

.empty-state {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 12px;
}
</style>