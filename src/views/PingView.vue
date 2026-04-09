<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import PingChart from '@/components/ping/PingChart.vue'
import PingStats from '@/components/ping/PingStats.vue'
import PingConfig from '@/components/ping/PingConfig.vue'
import PingTable from '@/components/ping/PingTable.vue'
import { usePingStore, useSettingsStore } from '@/stores'
import { usePing, usePingListener, stopAllPings } from '@/composables'
import type { TargetConfig, PingResult } from '@/types'

const router = useRouter()
const pingStore = usePingStore()
const settingsStore = useSettingsStore()

// Default target
const defaultTarget = '8.8.8.8'

const target = ref(defaultTarget)
const { startPing, stopPing, error } = usePing()

// Add default target if not already present
onMounted(() => {
  if (!pingStore.getConfig(defaultTarget)) {
    pingStore.addTarget({
      target: defaultTarget,
      interval_ms: settingsStore.settings.defaultPingInterval,
      timeout_ms: settingsStore.settings.defaultPingTimeout,
      count: null,
      packet_size: 64
    })
  }
})

// Cleanup on unmount (page refresh or navigation)
onUnmounted(async () => {
  // Stop all active ping sessions
  await stopAllPings()
  // Reset store state
  pingStore.resetStore()
  console.log('PingView unmounted - all ping sessions stopped')
})

// Listen to ping results
usePingListener(
  (result: PingResult) => {
    pingStore.addResult(result)
  }
)

async function handleStart(config: TargetConfig) {
  target.value = config.target
  if (!pingStore.getConfig(config.target)) {
    pingStore.addTarget(config)
  }
  pingStore.setRunning(config.target, true)
  try {
    await startPing(config)
  } catch (e) {
    // startPing failed, revert running state
    pingStore.setRunning(config.target, false)
    console.error('Failed to start ping:', e)
  }
}

function handleStop(t: string) {
  pingStore.setRunning(t, false)
  stopPing(t)
}

function handleClear(t: string) {
  pingStore.clearResults(t)
}
</script>

<template>
  <div class="ping-view">
    <div class="view-header">
      <h2>Network Ping Monitor</h2>
      <p class="subtitle">Real-time latency and packet loss monitoring</p>
    </div>

    <div class="ping-content">
      <div class="ping-main">
        <PingConfig
          :target="target"
          :is-running="pingStore.isRunning(target)"
          @start="handleStart"
          @stop="handleStop"
          @clear="handleClear"
        />

        <PingChart :target="target" />

        <PingStats :target="target" />

        <PingTable :target="target" />
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.view-header {
  margin-bottom: 8px;

  h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }

  .subtitle {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
}

.ping-content {
  display: flex;
  gap: 12px;
}

.ping-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
</style>