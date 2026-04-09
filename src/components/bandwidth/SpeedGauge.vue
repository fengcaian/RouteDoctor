<script setup lang="ts">
import { computed, ref } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { GaugeChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { BandwidthResult, BandwidthProgress } from '@/types'
import { useBandwidthStore } from '@/stores'

use([GaugeChart, TooltipComponent, CanvasRenderer])

const props = defineProps<{
  type: 'download' | 'upload'
}>()

const bandwidthStore = useBandwidthStore()

const speed = computed(() => {
  if (bandwidthStore.isRunning) {
    return bandwidthStore.progress.current_speed_mbps
  }
  if (bandwidthStore.lastResult) {
    return props.type === 'download'
      ? bandwidthStore.lastResult.download_speed_mbps
      : bandwidthStore.lastResult.upload_speed_mbps
  }
  return 0
})

const chartOption = computed(() => ({
  series: [
    {
      type: 'gauge',
      axisLine: {
        lineStyle: {
          width: 8,
          color: [
            [0.3, '#67e0e3'],
            [0.7, '#37a2da'],
            [1, '#fd666d']
          ]
        }
      },
      pointer: {
        itemStyle: {
          color: 'auto'
        }
      },
      axisTick: {
        distance: -10,
        length: 3,
        lineStyle: {
          color: '#fff',
          width: 1
        }
      },
      splitLine: {
        distance: -30,
        length: 8,
        lineStyle: {
          color: '#fff',
          width: 2
        }
      },
      axisLabel: {
        color: 'inherit',
        distance: 30,
        fontSize: 9
      },
      detail: {
        valueAnimation: true,
        formatter: '{value} Mbps',
        color: 'inherit',
        fontSize: 11
      },
      title: {
        show: false
      },
      radius: '85%',
      startAngle: 210,
      endAngle: -30,
      data: [
        {
          value: Math.round(speed.value * 10) / 10
        }
      ]
    }
  ]
}))
</script>

<template>
  <div class="speed-gauge">
    <v-chart
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; height: 100%"
    />
  </div>
</template>

<style lang="scss" scoped>
.speed-gauge {
  width: 180px;
  height: 160px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 8px;
  border: 1px solid var(--border-color);
}
</style>
