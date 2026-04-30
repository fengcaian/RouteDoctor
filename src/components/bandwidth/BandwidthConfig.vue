<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { BandwidthProgress } from '@/types'
import { useBandwidthStore } from '@/stores'

const { t } = useI18n()
const bandwidthStore = useBandwidthStore()

const progress = computed<BandwidthProgress>(() => bandwidthStore.progress)

const progressPercent = computed(() => {
  return Math.min(100, Math.max(0, progress.value.progress))
})

const phaseText = computed(() => {
  switch (progress.value.phase) {
    case 'download':
      return t('bandwidth.testingDownload')
    case 'upload':
      return t('bandwidth.testingUpload')
    default:
      return t('common.ready')
  }
})
</script>

<template>
  <div class="bandwidth-config">
    <div class="progress-section" v-if="bandwidthStore.isRunning">
      <div class="progress-info">
        <span class="phase-text">{{ phaseText }}</span>
        <span class="speed-text">{{ progress.current_speed_mbps.toFixed(1) }} Mbps</span>
      </div>
      <div class="progress-bar">
        <div
          class="progress-fill"
          :style="{ width: `${progressPercent}%` }"
          :class="progress.phase"
        ></div>
      </div>
      <div class="bytes-info">
        {{ (progress.bytes_transferred / 1024 / 1024).toFixed(1) }} MB {{ t('bandwidth.transferred') }}
      </div>
    </div>
    <div class="last-result" v-else-if="bandwidthStore.lastResult">
      <div class="result-item download">
        <span class="result-label">{{ t('bandwidth.download') }}</span>
        <span class="result-value">{{ bandwidthStore.lastResult.download_speed_mbps.toFixed(1) }} Mbps</span>
      </div>
      <div class="result-item upload">
        <span class="result-label">{{ t('bandwidth.upload') }}</span>
        <span class="result-value">{{ bandwidthStore.lastResult.upload_speed_mbps.toFixed(1) }} Mbps</span>
      </div>
      <div class="result-item latency">
        <span class="result-label">{{ t('bandwidth.latency') }}</span>
        <span class="result-value">{{ bandwidthStore.lastResult.latency_ms.toFixed(1) }} ms</span>
      </div>
    </div>
    <div class="actions">
      <button
        v-if="!bandwidthStore.isRunning"
        class="start-btn"
        @click="$emit('start')"
      >
        {{ t('bandwidth.startTest') }}
      </button>
      <button
        v-else
        class="stop-btn"
        @click="$emit('stop')"
      >
        {{ t('bandwidth.cancelTest') }}
      </button>
      <button
        class="clear-btn"
        @click="$emit('clear')"
        :disabled="bandwidthStore.isRunning"
      >
        {{ t('bandwidth.clearData') }}
      </button>
    </div>
  </div>
</template>

<script lang="ts">
export default {
  emits: ['start', 'stop', 'clear']
}
</script>

<style lang="scss" scoped>
.bandwidth-config {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px 20px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
}

.progress-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.phase-text {
  color: var(--text-secondary);
  font-size: 12px;
}

.speed-text {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 600;
}

.progress-bar {
  height: 6px;
  background: var(--border-color);
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 4px;
  transition: width 0.3s ease;

  &.download {
    background: #4CAF50;
  }

  &.upload {
    background: #2196F3;
  }
}

.bytes-info {
  color: var(--text-muted);
  font-size: 12px;
}

.last-result {
  display: flex;
  gap: 12px;
}

.result-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px;
  border-radius: 8px;
  background: var(--hover-bg);

  &.download {
    border-left: 3px solid #4CAF50;
  }

  &.upload {
    border-left: 3px solid #2196F3;
  }

  &.latency {
    border-left: 3px solid #FF9800;
  }
}

.result-label {
  color: var(--text-muted);
  font-size: 12px;
}

.result-value {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 600;
}

.actions {
  display: flex;
  justify-content: center;
}

.start-btn {
  padding: 10px 32px;
  background: var(--accent-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    background: var(--accent-color-hover);
    transform: translateY(-1px);
  }
}

.stop-btn {
  padding: 10px 32px;
  background: var(--error-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    background: var(--error-color-hover);
  }
}

.clear-btn {
  padding: 10px 32px;
  background: var(--border-color);
  border: none;
  border-radius: 8px;
  color: var(--text-primary);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover:not(:disabled) {
    background: var(--text-muted);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}
</style>