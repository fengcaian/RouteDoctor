<script setup lang="ts">
import { ref } from 'vue'
import TraceGraph from '@/components/traceroute/TraceGraph.vue'
import TraceTable from '@/components/traceroute/TraceTable.vue'
import TraceConfig from '@/components/traceroute/TraceConfig.vue'
import { useTracerouteStore } from '@/stores'
import { useTraceroute, useTracerouteListener } from '@/composables'
import type { HopResult, TracerouteResult, ProbeMethod } from '@/types'

const traceStore = useTracerouteStore()

const target = ref('google.com')
const { runTraceroute, stopTraceroute } = useTraceroute()

// Listen to traceroute results
useTracerouteListener(
  (hop: HopResult & { target: string }) => {
    traceStore.addHop(hop.target, hop)
  },
  (result: TracerouteResult) => {
    traceStore.completeTrace(result.target)
  }
)

async function handleStart(config: { target: string, maxHops: number, timeoutMs: number, probeMethod: ProbeMethod }) {
  target.value = config.target
  traceStore.startTrace(config.target, config.probeMethod)
  try {
    await runTraceroute(config.target, config.maxHops, config.timeoutMs, config.probeMethod)
  } catch (e) {
    console.error('Traceroute failed:', e)
    traceStore.completeTrace(config.target)
  }
}

async function handleStop(t: string) {
  traceStore.completeTrace(t)
  await stopTraceroute(t)
}

function handleClear(t: string) {
  traceStore.clearResult(t)
}
</script>

<template>
  <div class="traceroute-view">
    <div class="view-header">
      <h2>Traceroute</h2>
      <p class="subtitle">Network path discovery and hop analysis</p>
    </div>

    <div class="trace-content">
      <TraceConfig
        :target="target"
        :is-running="traceStore.isRunning(target)"
        @start="handleStart"
        @stop="handleStop"
        @clear="handleClear"
      />

      <div class="trace-results">
        <div class="trace-graph-section">
          <TraceGraph :target="target" />
        </div>
        <div class="trace-table-section">
          <TraceTable :target="target" />
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.traceroute-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.view-header {
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

.trace-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.trace-results {
  display: flex;
  gap: 16px;
  height: 380px;
}

.trace-graph-section {
  flex: 1;
  min-height: 0;
}

.trace-table-section {
  flex: 1;
  min-height: 0;
}
</style>