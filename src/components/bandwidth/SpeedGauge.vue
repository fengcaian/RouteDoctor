<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { GaugeChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { useBandwidthStore, useSettingsStore } from '@/stores'

use([GaugeChart, TooltipComponent, CanvasRenderer])

const props = defineProps<{
  type: 'download' | 'upload'
}>()

const bandwidthStore = useBandwidthStore()
const settingsStore = useSettingsStore()
const chartRef = ref<InstanceType<typeof VChart> | null>(null)
const { t } = useI18n()

// 根据主题决定指针和文字颜色
const isDark = computed(() => {
  const theme = settingsStore.settings.theme
  if (theme === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  return theme === 'dark'
})
const pointerColor = computed(() => isDark.value ? '#e0e0e0' : '#333333')
const textColor = computed(() => isDark.value ? '#e0e0e0' : '#333333')

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

// 动态量程：根据当前速度自动调整最大值
const maxSpeed = computed(() => {
  const current = speed.value
  if (current <= 0) return 100 // 默认 100 Mbps

  // 选择合适的量程档位
  const tiers = [10, 25, 50, 100, 200, 500, 1000, 2000, 5000, 10000]
  for (const tier of tiers) {
    if (current < tier * 0.85) return tier
  }
  // 超大速度：向上取整到最近的 1000
  return Math.ceil(current / 1000) * 1000
})

// 指针使用统一中性色，不与表盘颜色段混淆
// 下载/上传的区分通过底部标签的 CSS class 颜色体现

const chartOption = computed(() => {
  const currentSpeed = Math.round(speed.value * 10) / 10
  const max = maxSpeed.value

  // 根据速度占比计算颜色段

  return {
    backgroundColor: 'transparent',
    series: [
      {
        name: 'Speed',
        type: 'gauge',
        min: 0,
        max: max,
        splitNumber: 5,
        radius: '90%',
        center: ['50%', '55%'],
        startAngle: 210,
        endAngle: -30,
        // 进度条样式
        progress: {
          show: false
        },
        // 指针
        pointer: {
          length: '60%',
          width: 5,
          itemStyle: {
            color: pointerColor.value
          }
        },
        // 轴线
        axisLine: {
          lineStyle: {
            width: 14,
            color: [
              [0.3, '#f44336'],
              [0.7, '#FF9800'],
              [1, '#4CAF50']
            ]
          }
        },
        // 刻度
        axisTick: {
          distance: -18,
          length: 6,
          lineStyle: {
            color: '#999',
            width: 1
          }
        },
        // 分割线
        splitLine: {
          distance: -22,
          length: 12,
          lineStyle: {
            color: '#999',
            width: 2
          }
        },
        // 刻度标签
        axisLabel: {
          distance: 28,
          color: '#888',
          fontSize: 11,
          formatter: (value: number) => {
            if (max >= 1000) {
              return value >= 1000 ? `${(value / 1000).toFixed(0)}G` : `${value}`
            }
            return `${value}`
          }
        },
        // 中心数值
        detail: {
          valueAnimation: true,
          formatter: (value: number) => {
            if (value >= 1000) {
              return `${(value / 1000).toFixed(2)} Gbps`
            }
            return `${value.toFixed(1)} Mbps`
          },
          color: textColor.value,
          fontSize: 18,
          fontWeight: 700,
          offsetCenter: [0, '70%']
        },
        // 标题（阶段标签）
        title: {
          show: false
        },
        data: [
          {
            value: currentSpeed,
            name: props.type === 'download' ? t('bandwidth.download') : t('bandwidth.upload')
          }
        ],
        animation: true,
        animationDuration: 300,
        animationDurationUpdate: 300
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
    <div class="gauge-label" :class="props.type">
      {{ t(`bandwidth.${props.type}`) }}
    </div>
  </div>
</template>

<style lang="scss" scoped>
.speed-gauge {
  width: 100%;
  max-width: 480px;
  min-width: 280px;
  aspect-ratio: 1 / 0.85;
  min-height: 280px;
  max-height: 400px;
  background: var(--card-bg);
  border-radius: 12px;
  padding: 16px;
  border: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  align-items: center;
}

.gauge-label {
  font-size: 15px;
  font-weight: 600;
  margin-top: -24px;
  letter-spacing: 0.5px;

  &.download {
    color: #4CAF50;
  }

  &.upload {
    color: #2196F3;
  }
}
</style>
