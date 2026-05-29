<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import TraceLatencyChart from '@/components/traceroute/TraceLatencyChart.vue'
import { useContinuousTrace, useContinuousTraceListener } from '@/composables/useContinuousTrace'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'
import { useToast } from '@/composables/useToast'
import type { ContinuousTraceHopResult, PathDiscovered } from '@/composables/useContinuousTrace'

const { t } = useI18n()
const store = useContinuousTraceStore()
const toast = useToast()
const { startContinuousTrace, stopContinuousTrace } = useContinuousTrace()

const targetInput = ref('google.com')
const pingInterval = ref(2000)
const maxHops = ref(30)
const timeoutMs = ref(3000)
const probeMethod = ref<'icmp' | 'udp' | 'tcp'>('icmp')

// 选中显示在折线图上的跳号集合
const selectedHopNumbers = ref<number[]>([])

// 路径发现完成后默认选中最后一跳（最终目标）
watch(
  () => store.hops.length,
  (len) => {
    if (!len) {
      selectedHopNumbers.value = []
      return
    }
    // 仅当当前选中失效时才重置为最后一跳
    const validNumbers = store.hops.filter(h => h.ip).map(h => h.hop_number)
    const stillValid = selectedHopNumbers.value.filter(n => validNumbers.includes(n))
    if (stillValid.length === 0) {
      const lastValid = [...store.hops].reverse().find(h => h.ip)
      if (lastValid) {
        selectedHopNumbers.value = [lastValid.hop_number]
      }
    } else if (stillValid.length !== selectedHopNumbers.value.length) {
      selectedHopNumbers.value = stillValid
    }
  }
)

/**
 * 切换跳的显示
 * - 单击：替换为该跳
 * - Ctrl/Cmd+点击：追加/移除
 * 没有 IP 的跳无法 Ping 监控，不允许选中
 */
function toggleHopSelection(hopNumber: number, hasIp: boolean, event: MouseEvent) {
  if (!hasIp) return
  const isMulti = event.ctrlKey || event.metaKey
  const idx = selectedHopNumbers.value.indexOf(hopNumber)
  if (isMulti) {
    if (idx >= 0) {
      if (selectedHopNumbers.value.length > 1) {
        selectedHopNumbers.value = selectedHopNumbers.value.filter(n => n !== hopNumber)
      }
    } else {
      selectedHopNumbers.value = [...selectedHopNumbers.value, hopNumber]
    }
  } else {
    selectedHopNumbers.value = [hopNumber]
  }
}

const HOP_COLORS = [
  '#4CAF50', '#2196F3', '#FF9800', '#E91E63', '#9C27B0',
  '#00BCD4', '#FFC107', '#F44336', '#8BC34A', '#3F51B5'
]
function colorForHop(hopNumber: number): string {
  return HOP_COLORS[hopNumber % HOP_COLORS.length]
}

// 监听事件
useContinuousTraceListener(
  (data: PathDiscovered) => {
    store.setPath(data)
    toast.success(`路径发现完成：${data.hops.length} 跳，开始持续监控`)
  },
  (result: ContinuousTraceHopResult) => {
    store.addHopResult(result)
  },
  (errorMsg: string) => {
    toast.error(errorMsg)
    store.stopMonitoring()
  },
  (_target: string) => {
    store.stopMonitoring()
  }
)

async function handleStart() {
  if (!targetInput.value.trim()) return
  store.startMonitoring(targetInput.value.trim())
  try {
    await startContinuousTrace(targetInput.value.trim(), maxHops.value, timeoutMs.value, pingInterval.value, probeMethod.value)
  } catch (e: any) {
    toast.error(`启动失败: ${typeof e === 'string' ? e : e.message || '未知错误'}`)
    store.stopMonitoring()
  }
}

async function handleStop() {
  try {
    await stopContinuousTrace(store.target)
  } catch (e) {
    // ignore
  }
  store.stopMonitoring()
}

function handleClear() {
  store.resetStore()
}

