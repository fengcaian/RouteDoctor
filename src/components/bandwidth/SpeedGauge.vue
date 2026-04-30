<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
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
const chartRef = ref<InstanceType<typeof VChart> | null>(null)
const { t } = useI18n()

// 窗口大小变化时手动触发 echarts resize
let resizeTimer: ReturnType<typeof setTimeout> | null = null

function handleResize() {
  if (resizeTimer) clearTimeout(resizeTimer)
  resizeTimer = setTimeout(() => {
    chartRef.value?.resize()
  }, 150)
}

onMounted(() => {
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  if (resizeTimer) clearTimeout(resizeTimer)
})

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
      ref="chartRef"
      :option="chartOption"
      :autoresize="true"
      style="width: 100%; flex: 1; min-height: 0"
    />
    <div class="gauge-label" :class="props.type">{{ t(`bandwidth.${props.type}`) }}</div>
  </div>
</template>

<style lang="scss" scoped>
.speed-gauge {
  width: 100%;
  max-width: 600px;
  height: 420px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 16px;
  border: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  align-items: center;
}

.gauge-label {
  font-size: 16px;
  font-weight: 600;
  margin-top: -60px;

  &.download {
    color: #4CAF50;
  }

  &.upload {
    color: #2196F3;
  }
}
</style>
