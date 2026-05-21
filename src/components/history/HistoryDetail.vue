<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import type { HistorySession } from '@/stores/historyStore'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  record: HistorySession | null
}>()

const emit = defineEmits<{
  close: []
}>()

// Esc 键关闭弹窗
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) {
    emit('close')
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})

// 格式化时间
function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString('zh-CN')
}

// 格式化持续时间
function formatDuration(start: number, end: number): string {
  const duration = end - start
  if (duration < 1000) return `${duration}ms`
  if (duration < 60000) return `${(duration / 1000).toFixed(1)}s`
  return `${Math.floor(duration / 60000)}m ${Math.floor((duration % 60000) / 1000)}s`
}

// 获取类型图标
function getTypeIcon(type: string): string {
  switch (type) {
    case 'ping': return '📶'
    case 'traceroute': return '🛤️'
    case 'bandwidth': return '⚡'
    default: return '📋'
  }
}

// Ping 详情数据
const pingData = computed(() => {
  if (!props.record || props.record.test_type !== 'ping') return null
  const data = props.record.data
  if (!data || !data.statistics) return null
  return {
    statistics: data.statistics,
    results: data.results || []
  }
})

// Bandwidth 详情数据
const bandwidthData = computed(() => {
  if (!props.record || props.record.test_type !== 'bandwidth') return null
  return props.record.data
})

// Traceroute 详情数据
const tracerouteData = computed(() => {
  if (!props.record || props.record.test_type !== 'traceroute') return null
  return props.record.data
})
</script>

<template>
  <Teleport to="body">
    <Transition name="detail-fade">
      <div v-if="visible && record" class="detail-overlay" @click.self="emit('close')">
        <div class="detail-panel">
          <!-- 头部 -->
          <div class="detail-header">
            <div class="header-info">
              <span class="type-icon">{{ getTypeIcon(record.test_type) }}</span>
              <div class="header-text">
                <h3 class="detail-title">{{ record.target }}</h3>
                <span class="detail-type">{{ record.test_type.toUpperCase() }}</span>
              </div>
            </div>
            <button class="close-btn" @click="emit('close')">✕</button>
          </div>

          <!-- 基本信息 -->
          <div class="detail-meta">
            <div class="meta-item">
              <span class="meta-label">{{ t('history.detail.startTime') }}</span>
              <span class="meta-value">{{ formatDate(record.start_time) }}</span>
            </div>
            <div class="meta-item">
              <span class="meta-label">{{ t('history.detail.endTime') }}</span>
              <span class="meta-value">{{ formatDate(record.end_time) }}</span>
            </div>
            <div class="meta-item">
              <span class="meta-label">{{ t('history.detail.duration') }}</span>
              <span class="meta-value">{{ formatDuration(record.start_time, record.end_time) }}</span>
            </div>
          </div>

          <!-- Ping 详情 -->
          <template v-if="record.test_type === 'ping' && pingData">
            <div class="detail-section">
              <h4 class="section-title">{{ t('history.detail.statistics') }}</h4>
              <div class="stats-grid">
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.sent') }}</span>
                  <span class="stat-value">{{ pingData.statistics.sent }}</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.received') }}</span>
                  <span class="stat-value success">{{ pingData.statistics.received }}</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.lost') }}</span>
                  <span class="stat-value error">{{ pingData.statistics.lost }} ({{ pingData.statistics.loss_rate }}%)</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.min') }}</span>
                  <span class="stat-value">{{ pingData.statistics.min_ms?.toFixed(1) || '--' }} ms</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.max') }}</span>
                  <span class="stat-value">{{ pingData.statistics.max_ms?.toFixed(1) || '--' }} ms</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">{{ t('ping.avg') }}</span>
                  <span class="stat-value highlight">{{ pingData.statistics.avg_ms?.toFixed(1) || '--' }} ms</span>
                </div>
              </div>
            </div>
          </template>

          <!-- Bandwidth 详情 -->
          <template v-if="record.test_type === 'bandwidth' && bandwidthData">
            <div class="detail-section">
              <h4 class="section-title">{{ t('history.detail.testResults') }}</h4>
              <div class="bandwidth-results">
                <div class="bw-card download">
                  <span class="bw-icon">⬇️</span>
                  <div class="bw-info">
                    <span class="bw-label">{{ t('bandwidth.download') }}</span>
                    <span class="bw-value">{{ bandwidthData.download_speed_mbps?.toFixed(2) }} Mbps</span>
                  </div>
                </div>
                <div class="bw-card upload">
                  <span class="bw-icon">⬆️</span>
                  <div class="bw-info">
                    <span class="bw-label">{{ t('bandwidth.upload') }}</span>
                    <span class="bw-value">{{ bandwidthData.upload_speed_mbps?.toFixed(2) }} Mbps</span>
                  </div>
                </div>
                <div class="bw-card latency">
                  <span class="bw-icon">⏱️</span>
                  <div class="bw-info">
                    <span class="bw-label">{{ t('bandwidth.latency') }}</span>
                    <span class="bw-value">{{ bandwidthData.latency_ms?.toFixed(1) }} ms</span>
                  </div>
                </div>
              </div>
              <div v-if="bandwidthData.server" class="server-info">
                <span class="meta-label">{{ t('history.detail.server') }}</span>
                <span class="meta-value">{{ bandwidthData.server }}</span>
              </div>
            </div>
          </template>

          <!-- Traceroute 详情 -->
          <template v-if="record.test_type === 'traceroute' && tracerouteData">
            <div class="detail-section">
              <h4 class="section-title">{{ t('history.detail.routePath') }}</h4>
              <div class="trace-meta">
                <span class="trace-status" :class="{ completed: tracerouteData.completed }">
                  {{ tracerouteData.completed ? t('history.completed') : t('history.incomplete') }}
                </span>
                <span class="trace-hops">{{ tracerouteData.hops?.length || 0 }} {{ t('history.hops') }}</span>
              </div>
              <div v-if="tracerouteData.hops && tracerouteData.hops.length > 0" class="hops-table-wrapper">
                <table class="hops-table">
                  <thead>
                    <tr>
                      <th>#</th>
                      <th>IP</th>
                      <th>{{ t('traceroute.avgLatency') }}</th>
                      <th>{{ t('traceroute.lossRate') }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="hop in tracerouteData.hops" :key="hop.hop_number">
                      <td>{{ hop.hop_number }}</td>
                      <td class="mono">{{ hop.ip || '* * *' }}</td>
                      <td>{{ hop.avg_latency ? `${hop.avg_latency.toFixed(1)} ms` : '--' }}</td>
                      <td :class="{ 'loss-high': hop.packet_loss > 10 }">
                        {{ hop.packet_loss?.toFixed(0) || 0 }}%
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </template>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style lang="scss" scoped>
.detail-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  backdrop-filter: blur(3px);
  padding: 20px;
}