// 每跳统计：显示所有跳（包括无响应的），保持序号连续
const hopStats = computed(() => {
  if (store.hops.length === 0) return []

  // 找到最大跳数
  const maxHop = Math.max(...store.hops.map(h => h.hop_number))

  // 生成连续的跳列表
  const result = []
  for (let i = 1; i <= maxHop; i++) {
    const hop = store.hops.find(h => h.hop_number === i)
    if (hop && hop.ip) {
      result.push({ ...hop, stats: store.getHopStats(i) })
    } else {
      // 无响应的跳
      result.push({
        hop_number: i,
        ip: null,
        hostname: null,
        stats: { avg: 0, min: 0, max: 0, loss: 100, count: 0 }
      })
    }
  }
  return result
})

function getLatencyClass(avg: number): string {
  if (avg === 0) return ''
  if (avg < 50) return 'good'
  if (avg < 100) return 'medium'
  return 'bad'
}
</script>

<template>
  <div class="traceroute-view">
    <div class="view-header">
      <div>
        <h2>{{ $t('traceroute.title') }}</h2>
        <p class="subtitle">{{ $t('traceroute.subtitle') }}</p>
      </div>
    </div>

    <!-- 配置区 -->
    <div class="config-section">
      <div class="config-row">
        <div class="config-field target-field">
          <label class="config-label">{{ t('common.target') }}</label>
          <input
            v-model="targetInput"
            type="text"
            class="config-input"
            :placeholder="t('common.targetPlaceholder')"
            :disabled="store.isRunning"
            @keyup.enter="handleStart"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('continuousTrace.interval') }}</label>
          <input
            v-model.number="pingInterval"
            type="number"
            class="config-input"
            min="1000" max="10000" step="500"
            :disabled="store.isRunning"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.maxHops') }}</label>
          <input
            v-model.number="maxHops"
            type="number"
            class="config-input"
            min="5" max="64"
            :disabled="store.isRunning"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.timeoutMs') }}</label>
          <input
            v-model.number="timeoutMs"
            type="number"
            class="config-input"
            min="1000" max="10000" step="500"
            :disabled="store.isRunning"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.probeMethod') }}</label>
          <div class="probe-selector">
            <button
              v-for="method in ['icmp', 'udp', 'tcp'] as const"
              :key="method"
              :class="['probe-btn', { active: probeMethod === method }]"
              :disabled="store.isRunning"
              @click="probeMethod = method"
            >
              {{ method.toUpperCase() }}
            </button>
          </div>
        </div>
      </div>
      <div class="config-actions">
        <button
          v-if="!store.isRunning"
          class="start-btn"
          @click="handleStart"
          :disabled="!targetInput.trim()"
        >
          {{ t('continuousTrace.start') }}
        </button>
        <button v-else class="stop-btn" @click="handleStop">
          {{ t('continuousTrace.stop') }}
        </button>
        <button
          class="clear-btn"
          @click="handleClear"
          :disabled="store.isRunning"
        >
          {{ t('traceroute.clearData') }}
        </button>
      </div>
    </div>

    <!-- 发现中 -->
    <div v-if="store.isDiscovering" class="discovering-state">
      <div class="discovering-spinner"></div>
      <span>{{ t('continuousTrace.discovering') }}</span>
    </div>

    <!-- 实时延迟折线图（传送带模式） -->
    <div v-if="store.hops.length > 0" class="chart-section">
      <h3 class="section-title">{{ t('traceLatency.title') }}</h3>
      <div class="chart-container">
        <TraceLatencyChart :selected-hops="selectedHopNumbers" />
      </div>
    </div>

    <!-- 每跳统计 -->
    <div v-if="hopStats.length > 0" class="stats-section">
      <div class="stats-header">
        <h3 class="section-title">{{ t('continuousTrace.statsTitle') }}</h3>
        <span class="stats-tip">{{ t('traceLatency.tableTip') }}</span>
      </div>
      <table class="stats-table">
        <thead>
          <tr>
            <th>#</th>
            <th>IP</th>
            <th>{{ t('ping.avg') }}</th>
            <th>{{ t('ping.min') }}</th>
            <th>{{ t('ping.max') }}</th>
            <th>{{ t('traceroute.lossRate') }}</th>
            <th>{{ t('continuousTrace.samples') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="hop in hopStats"
            :key="hop.hop_number"
            :class="{
              'no-response': !hop.ip,
              'selectable': !!hop.ip,
              'selected': !!hop.ip && selectedHopNumbers.includes(hop.hop_number)
            }"
            @click="toggleHopSelection(hop.hop_number, !!hop.ip, $event)"
          >
            <td>
              <span
                v-if="hop.ip && selectedHopNumbers.includes(hop.hop_number)"
                class="hop-color-dot"
                :style="{ background: colorForHop(hop.hop_number) }"
              ></span>
              {{ hop.hop_number }}
            </td>
            <td class="mono">{{ hop.ip || '* * *' }}</td>
            <td :class="getLatencyClass(hop.stats.avg)">{{ hop.stats.avg > 0 ? `${hop.stats.avg.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.stats.min > 0 ? `${hop.stats.min.toFixed(1)} ms` : '--' }}</td>
            <td>{{ hop.stats.max > 0 ? `${hop.stats.max.toFixed(1)} ms` : '--' }}</td>
            <td :class="{ 'loss-high': hop.ip && hop.stats.loss > 5 }">{{ hop.ip ? `${hop.stats.loss.toFixed(1)}%` : '--' }}</td>
            <td class="muted">{{ hop.stats.count || '--' }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 空状态 -->
    <div v-if="!store.isRunning && store.hops.length === 0 && !store.isDiscovering" class="empty-state">
      <div class="empty-icon">🗺️</div>
      <p>{{ t('continuousTrace.emptyHint') }}</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.traceroute-view {
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

.config-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.config-row {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 100px;

  &.target-field { flex: 1; min-width: 200px; }
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

  &:focus { outline: none; border-color: var(--accent-color); }
  &:disabled { opacity: 0.6; }
}

.config-actions {
  display: flex;
  gap: 8px;
}

.probe-selector {
  display: flex;
  gap: 4px;
}

.probe-btn {
  padding: 6px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--input-bg);
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) {
    border-color: var(--accent-color);
    color: var(--text-primary);
  }

  &.active {
    background: var(--accent-color);
    border-color: var(--accent-color);
    color: white;
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

.start-btn {
  padding: 8px 20px;
  background: var(--accent-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) { background: var(--accent-color-hover); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
}

.stop-btn {
  padding: 8px 20px;
  background: var(--error-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;

  &:hover { background: var(--error-color-hover); }
}

.clear-btn {
  padding: 8px 16px;
  background: var(--button-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) { background: var(--hover-bg); }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
}

.discovering-state {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  color: var(--text-secondary);
  font-size: 13px;

  .discovering-spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border-color);
    border-top-color: var(--accent-color);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }
}

@keyframes spin { to { transform: rotate(360deg); } }

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 12px 0;
}

.chart-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.chart-container {
  width: 100%;
  height: 320px;
  min-width: 0;
}

.stats-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.stats-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: wrap;

  .section-title { margin: 0; }
}

.stats-tip {
  font-size: 11px;
  color: var(--text-muted);
}

.stats-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 12px;

  th {
    padding: 8px 10px;
    text-align: left;
    color: var(--text-muted);
    font-weight: 600;
    border-bottom: 1px solid var(--border-color);
  }

  td {
    padding: 6px 10px;
    color: var(--text-primary);
    border-bottom: 1px solid var(--border-color);

    &.mono { font-family: monospace; font-size: 11px; }
    &.muted { color: var(--text-muted); }
    &.good { color: #4CAF50; }
    &.medium { color: #FF9800; }
    &.bad { color: #f44336; }
    &.loss-high { color: #f44336; font-weight: 600; }
  }

  tr:hover td { background: var(--hover-bg); }

  tr.selectable {
    cursor: pointer;
    transition: background 0.15s;
  }

  tr.selected td {
    background: rgba(76, 175, 80, 0.12);
    font-weight: 500;
  }

  tr.selected:hover td {
    background: rgba(76, 175, 80, 0.18);
  }

  tr.no-response {
    opacity: 0.5;
    cursor: not-allowed;

    td { color: var(--text-muted); }
  }
}

.hop-color-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  min-height: 200px;
  color: var(--text-muted);
  gap: 8px;

  .empty-icon { font-size: 40px; opacity: 0.5; }
  p { font-size: 13px; margin: 0; }
}
</style>
