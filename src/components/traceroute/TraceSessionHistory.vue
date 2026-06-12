<script setup lang="ts">
/**
 * 路径监控会话历史抽屉
 *
 * 数据来源：后端 list_trace_sessions / load_trace_hops / load_trace_samples
 * 加载行为：点击"加载" → 拉取 hop 元信息 + 最近 N 条样本 → 写入 store 的历史只读模式
 *
 * 注意：长会话可能有几十万条样本，前端不应一次拉全。MVP 实现：固定加载最近 5000 条
 * （30 跳 × 0.5Hz ≈ 5.5 分钟，能覆盖最常见的"看一下结尾发生了什么"场景）。
 * 后续可以加时间范围选择器或分页加载。
 */
import { ref, watch, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useTraceHistory, useToast, useContinuousTrace } from '@/composables'
import type { TraceSessionRow } from '@/composables/useTraceHistory'
import { useContinuousTraceStore } from '@/stores/continuousTraceStore'

const SAMPLE_LIMIT = 5000

const props = defineProps<{
  open: boolean
}>()
const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'loaded', sessionId: number): void
}>()

const { t, locale } = useI18n()
const traceHistory = useTraceHistory()
const toast = useToast()
const store = useContinuousTraceStore()
const { stopContinuousTrace } = useContinuousTrace()

const sessions = ref<TraceSessionRow[]>([])
const loading = ref(false)
const loadingId = ref<number | null>(null)

async function refresh() {
  loading.value = true
  try {
    sessions.value = await traceHistory.listSessions(50)
  } catch (e: any) {
    toast.error(`${t('traceHistory.loadFailed')}: ${typeof e === 'string' ? e : e.message ?? ''}`)
  } finally {
    loading.value = false
  }
}

// 抽屉打开时刷新列表
watch(
  () => props.open,
  (val) => { if (val) refresh() },
  { immediate: true }
)

function close() {
  emit('update:open', false)
}

function fmtTime(ts: number): string {
  const d = new Date(ts)
  const lang = locale.value === 'zh' ? 'zh-CN' : 'en-US'
  return d.toLocaleString(lang, { hour12: false })
}

