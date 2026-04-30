<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { TracerouteResult, HopResult } from '@/types'
import { useTracerouteStore } from '@/stores'

const { t } = useI18n()

const props = defineProps<{
  target: string
}>()

const traceStore = useTracerouteStore()

const result = computed<TracerouteResult | undefined>(() => traceStore.getResult(props.target))
const isRunning = computed(() => traceStore.isRunning(props.target))
const currentHop = computed(() => traceStore.getCurrentHop(props.target))

const hops = computed<HopResult[]>(() => result.value?.hops || [])

function formatLatency(latency: number | null): string {
  if (latency === null) return '--'
  return `${latency.toFixed(1)} ms`
}

function formatLatencies(latencies: (number | null)[]): string {
  return latencies.map(l => l === null ? '*' : `${l.toFixed(0)}ms`).join(' / ')
}

function getStatusClass(hop: HopResult): string {
  if (!hop.ip) return 'unknown'
  if (hop.avg_latency === null) return 'unknown'
  if (hop.avg_latency < 50) return 'good'
  if (hop.avg_latency < 100) return 'medium'
  return 'slow'
}
</script>

<template>
  <div class="trace-table">
    <!-- In Progress Indicator -->
    <div v-if="isRunning" class="progress-bar">
      <div class="progress-content">
        <span class="progress-icon">🔍</span>
        <span class="progress-text">{{ t('traceroute.routeTracing', { hop: currentHop }) }}</span>
      </div>
      <div class="progress-animation"></div>
    </div>

    <!-- Fixed Header Table -->
    <table v-if="hops.length > 0" class="hops-table header-table">
      <thead>
        <tr>
          <th>{{ t('traceroute.hop') }}</th>
          <th>IP</th>
          <th>{{ t('traceroute.hostname') }}</th>
          <th>{{ t('traceroute.latencies') }}</th>
          <th>{{ t('traceroute.avgLatency') }}</th>
          <th>{{ t('traceroute.lossRate') }}</th>
        </tr>
      </thead>
    </table>

    <!-- Scrollable Body -->
    <div v-if="hops.length > 0" class="table-body-wrapper">
      <table class="hops-table">
        <tbody>
          <tr v-for="hop in hops" :key="hop.hop_number" :class="getStatusClass(hop)">
            <td>{{ hop.hop_number }}</td>
            <td>{{ hop.ip || '* * *' }}</td>
            <td class="hostname">{{ hop.hostname || '--' }}</td>
            <td>{{ formatLatencies(hop.latencies) }}</td>
            <td :class="getStatusClass(hop)">{{ formatLatency(hop.avg_latency) }}</td>
            <td>{{ hop.packet_loss.toFixed(1) }}%</td>
          </tr>
          <!-- Loading row for next hop -->
          <tr v-if="isRunning" class="loading-row">
            <td colspan="6">
              <div class="loading-cell">
                <span class="loading-dot"></span>
                <span class="loading-dot"></span>
                <span class="loading-dot"></span>
                <span class="loading-text">{{ t('traceroute.waitingNext') }}</span>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Empty State -->
    <div v-if="!isRunning && hops.length === 0" class="empty-state">
      <div class="empty-icon">📡</div>
      <p>{{ t('traceroute.noData') }}</p>
      <p class="hint">{{ t('traceroute.noDataHint') }}</p>
    </div>

    <!-- Initial Loading State -->
    <div v-if="isRunning && hops.length === 0" class="initial-loading">
      <div class="loading-spinner"></div>
      <p>{{ t('traceroute.initializingShort') }}</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.trace-table {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: 12px;
  box-sizing: border-box;
}

.progress-bar {
  background: linear-gradient(90deg, rgba(33, 150, 243, 0.1), rgba(33, 150, 243, 0.2));
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  position: relative;
  overflow: hidden;

  .progress-content {
    display: flex;
    align-items: center;
    gap: 6px;
    position: relative;
    z-index: 1;
  }

  .progress-icon {
    font-size: 12px;
  }

  .progress-text {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .progress-animation {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    width: 100%;
    background: linear-gradient(90deg, transparent, rgba(33, 150, 243, 0.1), transparent);
    animation: shimmer 2s infinite;
  }
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

.table-wrapper {
  flex: 1;
  overflow-y: auto;

  &::-webkit-scrollbar {
    width: 10px;
  }

  &::-webkit-scrollbar-track {
    border-radius: 0 12px 12px 0;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 12px;
  }

  &::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }
}

.hops-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 13px;

  th, td {
    &:nth-child(1) { width: 60px; }
    &:nth-child(2) { width: 140px; }
    &:nth-child(3) { width: 200px; }
    &:nth-child(4) { width: 250px; }
    &:nth-child(5) { width: 80px; }
    &:nth-child(6) { width: 70px; }
  }
}

.header-table {
  background: var(--card-bg);
  border-bottom: 1px solid var(--border-color);

  thead {
    background: var(--table-header-bg);
    position: static;
  }
}

.table-body-wrapper {
  flex: 1;
  overflow-y: auto;

  &::-webkit-scrollbar {
    width: 10px;
  }

  &::-webkit-scrollbar-track {
    border-radius: 0 12px 12px 0;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 12px;
  }

  &::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }
}

th {
  padding: 10px 10px;
  text-align: left;
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1px solid var(--border-color);
  background: var(--table-header-bg);
  font-size: 12px;
}

td {
  padding: 8px 10px;
  color: var(--text-primary);
  border-bottom: 1px solid var(--border-color);
  font-size: 12px;

  &.hostname {
    font-size: 12px;
    color: var(--text-secondary);
  }

  &.good {
    color: var(--success-color);
  }

  &.medium {
    color: var(--warning-color);
  }

  &.slow {
    color: var(--error-color);
  }
}

tr {
  &:hover {
    background: var(--hover-bg);
  }

  &.unknown {
    opacity: 0.7;
  }
}

.loading-row {
  td {
    padding: 8px;
  }

  .loading-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;

    .loading-dot {
      width: 5px;
      height: 5px;
      background: var(--accent-color);
      border-radius: 50%;
      animation: bounce 1.4s ease-in-out infinite;

      &:nth-child(1) { animation-delay: 0s; }
      &:nth-child(2) { animation-delay: 0.2s; }
      &:nth-child(3) { animation-delay: 0.4s; }
    }

    .loading-text {
      color: var(--text-muted);
      font-size: 11px;
    }
  }
}

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.5; }
  40% { transform: scale(1); opacity: 1; }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  padding: 24px;
  color: var(--text-muted);

  .empty-icon {
    font-size: 28px;
    margin-bottom: 8px;
    opacity: 0.5;
  }

  p {
    margin: 0;

    &.hint {
      font-size: 11px;
      margin-top: 4px;
      opacity: 0.7;
    }
  }
}

.initial-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px;
  gap: 8px;

  .loading-spinner {
    width: 28px;
    height: 28px;
    border: 2px solid var(--border-color);
    border-top-color: var(--accent-color);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  p {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0;
  }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>