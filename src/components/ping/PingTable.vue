<script setup lang="ts">
import { computed } from 'vue'
import type { PingResult } from '@/types'
import { usePingStore } from '@/stores'

const props = defineProps<{
  target: string
}>()

const pingStore = usePingStore()

const results = computed<PingResult[]>(() => {
  const all = pingStore.getResults(props.target)
  // Show last 50 results
  return all.slice(-50).reverse()
})

function formatLatency(result: PingResult): string {
  if (result.is_timeout) return 'Timeout'
  return `${result.latency_ms?.toFixed(1)} ms`
}

function getStatusClass(result: PingResult): string {
  if (result.is_timeout) return 'timeout'
  const latency = result.latency_ms
  if (latency === null) return 'timeout'
  if (latency < 50) return 'good'
  if (latency < 100) return 'medium'
  return 'slow'
}
</script>

<template>
  <div class="ping-table">
    <table class="results-table">
      <thead>
        <tr>
          <th>Seq</th>
          <th>IP</th>
          <th>Latency</th>
          <th>Time</th>
        </tr>
      </thead>
    </table>
    <div class="table-body-wrapper">
      <table class="results-table">
        <tbody>
          <tr v-for="result in results" :key="result.seq" :class="getStatusClass(result)">
            <td>{{ result.seq }}</td>
            <td>{{ result.ip }}</td>
            <td :class="getStatusClass(result)">{{ formatLatency(result) }}</td>
            <td>{{ new Date(result.timestamp).toLocaleTimeString() }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-table {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  max-height: 400px;
  display: flex;
  flex-direction: column;
}

.table-body-wrapper {
  overflow-y: auto;
  flex: 1;

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

.results-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 13px;

  thead {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--table-header-bg);

    th:first-child {
      border-top-left-radius: 12px;
    }

    th:last-child {
      border-top-right-radius: 12px;
    }
  }

  tbody {
    tr:last-child {
      td:first-child {
        border-bottom-left-radius: 12px;
      }

      td:last-child {
        border-bottom-right-radius: 12px;
      }
    }
  }
}

th {
  padding: 10px 12px;
  text-align: left;
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1px solid var(--border-color);
  font-size: 12px;
}

td {
  padding: 8px 12px;
  color: var(--text-primary);
  border-bottom: 1px solid var(--border-color);
  font-size: 12px;

  &.good {
    color: var(--success-color);
  }

  &.medium {
    color: var(--warning-color);
  }

  &.slow {
    color: var(--error-color);
  }

  &.timeout {
    color: var(--error-color);
    font-weight: 500;
  }
}

tr {
  &:hover {
    background: var(--hover-bg);
  }

  &.good td:first-child::before {
    content: '';
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success-color);
    margin-right: 8px;
  }

  &.timeout td:first-child::before {
    content: '';
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--error-color);
    margin-right: 8px;
  }
}
</style>