<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { GaugeChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { useBandwidthStore } from '@/stores'

use([GaugeChart, TooltipComponent, CanvasRenderer])

const props = defineProps<{
  type: 'download' | 'upload'
}>()

const bandwidthStore = useBandwidthStore()

const speed = computed(() => {
  let val = 0

  if (bandwidthStore.isRunning) {
    const currentPhase = bandwidthStore.progress.phase

    if (props.type === currentPhase) {
      // 当前正在测试的阶段：显示实时速度
      val = bandwidthStore.progress.current_speed_mbps
    } else if (props.type === 'download' && (currentPhase === 'upload' || currentPhase === 'idle')) {
      // download 已完成，显示 download 阶段记录的速度
      val = bandwidthStore.downloadPhaseSpeed
    }
  } else if (bandwidthStore.lastResult) {
    // 测试结束，显示最终结果
    val = props.type === 'download'
      ? bandwidthStore.lastResult.download_speed_mbps
      : bandwidthStore.lastResult.upload_speed_mbps
  }

  // 防止 NaN / undefined
  return Number.isFinite(val) ? val : 0
})

const chartOption = computed(() => {
  const currentSpeed = Math.round(speed.value * 10) / 10

  return {
    series: [
      {
        name: 'Pressure',
        type: 'gauge',
        detail: {
          formatter: '{value} Mbps',
          fontSize: 14
        },
        data: [
          {
            value: currentSpeed
          }
        ]
      }
    ]
  }
})
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
  flex: 1;
  min-width: 200px;
  min-height: 0;
  max-width: 600px;
  max-height: 600px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 16px;
  border: 1px solid var(--border-color);
}
</style>