.detail-panel {
  background: var(--card-bg, #1e1e1e);
  border: 1px solid var(--border-color, #333);
  border-radius: 16px;
  width: 100%;
  max-width: 560px;
  max-height: 80vh;
  overflow-y: auto;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;

  &::-webkit-scrollbar {
    width: 8px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 4px;
  }
}

.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--border-color);
  position: sticky;
  top: 0;
  background: var(--card-bg, #1e1e1e);
  z-index: 1;
}

.header-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.type-icon {
  font-size: 28px;
}

.header-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.detail-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  font-family: monospace;
}

.detail-type {
  font-size: 11px;
  font-weight: 600;
  color: var(--accent-color);
  background: rgba(var(--accent-color-rgb, 99, 102, 241), 0.1);
  padding: 2px 8px;
  border-radius: 4px;
  width: fit-content;
}

.close-btn {
  width: 32px;
  height: 32px;
  border: none;
  background: var(--hover-bg);
  color: var(--text-muted);
  border-radius: 8px;
  cursor: pointer;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;

  &:hover {
    background: var(--error-color);
    color: white;
  }
}

.detail-meta {
  display: flex;
  gap: 16px;
  padding: 16px 24px;
  border-bottom: 1px solid var(--border-color);
  flex-wrap: wrap;
}

.meta-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.meta-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.meta-value {
  font-size: 13px;
  color: var(--text-primary);
  font-weight: 500;
}

.detail-section {
  padding: 16px 24px 20px;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 12px 0;
}

// Ping 统计网格
.stats-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.stat-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  background: var(--hover-bg);
  border-radius: 8px;
}

.stat-label {
  font-size: 11px;
  color: var(--text-muted);
}

.stat-value {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);

  &.success { color: #4CAF50; }
  &.error { color: #f44336; }
  &.highlight { color: var(--accent-color); }
}

// Bandwidth 结果
.bandwidth-results {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.bw-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  background: var(--hover-bg);
  border-radius: 10px;
  border-left: 4px solid transparent;

  &.download { border-left-color: #4CAF50; }
  &.upload { border-left-color: #2196F3; }
  &.latency { border-left-color: #FF9800; }
}

.bw-icon {
  font-size: 20px;
}

.bw-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.bw-label {
  font-size: 12px;
  color: var(--text-muted);
}

.bw-value {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}

.server-info {
  margin-top: 12px;
  padding: 10px 14px;
  background: var(--hover-bg);
  border-radius: 8px;
  display: flex;
  gap: 8px;
  align-items: center;
}

// Traceroute 详情
.trace-meta {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 12px;
}

.trace-status {
  font-size: 12px;
  font-weight: 500;
  padding: 3px 10px;
  border-radius: 10px;
  background: rgba(244, 67, 54, 0.1);
  color: #f44336;

  &.completed {
    background: rgba(76, 175, 80, 0.1);
    color: #4CAF50;
  }
}

.trace-hops {
  font-size: 12px;
  color: var(--text-muted);
}

.hops-table-wrapper {
  max-height: 240px;
  overflow-y: auto;
  border-radius: 8px;
  border: 1px solid var(--border-color);

  &::-webkit-scrollbar {
    width: 6px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 3px;
  }
}

.hops-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;

  th {
    padding: 8px 12px;
    text-align: left;
    color: var(--text-muted);
    font-weight: 600;
    background: var(--hover-bg);
    position: sticky;
    top: 0;
  }

  td {
    padding: 6px 12px;
    color: var(--text-primary);
    border-bottom: 1px solid var(--border-color);

    &.mono {
      font-family: monospace;
      font-size: 11px;
    }

    &.loss-high {
      color: #f44336;
      font-weight: 600;
    }
  }

  tr:hover td {
    background: var(--hover-bg);
  }
}

// 过渡动画
.detail-fade-enter-active,
.detail-fade-leave-active {
  transition: opacity 0.25s ease;

  .detail-panel {
    transition: transform 0.25s ease;
  }
}

.detail-fade-enter-from,
.detail-fade-leave-to {
  opacity: 0;

  .detail-panel {
    transform: scale(0.95) translateY(10px);
  }
}
</style>
