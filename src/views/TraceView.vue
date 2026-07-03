<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import TraceLatencyChart from '@/components/traceroute/TraceLatencyChart.vue'
import TraceSessionHistory from '@/components/traceroute/TraceSessionHistory.vue'
import NpcapGuideDialog from '@/components/common/NpcapGuideDialog.vue'
import { useContinuousTrace, useContinuousTraceListener } from '@/composables/useContinuousTrace'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'
import { useSettingsStore } from '@/stores/settingsStore'
import { useNpcapStore } from '@/stores'
import { useToast } from '@/composables/useToast'
import type { ContinuousTraceHopResult, PathDiscovered, PathUpdate } from '@/composables/useContinuousTrace'

const { t } = useI18n()
const store = useContinuousTraceStore()
const settingsStore = useSettingsStore()
const npcapStore = useNpcapStore()
const toast = useToast()
const { startContinuousTrace, stopContinuousTrace } = useContinuousTrace()

const targetInput = ref('google.com')
const pingInterval = ref(2000)
const maxHops = ref(30)
const timeoutMs = ref(3000)
const probeMethod = ref<'icmp' | 'udp' | 'tcp'>('icmp')
// TCP 探测的目标端口，默认 80（与 PingPlotter 一致），仅 probeMethod='tcp' 时生效
const tcpPort = ref(80)

// 历史会话抽屉
const historyOpen = ref(false)

// 输入区禁用条件：实时监控中 OR 正在查看历史会话
const inputsDisabled = computed(() => store.isRunning || store.isHistoricalView)

// 选中显示在折线图上的跳号集合
const selectedHopNumbers = ref<number[]>([])

// 展开显示"每跳 IP 分布"的跳号集合（PingPlotter Pro 风格的展开子行）
// 只对观察到 ≥2 个 IP 的跳才允许展开
const expandedHopNumbers = ref<Set<number>>(new Set())

/** 切换某跳的展开/折叠。仅在多 IP 时有效。 */
function toggleHopExpand(hopNumber: number, hasMultipleIps: boolean, event: MouseEvent) {
  if (!hasMultipleIps) return
  event.stopPropagation() // 避免触发 toggleHopSelection
  const s = expandedHopNumbers.value
  if (s.has(hopNumber)) {
    s.delete(hopNumber)
  } else {
    s.add(hopNumber)
  }
  // 触发响应式
  expandedHopNumbers.value = new Set(s)
}

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

// 加载历史会话后,把 probeMethod 显示同步成会话使用的方式(ICMP/UDP/TCP)
// 否则永远显示初始值 'icmp',与会话实际探测方式不符
watch(
  () => store.loadedProbeMethod,
  (val) => {
    if (val === 'icmp' || val === 'udp' || val === 'tcp') {
      probeMethod.value = val
    }
  }
)

// ============================================================================
// Npcap 集成：UDP/TCP 探测方式按钮的状态提示与首次引导
// ============================================================================

// 引导对话框相关状态
const npcapDialogVisible = ref(false)
const npcapDialogProtocol = ref<'UDP' | 'TCP'>('UDP')

/**
 * 给指定协议返回 tooltip 文本
 * - icmp：无提示（ICMP 不需要 Npcap）
 * - udp/tcp：根据 Npcap 安装状态返回不同文案
 */
function probeMethodTooltip(method: 'icmp' | 'udp' | 'tcp'): string {
  if (method === 'icmp') return ''
  // 非 Windows 平台不显示 Npcap 提示
  if (!npcapStore.status.supported_platform) return ''
  const protocolUpper = method.toUpperCase()
  if (npcapStore.status.installed) {
    return t('traceroute.npcapTip.enhanced', { protocol: protocolUpper })
  }
  return t('traceroute.npcapTip.basic', { protocol: protocolUpper })
}

/**
 * 给按钮加视觉状态指示
 * - Npcap 已装：UDP/TCP 按钮显示绿色小点（增强模式）
 * - Npcap 未装：UDP/TCP 按钮显示灰色提示点（基础模式，可点击下载）
 */