function fmtDuration(start: number, end: number | null): string {
  const ms = (end ?? Date.now()) - start
  if (ms < 1000) return `${ms} ms`
  const s = Math.floor(ms / 1000)
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m ${s % 60}s`
  const h = Math.floor(m / 60)
  return `${h}h ${m % 60}m`
}

function statusLabel(status: string): string {
  switch (status) {
    case 'running': return t('traceHistory.statusRunning')
    case 'stopped': return t('traceHistory.statusStopped')
    case 'crashed': return t('traceHistory.statusCrashed')
    default: return status
  }
}

function statusClass(status: string): string {
  switch (status) {
    case 'running': return 'status-running'
    case 'stopped': return 'status-stopped'
    case 'crashed': return 'status-crashed'
    default: return ''
  }
}

const isCurrentlyViewing = computed(() => store.isHistoricalView)

async function handleLoad(session: TraceSessionRow) {
  // 当前正在实时监控？先停掉
  if (store.isRunning && !store.isHistoricalView) {
    if (!window.confirm(t('traceHistory.confirmReplace'))) return
    try {
      await stopContinuousTrace(store.target)
    } catch { /* ignore */ }
    store.stopMonitoring()
  }

  loadingId.value = session.id
  try {
    const [hops, samples] = await Promise.all([
      traceHistory.loadHops(session.id),
      traceHistory.loadSamples(session.id, { limit: SAMPLE_LIMIT })
    ])
    store.loadHistoricalSession(session, hops, samples)
    emit('loaded', session.id)
    close()
    // 提示样本数 = 所有跳的探测次数总和(不是时间点数,也不是节点数)
    // 例如 30 跳里 26 个有响应,跑了 4 秒 ≈ 26×2 = 52 条样本(0.5Hz)
    const validHopCount = hops.filter(h => h.ip).length
    toast.info(t('traceHistory.loadLimit', { hops: validHopCount, n: samples.length }))
  } catch (e: any) {
    toast.error(`${t('traceHistory.loadFailed')}: ${typeof e === 'string' ? e : e.message ?? ''}`)
  } finally {
    loadingId.value = null
  }
}

async function handleDelete(session: TraceSessionRow) {
  if (!window.confirm(t('traceHistory.deleteConfirm'))) return
  try {
    await traceHistory.deleteSession(session.id)
    // 如果删除的是正在查看的，退出查看
    if (store.isHistoricalView && store.loadedSessionId === session.id) {
      store.exitHistoricalView()
    }
    await refresh()
  } catch (e: any) {
    toast.error(typeof e === 'string' ? e : e.message ?? '')
  }
}
</script>

<template>
  <transition name="drawer">
    <div v-if="open" class="drawer-overlay" @click.self="close">
      <div class="drawer">
        <header class="drawer-header">
          <h3>{{ t('traceHistory.title') }}</h3>
          <div class="header-actions">
            <button class="icon-btn" :disabled="loading" @click="refresh" :title="t('traceHistory.refresh')">
              <span class="refresh-icon" :class="{ spinning: loading }">↻</span>
            </button>
            <button class="icon-btn" @click="close" title="✕">✕</button>
          </div>
        </header>

        <div class="drawer-body">
          <div v-if="loading && sessions.length === 0" class="state-msg">
            {{ t('traceHistory.loading') }}
          </div>

          <div v-else-if="sessions.length === 0" class="state-msg empty">
            <div class="empty-icon">📂</div>
            <div>{{ t('traceHistory.empty') }}</div>
            <div class="hint">{{ t('traceHistory.emptyHint') }}</div>
          </div>

          <ul v-else class="session-list">
            <li
              v-for="s in sessions"
              :key="s.id"
              class="session-item"
              :class="{ active: store.loadedSessionId === s.id && isCurrentlyViewing }"
            >
              <div class="session-main">
                <div class="row-1">
                  <span class="target mono">{{ s.target }}</span>
                  <span class="status-badge" :class="statusClass(s.status)">
                    {{ statusLabel(s.status) }}
                  </span>
                </div>
                <div class="row-2">
                  <span class="time">{{ fmtTime(s.started_at) }}</span>
                  <span class="dot">·</span>
                  <span>{{ fmtDuration(s.started_at, s.ended_at) }}</span>
                  <span class="dot">·</span>
                  <span>{{ s.ping_interval_ms }}ms / {{ s.probe_method.toUpperCase() }}</span>
                </div>
              </div>
              <div class="session-actions">
                <button
                  class="action-btn primary"
                  :disabled="loadingId === s.id"
                  @click="handleLoad(s)"
                >
                  {{ loadingId === s.id ? t('traceHistory.loading') : t('traceHistory.load') }}
                </button>
                <button class="action-btn danger" @click="handleDelete(s)">
                  {{ t('traceHistory.delete') }}
                </button>
              </div>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </transition>
</template>

<style lang="scss" scoped>
.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 100;
  display: flex;
  justify-content: flex-end;
}

.drawer {
  width: 480px;
  max-width: 90vw;
  background: var(--card-bg);
  border-left: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  box-shadow: -4px 0 24px rgba(0, 0, 0, 0.3);
}

.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-color);

  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }
}

.header-actions {
  display: flex;
  gap: 4px;
}

.icon-btn {
  width: 32px;
  height: 32px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  transition: background 0.15s;

  &:hover:not(:disabled) {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

.refresh-icon {
  display: inline-block;
  &.spinning {
    animation: spin 0.8s linear infinite;
  }
}

@keyframes spin { to { transform: rotate(360deg); } }

.drawer-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}

.state-msg {
  padding: 32px 16px;
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;

  &.empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;

    .empty-icon { font-size: 36px; opacity: 0.6; }
    .hint { font-size: 11px; opacity: 0.7; max-width: 280px; }
  }
}

.session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.session-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border-color);
  transition: background 0.15s;

  &:hover {
    background: var(--hover-bg);
  }

  &.active {
    background: rgba(76, 175, 80, 0.08);
    border-left: 3px solid #4CAF50;
    padding-left: 13px;
  }
}

.session-main {
  flex: 1;
  min-width: 0;
}

.row-1 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.target {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mono { font-family: monospace; }

.status-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 8px;
  text-transform: uppercase;
  letter-spacing: 0.3px;

  &.status-running {
    background: rgba(76, 175, 80, 0.15);
    color: #4CAF50;
  }
  &.status-stopped {
    background: rgba(127, 127, 127, 0.15);
    color: var(--text-muted);
  }
  &.status-crashed {
    background: rgba(244, 67, 54, 0.15);
    color: #F44336;
  }
}

.row-2 {
  font-size: 11px;
  color: var(--text-muted);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;

  .time {
    font-family: monospace;
  }

  .dot {
    opacity: 0.5;
  }
}

.session-actions {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex-shrink: 0;
}

.action-btn {
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border-color);
  background: var(--input-bg);
  color: var(--text-secondary);
  transition: all 0.15s;
  white-space: nowrap;

  &:hover:not(:disabled) {
    border-color: var(--accent-color);
    color: var(--text-primary);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  &.primary {
    background: var(--accent-color);
    border-color: var(--accent-color);
    color: white;
    &:hover:not(:disabled) { background: var(--accent-color-hover); }
  }

  &.danger:hover:not(:disabled) {
    border-color: #F44336;
    color: #F44336;
    background: rgba(244, 67, 54, 0.08);
  }
}

// 抽屉滑入动画
.drawer-enter-active, .drawer-leave-active {
  transition: opacity 0.2s;
  .drawer {
    transition: transform 0.25s ease-out;
  }
}
.drawer-enter-from, .drawer-leave-to {
  opacity: 0;
  .drawer { transform: translateX(100%); }
}
</style>
