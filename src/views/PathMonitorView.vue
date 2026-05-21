<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import PathHeatmap from '@/components/pathmonitor/PathHeatmap.vue'
import { usePathMonitorStore } from '@/stores/pathMonitorStore'
import { useToast } from '@/composables/useToast'

const { t } = useI18n()
const pathStore = usePathMonitorStore()
const toast = useToast()

// 配置
const targetInput = ref('8.8.8.8')
const intervalMs = ref(1000)
const maxHops = ref(30)
const timeoutMs = ref(3000)

// 状态
const phase = ref<'idle' | 'discovering' | 'monitoring'>('idle')
const discoveredHops = ref<{ hopNumber: number, ip: string | null, hostname: string | null }[]>([])

// 定时器
let monitorTimer: ReturnType<typeof setInterval> | null = null
let traceUnlisten: (() => void) | null = null
let traceCompleteUnlisten: (() => void) | null = null

// 是否正在运行
const isRunning = computed(() => phase.value !== 'idle')

/**
 * 开始路径监控：先发现路径，再持续 Ping 每一跳
 */
async function startMonitor() {
  if (!targetInput.value.trim()) return

  const target = targetInput.value.trim()
  phase.value = 'discovering'
  discoveredHops.value = []

  try {
    // 第一步：监听 Traceroute 事件
    traceUnlisten = await listen<any>('trace-hop', (event) => {
      const hop = event.payload
      if (hop.target === target) {
        discoveredHops.value.push({
          hopNumber: hop.hop_number,
          ip: hop.ip || null,
          hostname: hop.hostname || null
        })
      }
    })

    traceCompleteUnlisten = await listen<any>('trace-complete', (event) => {
      if (event.payload.target === target) {
        onTraceComplete(target)
      }
    })

    // 第二步：启动 Traceroute 发现路径
    await invoke('run_traceroute', {
      target,
      maxHops: maxHops.value,
      timeoutMs: timeoutMs.value,
      probeMethod: 'icmp'
    })
  } catch (e: any) {
    phase.value = 'idle'
    toast.error(`路径发现失败: ${typeof e === 'string' ? e : e.message || '未知错误'}`)
    cleanup()
  }
}

/**
 * Traceroute 完成后，开始持续 Ping 每一跳
 */
function onTraceComplete(target: string) {
  // 清理 Traceroute 监听器
  traceUnlisten?.()
  traceCompleteUnlisten?.()
  traceUnlisten = null
  traceCompleteUnlisten = null

  // 过滤出有 IP 的跳（无 IP 的跳无法 Ping）
  const validHops = discoveredHops.value.filter(h => h.ip !== null)

  if (validHops.length === 0) {
    phase.value = 'idle'
    toast.error('未发现有效路径节点')
    return
  }

  // 初始化 store
  pathStore.initSession(
    target,
    validHops[validHops.length - 1]?.ip || target,
    discoveredHops.value,  // 包含所有跳（含超时的）
    intervalMs.value
  )

  phase.value = 'monitoring'
  toast.success(`路径发现完成，${validHops.length} 个节点开始持续监控`)

  // 开始定时 Ping 每一跳
  startPingLoop(validHops)
}

/**
 * 定时 Ping 循环
 */
function startPingLoop(hops: { hopNumber: number, ip: string | null }[]) {
  monitorTimer = setInterval(async () => {
    if (phase.value !== 'monitoring') return

    // 并行 Ping 所有有 IP 的跳
    const pingPromises = hops
      .filter(h => h.ip !== null)
      .map(async (hop) => {
        try {
          const result = await invoke<any>('ping_once', {
            target: hop.ip!,
            timeoutMs: timeoutMs.value,
            packetSize: 64
          })
          return {
            hopNumber: hop.hopNumber,
            latency: result.is_timeout ? null : result.latency_ms
          }
        } catch {
          return {
            hopNumber: hop.hopNumber,
            latency: null
          }
        }
      })

    // 等待所有 Ping 完成
    const results = await Promise.all(pingPromises)

    // 补充无 IP 跳的结果（标记为超时）
    const allResults = discoveredHops.value.map(h => {
      const found = results.find(r => r.hopNumber === h.hopNumber)
      return found || { hopNumber: h.hopNumber, latency: null }
    })

    // 更新 store
    pathStore.addSampleRound(allResults)
  }, intervalMs.value)
}