function probeMethodBadgeClass(method: 'icmp' | 'udp' | 'tcp'): string {
  if (method === 'icmp') return ''
  if (!npcapStore.status.supported_platform) return ''
  return npcapStore.status.installed ? 'badge-enhanced' : 'badge-basic'
}

/**
 * 切换探测方式时的钩子。
 * UDP/TCP + Npcap 未装 + 用户从未看过引导 → 弹引导对话框（不强制）。
 * 引导对话框不会阻止切换：用户切到 UDP/TCP 后照样能用（走 ICMP 兜底）。
 */
function handleProbeMethodChange(method: 'icmp' | 'udp' | 'tcp') {
  probeMethod.value = method
  if (method === 'icmp') return
  if (!npcapStore.status.supported_platform) return
  if (npcapStore.status.installed) return
  if (npcapStore.guideShown) return
  // 首次切到 UDP/TCP 且未装 Npcap → 弹一次引导
  npcapDialogProtocol.value = method.toUpperCase() as 'UDP' | 'TCP'
  npcapDialogVisible.value = true
}

/**
 * 用户点击"了解详情"链接时主动打开对话框（不受 guideShown 限制）
 */
function openNpcapGuide(method: 'udp' | 'tcp') {
  npcapDialogProtocol.value = method.toUpperCase() as 'UDP' | 'TCP'
  npcapDialogVisible.value = true
}

function closeNpcapDialog() {
  npcapDialogVisible.value = false
}

function acknowledgeNpcapDialog() {
  npcapStore.markGuideShown()
}

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
    console.log('[TraceView] path-discovered received', data.hops.length, 'hops, target=', data.target)
    store.setPath(data)
    // 显示"实际路径长度"（最后响应跳的编号），而不是原始探测跳数 max_hops
    // UDP/TCP 追踪目标不响应时 data.hops 长度可能是 30，但真实路径要短得多；
    // 用 max(hop_number) over 有 IP 的跳，能得到真实的路径长度（含中间超时跳）
    const responded = data.hops.filter(h => h.ip)
    const actualHopCount = responded.length > 0
      ? Math.max(...responded.map(h => h.hop_number))
      : data.hops.length
    toast.success(`路径发现完成：${actualHopCount} 跳，开始持续监控`)
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
  },
  undefined,
  undefined,
  // path-update：后端每轮 emit 完整路径快照，前端做增量 merge
  // 用来补上初始超时后来响应的跳（mtr / PingPlotter 招牌行为）
  (data: PathUpdate) => {
    store.mergePath(data)
  }
)

