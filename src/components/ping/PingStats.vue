<script setup lang="ts">
import { computed } from 'vue'
import type { PingStatistics } from '@/types'
import { usePingStore } from '@/stores'

const props = defineProps<{
  target: string
}>()

const pingStore = usePingStore()

const stats = computed<PingStatistics>(() => pingStore.getStatistics(props.target))

const formatValue = (value: number | null, unit: string = 'ms'): string => {
  if (value === null || value === 0) return '--'
  return `${value.toFixed(1)} ${unit}`
}

const lossColor = computed(() => {
  const loss = stats.value.loss_rate
  if (loss === 0) return 'var(--success-color)'
  if (loss < 5) return 'var(--warning-color)'
  return 'var(--error-color)'
})

const avgColor = computed(() => {
  const avg = stats.value.avg_ms
  if (avg === 0) return 'var(--text-muted)'
  if (avg < 50) return 'var(--success-color)'
  if (avg < 100) return 'var(--warning-color)'
  return 'var(--error-color)'
})
</script>

<template>
  <div class="ping-stats">
    <div class="stat-item">
      <span class="stat-label">Sent</span>
      <span class="stat-value">{{ stats.sent }}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Received</span>
      <span class="stat-value">{{ stats.received }}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Lost</span>
      <span class="stat-value" :style="{ color: lossColor }">{{ stats.lost }} ({{ stats.loss_rate.toFixed(1) }}%)</span>
    </div>
    <div class="stat-divider"></div>
    <div class="stat-item">
      <span class="stat-label">Min</span>
      <span class="stat-value">{{ formatValue(stats.min_ms) }}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Max</span>
      <span class="stat-value">{{ formatValue(stats.max_ms) }}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Avg</span>
      <span class="stat-value" :style="{ color: avgColor }">{{ formatValue(stats.avg_ms) }}</span>
    </div>
    <div class="stat-divider"></div>
    <div class="stat-item">
      <span class="stat-label">Jitter</span>
      <span class="stat-value">{{ formatValue(stats.jitter_ms) }}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Std Dev</span>
      <span class="stat-value">{{ formatValue(stats.std_dev_ms) }}</span>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
}

.stat-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 70px;
}

.stat-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.stat-value {
  font-size: 14px;
  color: var(--text-primary);
  font-weight: 600;
}

.stat-divider {
  width: 1px;
  height: 32px;
  background: var(--border-color);
}
</style>