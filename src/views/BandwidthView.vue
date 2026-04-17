<script setup lang="ts">
import SpeedGauge from '@/components/bandwidth/SpeedGauge.vue'
import BandwidthConfig from '@/components/bandwidth/BandwidthConfig.vue'
import { useBandwidthStore } from '@/stores'
import { useBandwidth, useBandwidthListener } from '@/composables'
import type { BandwidthResult, BandwidthProgress } from '@/types'

const bandwidthStore = useBandwidthStore()
const { startBandwidthTest, stopBandwidthTest } = useBandwidth()

// Listen to bandwidth progress
useBandwidthListener(
  (progress: BandwidthProgress) => {
    bandwidthStore.updateProgress(progress)
  },
  (result: BandwidthResult) => {
    bandwidthStore.setResult(result)
    bandwidthStore.setRunning(false)
  }
)

function handleStart() {
  bandwidthStore.setRunning(true)
  startBandwidthTest()
}

function handleStop() {
  bandwidthStore.setRunning(false)
  stopBandwidthTest()
}

function handleClear() {
  bandwidthStore.clearHistory()
}
</script>

<template>
  <div class="bandwidth-view">
    <div class="view-header">
      <h2>Bandwidth Test</h2>
      <p class="subtitle">Measure your network download and upload speed</p>
    </div>

    <div class="bandwidth-content">
      <div class="gauges-section">
        <SpeedGauge type="download" />
        <SpeedGauge type="upload" />
      </div>

      <BandwidthConfig
        @start="handleStart"
        @stop="handleStop"
        @clear="handleClear"
      />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.bandwidth-view {
  display: flex;
  flex-direction: column;
  gap: 32px;
  padding: 24px;
  height: 100%;
}

.view-header {
  h2 {
    font-size: 24px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }

  .subtitle {
    font-size: 14px;
    color: var(--text-muted);
    margin-top: 6px;
  }
}

.bandwidth-content {
  display: flex;
  flex-direction: column;
  gap: 24px;
  align-items: center;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.gauges-section {
  display: flex;
  gap: 48px;
  justify-content: center;
  width: 100%;
  flex: 1;
  min-height: 0;
}
</style>