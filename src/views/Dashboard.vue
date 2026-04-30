<script setup lang="ts">
import { useI18n } from 'vue-i18n'
const { t } = useI18n()
</script>

<template>
  <div class="dashboard-view">
    <div class="view-header">
      <h2>{{ t('dashboard.title') }}</h2>
      <p class="subtitle">{{ t('dashboard.subtitle') }}</p>
    </div>

    <div class="dashboard-content">
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

      <div class="info-section">
        <h3>{{ t('dashboard.gettingStarted') }}</h3>
        <p>{{ t('dashboard.welcome') }}</p>
        <div class="tips">
          <div class="tip">
            <strong>{{ t('dashboard.pingMonitor') }}:</strong> {{ t('dashboard.tipPing') }}
          </div>
          <div class="tip">
            <strong>{{ t('dashboard.traceroute') }}:</strong> {{ t('dashboard.tipTrace') }}
          </div>
          <div class="tip">
            <strong>{{ t('dashboard.bandwidthTest') }}:</strong> {{ t('dashboard.tipBandwidth') }}
          </div>
          <div class="tip">
            <strong>{{ t('dashboard.dnsLookup') }}:</strong> {{ t('dashboard.tipDns') }}
          </div>
          <div class="tip">
            <strong>{{ t('dashboard.networkInfo') }}:</strong> {{ t('dashboard.tipNetworkInfo') }}
          </div>
        </div>
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

  &.ping {
    .action-icon {
      color: #4CAF50;
    }
  }

  &.trace {
    .action-icon {
      color: #2196F3;
    }
  }

  &.bandwidth {
    .action-icon {
      color: #FF9800;
    }
  }

  &.history {
    .action-icon {
      color: #9C27B0;
    }
  }

  &.dns {
    .action-icon {
      color: #00BCD4;
    }
  }

  &.network-info {
    .action-icon {
      color: #607D8B;
    }
  }
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

.info-section {
  background: var(--card-bg);
  border-radius: 10px;
  border: 1px solid var(--border-color);
  padding: 14px 16px;

  h3 {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 8px;
  }

  p {
    color: var(--text-secondary);
    margin-bottom: 10px;
    font-size: 13px;
  }
}

.tips {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tip {
  padding: 7px 12px;
  background: var(--hover-bg);
  border-radius: 6px;
  color: var(--text-secondary);
  font-size: 12px;

  strong {
    color: var(--text-primary);
  }
}
</style>