async function handleStart() {
  if (!targetInput.value.trim()) return

  // 根据设置项 traceWindowMinutes 和 pingInterval 计算滑动窗口大小
  // 例如 60 分钟 / 2 秒 = 1800 个样本
  const windowMinutes = settingsStore.settings.traceWindowMinutes || 60
  const samplesPerWindow = Math.ceil((windowMinutes * 60 * 1000) / pingInterval.value)
  store.setMaxSamples(samplesPerWindow)

  store.startMonitoring(targetInput.value.trim(), probeMethod.value)
  try {
    await startContinuousTrace(
      targetInput.value.trim(),
      maxHops.value,
      timeoutMs.value,
      pingInterval.value,
      probeMethod.value,
      settingsStore.settings.tracePersistEnabled,
      // 仅 TCP 模式下传端口；其他模式后端会忽略
      probeMethod.value === 'tcp' ? tcpPort.value : undefined
    )
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
// 每行携带 ipBreakdown（多 IP 分组统计）供 UI 展开子行使用
// 主行显示的 IP 优先取"样本最多的 IP"（PingPlotter 一致），首轮刚发现还没样本时
// 回退到 hop.ip（最新观察到的 IP）
//
// 尾部空跳截断（PingPlotter 一致）：
// UDP/TCP 追踪时目标常不响应 ICMP Port Unreachable / TCP SYN-ACK，导致
// fast_udp/tcp 结果里从"实际最后一跳"到 max_hops 都是空的 * * *。
// 找到最后一个曾经观察到 IP 的跳作为路径终点，之后的空跳不显示。
// 中间空跳（防火墙屏蔽某个中间路由器）仍保留，方便诊断。
const hopStats = computed(() => {
  if (store.hops.length === 0) return []

  // 找到最后一个曾观察到 IP 的跳（作为路径显示终点）
  let lastResponsiveHop = 0
  for (const h of store.hops) {
    if (h.ip && h.hop_number > lastResponsiveHop) {
      lastResponsiveHop = h.hop_number
    }
  }

  // 一个响应都没有：显示全部（让用户看到"完全无响应"）
  const maxHop = lastResponsiveHop > 0
    ? lastResponsiveHop
    : Math.max(...store.hops.map(h => h.hop_number))

  // 生成连续的跳列表
  const result = []
  for (let i = 1; i <= maxHop; i++) {
    const hop = store.hops.find(h => h.hop_number === i)
    if (hop && hop.ip) {
      const ipBreakdown = store.getHopIpBreakdown(i)
      // ipBreakdown 已按 count 降序排列（see store.getHopIpBreakdown）
      // 首轮刚发现路径还没样本时 ipBreakdown 为空，回退到 hop.ip
      const displayIp = ipBreakdown.length > 0 ? ipBreakdown[0].ip : hop.ip
      result.push({
        ...hop,
        // 覆盖主行显示 IP
        ip: displayIp,
        stats: store.getHopStats(i),
        ipBreakdown,
        hasMultipleIps: ipBreakdown.length >= 2
      })
    } else {
      // 无响应的跳
      result.push({
        hop_number: i,
        ip: null,
        hostname: null,
        stats: { avg: 0, min: 0, max: 0, loss: 100, count: 0 },
        ipBreakdown: [],
        hasMultipleIps: false
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
      <div class="header-actions">
        <button class="history-btn" @click="historyOpen = true">
          📂 {{ t('traceHistory.button') }}
        </button>
      </div>
    </div>

    <!-- 历史模式横幅 -->
    <div v-if="store.isHistoricalView" class="history-banner">
      <span class="banner-icon">👁</span>
      <span class="banner-text">{{ t('traceHistory.viewBanner') }}</span>
      <button class="banner-exit" @click="store.exitHistoricalView()">
        {{ t('traceHistory.exitView') }}
      </button>
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
            :disabled="inputsDisabled"
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
            :disabled="inputsDisabled"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.maxHops') }}</label>
          <input
            v-model.number="maxHops"
            type="number"
            class="config-input"
            min="5" max="64"
            :disabled="inputsDisabled"
          />
        </div>
        <div class="config-field">
          <label class="config-label">{{ t('traceroute.timeoutMs') }}</label>
          <input
            v-model.number="timeoutMs"
            type="number"
            class="config-input"
            min="1000" max="10000" step="500"
            :disabled="inputsDisabled"
          />
        </div>
        <div class="config-field">
          <label class="config-label">
            {{ t('traceroute.probeMethod') }}
            <a
              v-if="
                probeMethod !== 'icmp' &&
                npcapStore.status.supported_platform &&
                !npcapStore.status.installed
              "
              class="learn-more-link"
              @click.prevent="openNpcapGuide(probeMethod)"
            >
              {{ t('traceroute.npcapTip.learnMore') }}
            </a>
          </label>
          <div class="probe-selector">
            <button
              v-for="method in ['icmp', 'udp', 'tcp'] as const"
              :key="method"
              :class="['probe-btn', { active: probeMethod === method }]"
              :disabled="inputsDisabled"
              :title="probeMethodTooltip(method)"
              @click="handleProbeMethodChange(method)"
            >
              {{ method.toUpperCase() }}
              <span
                v-if="probeMethodBadgeClass(method)"
                :class="['probe-badge', probeMethodBadgeClass(method)]"
              ></span>
            </button>
          </div>
        </div>
        <!-- TCP 目标端口：仅 probe_method='tcp' 时显示（默认 80，与 PingPlotter 一致） -->
        <div v-if="probeMethod === 'tcp'" class="config-field">
          <label class="config-label">{{ t('traceroute.tcpPort') }}</label>
          <input
            v-model.number="tcpPort"
            type="number"
            class="config-input"
            min="1" max="65535" step="1"
            :disabled="inputsDisabled"
          />
        </div>
      </div>
      <div class="config-actions">
        <button
          v-if="!store.isRunning"
          class="start-btn"
          @click="handleStart"
          :disabled="!targetInput.trim() || store.isHistoricalView"
        >
          {{ t('continuousTrace.start') }}
        </button>
        <button v-else class="stop-btn" @click="handleStop">
          {{ t('continuousTrace.stop') }}
        </button>
        <button
          class="clear-btn"
          @click="handleClear"
          :disabled="inputsDisabled"
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
          <template v-for="hop in hopStats" :key="hop.hop_number">
            <tr
              :class="{
                'no-response': !hop.ip,
                'selectable': !!hop.ip,
                'selected': !!hop.ip && selectedHopNumbers.includes(hop.hop_number)
              }"
              @click="toggleHopSelection(hop.hop_number, !!hop.ip, $event)"
            >
              <td>
                <!-- 多 IP 时显示展开/折叠按钮（PingPlotter Pro 风格） -->
                <button
                  v-if="hop.hasMultipleIps"
                  class="expand-btn"
                  :title="expandedHopNumbers.has(hop.hop_number) ? t('traceroute.collapseIps') : t('traceroute.expandIps', { count: hop.ipBreakdown.length })"
                  @click="toggleHopExpand(hop.hop_number, hop.hasMultipleIps, $event)"
                >
                  {{ expandedHopNumbers.has(hop.hop_number) ? '▼' : '▶' }}
                </button>
                <span
                  v-if="hop.ip && selectedHopNumbers.includes(hop.hop_number)"
                  class="hop-color-dot"
                  :style="{ background: colorForHop(hop.hop_number) }"
                ></span>
                {{ hop.hop_number }}
              </td>
              <td class="mono">
                {{ hop.ip || '* * *' }}
                <span v-if="hop.hasMultipleIps" class="ip-count-badge">
                  +{{ hop.ipBreakdown.length - 1 }}
                </span>
              </td>
              <td :class="getLatencyClass(hop.stats.avg)">{{ hop.stats.avg > 0 ? `${hop.stats.avg.toFixed(1)} ms` : '--' }}</td>
              <td>{{ hop.stats.min > 0 ? `${hop.stats.min.toFixed(1)} ms` : '--' }}</td>
              <td>{{ hop.stats.max > 0 ? `${hop.stats.max.toFixed(1)} ms` : '--' }}</td>
              <td :class="{ 'loss-high': hop.ip && hop.stats.loss > 5 }">{{ hop.ip ? `${hop.stats.loss.toFixed(1)}%` : '--' }}</td>
              <td class="muted">{{ hop.stats.count || '--' }}</td>
            </tr>
            <!-- 展开的 IP 分组子行 -->
            <tr
              v-for="ipRow in (expandedHopNumbers.has(hop.hop_number) ? hop.ipBreakdown : [])"
              :key="`${hop.hop_number}-${ipRow.ip}`"
              class="ip-subrow"
            >
              <td class="subrow-indent">↳</td>
              <td class="mono subrow-ip">{{ ipRow.ip }}</td>
              <td :class="getLatencyClass(ipRow.avg)">{{ ipRow.avg > 0 ? `${ipRow.avg.toFixed(1)} ms` : '--' }}</td>
              <td>{{ ipRow.min > 0 ? `${ipRow.min.toFixed(1)} ms` : '--' }}</td>
              <td>{{ ipRow.max > 0 ? `${ipRow.max.toFixed(1)} ms` : '--' }}</td>
              <td :class="{ 'loss-high': ipRow.loss > 5 }">{{ ipRow.loss.toFixed(1) }}%</td>
              <td class="muted">{{ ipRow.count }}</td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>

    <!-- 空状态 -->
    <div v-if="!store.isRunning && store.hops.length === 0 && !store.isDiscovering" class="empty-state">
      <div class="empty-icon">🗺️</div>
      <p>{{ t('continuousTrace.emptyHint') }}</p>
    </div>

    <!-- 历史会话抽屉 -->
    <TraceSessionHistory v-model:open="historyOpen" />

    <!-- Npcap 引导对话框：UDP/TCP 探测方式首次切换且未装 Npcap 时弹出 -->
    <NpcapGuideDialog
      :visible="npcapDialogVisible"
      :protocol="npcapDialogProtocol"
      @close="closeNpcapDialog"
      @acknowledged="acknowledgeNpcapDialog"
    />
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
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;

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

.header-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.history-btn {
  padding: 6px 14px;
  background: var(--input-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;

  &:hover {
    background: var(--hover-bg);
    border-color: var(--accent-color);
  }
}

.history-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  background: rgba(76, 175, 80, 0.08);
  border: 1px solid rgba(76, 175, 80, 0.3);
  border-radius: 10px;
  color: var(--text-primary);
  font-size: 12px;

  .banner-icon {
    font-size: 14px;
  }

  .banner-text {
    flex: 1;
    font-weight: 500;
  }

  .banner-exit {
    padding: 4px 12px;
    background: transparent;
    border: 1px solid #4CAF50;
    color: #4CAF50;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;

    &:hover {
      background: rgba(76, 175, 80, 0.15);
    }
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
  position: relative;
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

// Npcap 状态徽章：UDP/TCP 按钮右上角的小圆点
.probe-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  pointer-events: none;

  &.badge-enhanced {
    background: #10b981;        // 绿色 = Npcap 已装，完整模式
    box-shadow: 0 0 4px rgba(16, 185, 129, 0.6);
  }

  &.badge-basic {
    background: #f59e0b;        // 橙色 = Npcap 未装，基础模式
  }
}

// "了解详情"链接：Npcap 未装时显示在标签右侧
.learn-more-link {
  margin-left: 6px;
  font-size: 10px;
  font-weight: 400;
  color: var(--accent-color);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;

  &:hover {
    filter: brightness(1.2);
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

// 多 IP 展开/折叠按钮
.expand-btn {
  display: inline-block;
  padding: 0 4px;
  margin-right: 4px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 10px;
  cursor: pointer;
  vertical-align: middle;
  transition: color 0.15s;

  &:hover {
    color: var(--accent-color);
  }
}

// 多 IP 数量徽章：显示 +N 表示除主 IP 外还有几个
.ip-count-badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  background: var(--accent-color);
  color: white;
  border-radius: 10px;
  font-size: 10px;
  font-weight: 600;
  font-family: sans-serif;
  vertical-align: middle;
}

// 展开的 IP 分组子行
.stats-table tr.ip-subrow {
  background: rgba(0, 0, 0, 0.03);

  td {
    padding: 4px 10px;
    font-size: 11px;
    color: var(--text-secondary);
    border-bottom: 1px dashed var(--border-color);
  }

  &:hover td {
    background: rgba(0, 0, 0, 0.05);
  }

  .subrow-indent {
    text-align: right;
    padding-right: 8px;
    color: var(--text-muted);
  }

  .subrow-ip {
    color: var(--text-primary);
  }
}

// 暗色主题下的子行背景
[data-theme='dark'] .stats-table tr.ip-subrow {
  background: rgba(255, 255, 255, 0.03);

  &:hover td {
    background: rgba(255, 255, 255, 0.05);
  }
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
