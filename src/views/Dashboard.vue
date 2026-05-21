<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { usePingStore, useBandwidthStore } from '@/stores'
import { useHistoryStore } from '@/stores/historyStore'

const { t } = useI18n()
const pingStore = usePingStore()
const bandwidthStore = useBandwidthStore()
const historyStore = useHistoryStore()

// 实时状态数据
const runningPingCount = computed(() => pingStore.runningTargets.size)

const latestPingStats = computed(() => {
  // 获取所有正在运行的目标中最新的统计数据
  const targets = Array.from(pingStore.runningTargets)
  if (targets.length === 0) return null

  // 取第一个运行中目标的统计
  const target = targets[0]
  const stats = pingStore.getStatistics(target)
  if (stats.sent === 0) return null
  return { target, stats }
})

const lastBandwidthResult = computed(() => bandwidthStore.lastResult)
const isBandwidthRunning = computed(() => bandwidthStore.isRunning)

const historyCount = computed(() => historyStore.records.length)
</script>

<template>
  <div class="dashboard-view">
    <div class="view-header">
      <h2>{{ t('dashboard.title') }}</h2>
      <p class="subtitle">{{ t('dashboard.subtitle') }}</p>
    </div>

    <div class="dashboard-content">
      <!-- 实时状态概览 -->
      <div class="status-overview">
        <!-- Ping 状态卡片 -->
        <div class="status-card" :class="{ active: runningPingCount > 0 }">
          <div class="status-card-header">
            <span class="status-icon">📡</span>
            <span class="status-badge" :class="runningPingCount > 0 ? 'running' : 'idle'">
              {{ runningPingCount > 0 ? t('dashboard.status.running') : t('dashboard.status.idle') }}
            </span>
          </div>
          <div class="status-card-body">
            <template v-if="latestPingStats">
              <div class="status-metric">
                <span class="metric-value" :class="{
                  good: latestPingStats.stats.avg_ms < 50,
                  medium: latestPingStats.stats.avg_ms >= 50 && latestPingStats.stats.avg_ms < 100,
                  bad: latestPingStats.stats.avg_ms >= 100
                }">
                  {{ latestPingStats.stats.avg_ms.toFixed(1) }} ms
                </span>
                <span class="metric-label">{{ t('dashboard.status.avgLatency') }}</span>
              </div>
              <div class="status-detail">
                <span class="detail-item">
                  {{ t('dashboard.status.target') }}: {{ latestPingStats.target }}
                </span>
                <span class="detail-item" :class="{ 'loss-warn': latestPingStats.stats.loss_rate > 0 }">
                  {{ t('dashboard.status.loss') }}: {{ latestPingStats.stats.loss_rate.toFixed(1) }}%
                </span>
              </div>
            </template>
            <template v-else>
              <span class="status-empty">{{ t('dashboard.status.noPing') }}</span>
            </template>
          </div>
          <router-link to="/ping" class="status-card-link">
            {{ t('dashboard.status.goTo') }} →
          </router-link>
        </div>

        <!-- 带宽状态卡片 -->
        <div class="status-card" :class="{ active: isBandwidthRunning }">
          <div class="status-card-header">
            <span class="status-icon">⚡</span>
            <span class="status-badge" :class="isBandwidthRunning ? 'running' : 'idle'">
              {{ isBandwidthRunning ? t('dashboard.status.testing') : t('dashboard.status.idle') }}
            </span>
          </div>
          <div class="status-card-body">
            <template v-if="lastBandwidthResult">
              <div class="bw-metrics">
                <div class="bw-metric">
                  <span class="metric-value download">{{ lastBandwidthResult.download_speed_mbps.toFixed(1) }}</span>
                  <span class="metric-unit">Mbps ⬇</span>
                </div>
                <div class="bw-metric">
                  <span class="metric-value upload">{{ lastBandwidthResult.upload_speed_mbps.toFixed(1) }}</span>
                  <span class="metric-unit">Mbps ⬆</span>
                </div>
              </div>
            </template>
            <template v-else-if="isBandwidthRunning">
              <div class="status-metric">
                <span class="metric-value testing">{{ bandwidthStore.progress.current_speed_mbps.toFixed(1) }} Mbps</span>
                <span class="metric-label">{{ bandwidthStore.progress.phase === 'download' ? t('bandwidth.testingDownload') : t('bandwidth.testingUpload') }}</span>
              </div>
            </template>
            <template v-else>
              <span class="status-empty">{{ t('dashboard.status.noBandwidth') }}</span>
            </template>
          </div>
          <router-link to="/bandwidth" class="status-card-link">
            {{ t('dashboard.status.goTo') }} →
          </router-link>
        </div>

        <!-- 历史记录卡片 -->
        <div class="status-card">
          <div class="status-card-header">
            <span class="status-icon">📊</span>
            <span class="status-badge info">{{ historyCount }} {{ t('dashboard.status.records') }}</span>
          </div>
          <div class="status-card-body">
            <span class="status-empty" v-if="historyCount === 0">{{ t('dashboard.status.noHistory') }}</span>
            <span class="history-summary" v-else>{{ t('dashboard.status.historyHint') }}</span>
          </div>
          <router-link to="/history" class="status-card-link">
            {{ t('dashboard.status.goTo') }} →
          </router-link>
        </div>
      </div>

      <!-- 快速操作 -->
      <div class="quick-actions">
        <router-link to="/ping" class="action-card ping">
          <span class="action-icon">📡</span>
          <span class="action-title">{{ t('dashboard.pingMonitor') }}</span>
          <span class="action-desc">{{ t('dashboard.pingDesc') }}</span>
        </router-link>

        <router-link to="/traceroute" class="action-card trace">
          <span class="action-icon">🔗</span>
          <span class="action-title">{{ t('dashboard.traceroute') }}</span>
          <span class="action-desc">{{ t('dashboard.traceDesc') }}</span>
        </router-link>

        <router-link to="/bandwidth" class="action-card bandwidth">
          <span class="action-icon">⚡</span>
          <span class="action-title">{{ t('dashboard.bandwidthTest') }}</span>
          <span class="action-desc">{{ t('dashboard.bandwidthDesc') }}</span>
        </router-link>

        <router-link to="/dns" class="action-card dns">
          <span class="action-icon">🔍</span>
          <span class="action-title">{{ t('dashboard.dnsLookup') }}</span>
          <span class="action-desc">{{ t('dashboard.dnsDesc') }}</span>
        </router-link>

        <router-link to="/network-info" class="action-card network-info">
          <span class="action-icon">🌐</span>
          <span class="action-title">{{ t('dashboard.networkInfo') }}</span>
          <span class="action-desc">{{ t('dashboard.networkInfoDesc') }}</span>
        </router-link>

        <router-link to="/history" class="action-card history">
          <span class="action-icon">📊</span>
          <span class="action-title">{{ t('dashboard.history') }}</span>
          <span class="action-desc">{{ t('dashboard.historyDesc') }}</span>
        </router-link>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.dashboard-view {
  display: flex;
  flex-direction: column;
  gap: 14px;
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

.dashboard-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

// 实时状态概览
.status-overview {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 12px;
}

.status-card {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  transition: all 0.2s ease;

  &.active {
    border-color: var(--accent-color);
    box-shadow: 0 0 12px rgba(var(--accent-color-rgb, 99, 102, 241), 0.1);
  }
}

.status-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.status-icon {
  font-size: 22px;
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

  &.testing {
    background: rgba(255, 152, 0, 0.15);
    color: #FF9800;
  }

  &.idle {
    background: var(--hover-bg);
    color: var(--text-muted);
  }

  &.info {
    background: rgba(33, 150, 243, 0.15);
    color: #2196F3;
  }
}

.status-card-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.status-metric {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.metric-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--text-primary);

  &.good { color: #4CAF50; }
  &.medium { color: #FF9800; }
  &.bad { color: #f44336; }
  &.download { color: #4CAF50; }
  &.upload { color: #2196F3; }
  &.testing { color: #FF9800; font-size: 18px; }
}

.metric-label {
  font-size: 11px;
  color: var(--text-muted);
}

.metric-unit {
  font-size: 11px;
  color: var(--text-muted);
  margin-left: 2px;
}

.bw-metrics {
  display: flex;
  gap: 20px;
}

.bw-metric {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.status-detail {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.detail-item {
  font-size: 11px;
  color: var(--text-muted);

  &.loss-warn {
    color: #FF9800;
  }
}

.status-empty {
  font-size: 12px;
  color: var(--text-muted);
}

.history-summary {
  font-size: 12px;
  color: var(--text-secondary);
}

.status-card-link {
  font-size: 12px;
  color: var(--accent-color);
  text-decoration: none;
  font-weight: 500;
  transition: opacity 0.2s;

  &:hover {
    opacity: 0.8;
  }
}

// 快速操作
.quick-actions {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
}

.action-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 16px 12px;
  background: var(--card-bg);
  border-radius: 10px;
  border: 1px solid var(--border-color);
  text-decoration: none;
  transition: all 0.2s ease;

  &:hover {
    transform: translateY(-2px);
    border-color: var(--accent-color);
    box-shadow: 0 2px 8px var(--accent-color-alpha);
  }

  &.ping .action-icon { color: #4CAF50; }
  &.trace .action-icon { color: #2196F3; }
  &.bandwidth .action-icon { color: #FF9800; }
  &.history .action-icon { color: #9C27B0; }
  &.dns .action-icon { color: #00BCD4; }
  &.network-info .action-icon { color: #607D8B; }
}

.action-icon {
  font-size: 28px;
}

.action-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.action-desc {
  font-size: 12px;
  color: var(--text-muted);
}
</style>
