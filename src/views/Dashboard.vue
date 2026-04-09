<script setup lang="ts">
// Dashboard view - shows quick overview of all features
</script>

<template>
  <div class="dashboard-view">
    <div class="view-header">
      <h2>Dashboard</h2>
      <p class="subtitle">Network monitoring overview</p>
    </div>

    <div class="dashboard-content">
      <div class="quick-actions">
        <router-link to="/ping" class="action-card ping">
          <span class="action-icon">📡</span>
          <span class="action-title">Ping Monitor</span>
          <span class="action-desc">Real-time latency tracking</span>
        </router-link>

        <router-link to="/traceroute" class="action-card trace">
          <span class="action-icon">🔗</span>
          <span class="action-title">Traceroute</span>
          <span class="action-desc">Network path discovery</span>
        </router-link>

        <router-link to="/bandwidth" class="action-card bandwidth">
          <span class="action-icon">⚡</span>
          <span class="action-title">Bandwidth Test</span>
          <span class="action-desc">Speed measurement</span>
        </router-link>

        <router-link to="/history" class="action-card history">
          <span class="action-icon">📊</span>
          <span class="action-title">History</span>
          <span class="action-desc">View past results</span>
        </router-link>
      </div>

      <div class="info-section">
        <h3>Getting Started</h3>
        <p>Welcome to PingPlotter Next! This tool helps you monitor and diagnose network issues.</p>
        <div class="tips">
          <div class="tip">
            <strong>Ping Monitor:</strong> Continuously monitor latency and packet loss to any target.
          </div>
          <div class="tip">
            <strong>Traceroute:</strong> Discover the network path and identify bottlenecks.
          </div>
          <div class="tip">
            <strong>Bandwidth Test:</strong> Measure your download and upload speeds.
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
  gap: 20px;
}

.view-header {
  h2 {
    font-size: 24px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }

  .subtitle {
    font-size: 14px;
    color: var(--text-muted);
    margin-top: 4px;
  }
}

.dashboard-content {
  display: flex;
  flex-direction: column;
  gap: 30px;
}

.quick-actions {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 20px;
}

.action-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 30px 20px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  text-decoration: none;
  transition: all 0.2s ease;

  &:hover {
    transform: translateY(-4px);
    border-color: var(--accent-color);
    box-shadow: 0 4px 12px var(--accent-color-alpha);
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
}

.action-icon {
  font-size: 36px;
}

.action-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.action-desc {
  font-size: 13px;
  color: var(--text-muted);
}

.info-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 20px;

  h3 {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 10px;
  }

  p {
    color: var(--text-secondary);
    margin-bottom: 16px;
  }
}

.tips {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tip {
  padding: 10px 16px;
  background: var(--hover-bg);
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 13px;

  strong {
    color: var(--text-primary);
  }
}
</style>