/**
 * 停止监控
 */
function stopMonitor() {
  phase.value = 'idle'
  pathStore.stopSession()
  cleanup()
  toast.info('路径监控已停止')
}

/**
 * 清理资源
 */
function cleanup() {
  if (monitorTimer) {
    clearInterval(monitorTimer)
    monitorTimer = null
  }
  traceUnlisten?.()
  traceCompleteUnlisten?.()
  traceUnlisten = null
  traceCompleteUnlisten = null
}

/**
 * 重置
 */
function handleClear() {
  stopMonitor()
  pathStore.clearSession()
}

onUnmounted(() => {
  cleanup()
})
</script>

<template>
  <div class="path-monitor-view">
    <div class="view-header">
      <div>
        <h2>{{ t('pathMonitor.title') }}</h2>
        <p class="subtitle">{{ t('pathMonitor.subtitle') }}</p>
      </div>
    </div>

    <!-- 配置区 -->
    <div class="monitor-config">
      <div class="config-row">
        <div class="config-field target-field">
          <label class="config-label">{{ t('common.target') }}</label>
          <input
            v-model="targetInput"
            type="text"
            class="config-input"
            :placeholder="t('common.targetPlaceholder')"
            :disabled="isRunning"
            @keyup.enter="startMonitor"
          />
        </div>
        <div class="config-field small">
          <label class="config-label">{{ t('pathMonitor.interval') }}</label>
          <input
            v-model.number="intervalMs"
            type="number"
            class="config-input"
            min="500" max="10000" step="500"
            :disabled="isRunning"
          />
        </div>
        <div class="config-field small">
          <label class="config-label">{{ t('traceroute.maxHops') }}</label>
          <input
            v-model.number="maxHops"
            type="number"
            class="config-input"
            min="5" max="64"
            :disabled="isRunning"
          />
        </div>
        <div class="config-field actions">
          <label class="config-label">&nbsp;</label>
          <div class="btn-group">
            <button
              v-if="!isRunning"
              class="action-btn start"
              @click="startMonitor"
              :disabled="!targetInput.trim()"
            >
              {{ t('pathMonitor.start') }}
            </button>
            <button
              v-else
              class="action-btn stop"
              @click="stopMonitor"
            >
              {{ phase === 'discovering' ? t('pathMonitor.discovering') : t('pathMonitor.stop') }}
            </button>
            <button
              class="action-btn clear"
              @click="handleClear"
              :disabled="isRunning"
            >
              {{ t('common.clear') }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 状态信息 -->
    <div v-if="pathStore.session" class="monitor-status">
      <div class="status-item">
        <span class="status-label">{{ t('pathMonitor.target') }}</span>
        <span class="status-value mono">{{ pathStore.session.target }}</span>
      </div>
      <div class="status-item">
        <span class="status-label">{{ t('pathMonitor.hops') }}</span>
        <span class="status-value">{{ pathStore.session.hops.length }}</span>
      </div>
      <div class="status-item">
        <span class="status-label">{{ t('pathMonitor.samples') }}</span>
        <span class="status-value">{{ pathStore.session.totalSamples }}</span>
      </div>
      <div class="status-item">
        <span class="status-badge" :class="pathStore.session.isRunning ? 'running' : 'stopped'">
          {{ pathStore.session.isRunning ? t('pathMonitor.running') : t('pathMonitor.stopped') }}
        </span>
      </div>
    </div>

    <!-- 热力图 -->
    <div class="heatmap-section">
      <PathHeatmap />
    </div>

    <!-- 跳数详情表格 -->
    <div v-if="pathStore.session && pathStore.session.hops.length > 0" class="hops-detail">
      <table class="hops-table">
        <thead>
          <tr>
            <th>#</th>
            <th>IP</th>
            <th>{{ t('pathMonitor.current') }}</th>
            <th>{{ t('ping.avg') }}</th>
            <th>{{ t('ping.min') }}</th>
            <th>{{ t('ping.max') }}</th>
            <th>{{ t('pathMonitor.loss') }}</th>
            <th>{{ t('pathMonitor.sent') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="hop in pathStore.session.hops" :key="hop.hopNumber">
            <td>{{ hop.hopNumber }}</td>
            <td class="mono">{{ hop.ip || '* * *' }}</td>
            <td :class="getLatencyClass(hop.currentLatency)">
              {{ hop.currentLatency !== null ? `${hop.currentLatency.toFixed(1)} ms` : '--' }}
            </td>
            <td>{{ hop.avgLatency !== null ? `${hop.avgLatency.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.minLatency !== null ? `${hop.minLatency.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.maxLatency !== null ? `${hop.maxLatency.toFixed(1)} ms` : '--' }}</td>
            <td :class="{ 'loss-high': hop.lossRate > 5 }">{{ hop.lossRate.toFixed(1) }}%</td>
            <td>{{ hop.totalSent }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script lang="ts">
export default {
  methods: {
    getLatencyClass(latency: number | null): string {
      if (latency === null) return 'timeout'
      if (latency < 50) return 'good'
      if (latency < 100) return 'medium'
      return 'slow'
    }
  }
}
</script>

<style lang="scss" scoped>
.path-monitor-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  min-width: 0;
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

.monitor-config {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 12px 16px;
}

.config-row {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  flex-wrap: wrap;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 4px;

  &.target-field { flex: 1; min-width: 200px; }
  &.small { width: 100px; }
  &.actions { width: auto; }
}

.config-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.config-input {
  padding: 8px 10px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
  color: var(--text-primary);
  font-size: 13px;

  &:focus {
    outline: none;
    border-color: var(--accent-color);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.btn-group {
  display: flex;
  gap: 8px;
}

.action-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;

  &.start {
    background: var(--accent-color);
    color: white;
    &:hover:not(:disabled) { background: var(--accent-color-hover); }
    &:disabled { opacity: 0.5; cursor: not-allowed; }
  }

  &.stop {
    background: var(--error-color);
    color: white;
    &:hover { background: var(--error-color-hover); }
  }

  &.clear {
    background: var(--border-color);
    color: var(--text-primary);
    &:hover:not(:disabled) { background: var(--text-muted); }
    &:disabled { opacity: 0.5; cursor: not-allowed; }
  }
}

.monitor-status {
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 10px 16px;
  background: var(--card-bg);
  border-radius: 10px;
  border: 1px solid var(--border-color);
  flex-wrap: wrap;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-label {
  font-size: 11px;
  color: var(--text-muted);
}

.status-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);

  &.mono { font-family: monospace; }
}

.status-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 3px 10px;
  border-radius: 10px;

  &.running {
    background: rgba(76, 175, 80, 0.15);
    color: #4CAF50;
  }
  &.stopped {
    background: var(--hover-bg);
    color: var(--text-muted);
  }
}

.heatmap-section {
  flex: 1;
  min-height: 300px;
}

.hops-detail {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  overflow: hidden;
  max-height: 300px;
  overflow-y: auto;
}

.hops-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;

  th {
    padding: 8px 10px;
    text-align: left;
    color: var(--text-muted);
    font-weight: 600;
    background: var(--table-header-bg);
    border-bottom: 1px solid var(--border-color);
    position: sticky;
    top: 0;
  }

  td {
    padding: 6px 10px;
    color: var(--text-primary);
    border-bottom: 1px solid var(--border-color);

    &.mono { font-family: monospace; font-size: 11px; }
    &.good { color: var(--success-color); }
    &.medium { color: var(--warning-color); }
    &.slow { color: var(--error-color); }
    &.timeout { color: var(--error-color); font-weight: 500; }
    &.loss-high { color: var(--error-color); font-weight: 600; }
  }

  tr:hover td {
    background: var(--hover-bg);
  }
}
</style>
