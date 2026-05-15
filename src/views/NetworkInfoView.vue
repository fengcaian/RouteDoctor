<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import type { NetworkInfo, PublicIpInfo } from '@/types'

const { t } = useI18n()

const networkInfo = ref<NetworkInfo | null>(null)
const publicIpInfo = ref<PublicIpInfo | null>(null)
const isLoading = ref(false)
const isPublicIpLoading = ref(false)
const error = ref('')
const publicIpError = ref('')

async function loadNetworkInfo() {
  isLoading.value = true
  error.value = ''
  try {
    networkInfo.value = await invoke<NetworkInfo>('get_network_info')
  } catch (e: any) {
    error.value = typeof e === 'string' ? e : e.message || JSON.stringify(e)
  } finally {
    isLoading.value = false
  }
}

async function loadPublicIpInfo() {
  isPublicIpLoading.value = true
  publicIpError.value = ''
  try {
    publicIpInfo.value = await invoke<PublicIpInfo>('get_public_ip_info')
  } catch (e: any) {
    publicIpError.value = typeof e === 'string' ? e : e.message || JSON.stringify(e)
  } finally {
    isPublicIpLoading.value = false
  }
}

async function refreshAll() {
  loadNetworkInfo()
  loadPublicIpInfo()
}

onMounted(() => {
  refreshAll()
})
</script>

<template>
  <div class="network-info-view">
    <div class="view-header">
      <div>
        <h2>{{ t('networkInfo.title') }}</h2>
        <p class="subtitle">{{ t('networkInfo.subtitle') }}</p>
      </div>
      <button class="refresh-btn" @click="refreshAll" :disabled="isLoading">
        {{ t('networkInfo.refresh') }}
      </button>
    </div>

    <div v-if="isLoading && !networkInfo" class="loading-state">
      <p>{{ t('networkInfo.loading') }}</p>
    </div>

    <div v-else-if="error" class="error-state">
      <p>{{ error }}</p>
    </div>

    <template v-if="networkInfo">
      <!-- 基本信息 -->
      <div class="info-cards">
        <div class="info-card">
          <span class="card-icon">💻</span>
          <div class="card-content">
            <span class="card-label">{{ t('networkInfo.hostname') }}</span>
            <span class="card-value">{{ networkInfo.hostname }}</span>
          </div>
        </div>
        <div class="info-card">
          <span class="card-icon">🌐</span>
          <div class="card-content">
            <span class="card-label">{{ t('networkInfo.localIp') }}</span>
            <span class="card-value mono">{{ networkInfo.local_ip || t('networkInfo.unknown') }}</span>
          </div>
        </div>
        <div class="info-card">
          <span class="card-icon">🚪</span>
          <div class="card-content">
            <span class="card-label">{{ t('networkInfo.defaultGateway') }}</span>
            <span class="card-value mono">{{ networkInfo.default_gateway || t('networkInfo.unknown') }}</span>
          </div>
        </div>
      </div>

      <!-- 公网信息 -->
      <div class="info-section public-ip-section">
        <h3 class="section-title">🌍 {{ t('networkInfo.publicIp') }}</h3>
        <div v-if="isPublicIpLoading && !publicIpInfo" class="public-ip-loading">
          {{ t('networkInfo.publicIpLoading') }}
        </div>
        <div v-else-if="publicIpError" class="public-ip-error">
          {{ t('networkInfo.publicIpError') }}: {{ publicIpError }}
        </div>
        <div v-else-if="publicIpInfo" class="public-ip-grid">
          <div class="public-ip-item highlight">
            <span class="pip-label">{{ t('networkInfo.publicIpAddress') }}</span>
            <span class="pip-value mono">{{ publicIpInfo.ip }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.isp">
            <span class="pip-label">{{ t('networkInfo.publicIpIsp') }}</span>
            <span class="pip-value">{{ publicIpInfo.isp }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.country">
            <span class="pip-label">{{ t('networkInfo.publicIpCountry') }}</span>
            <span class="pip-value">{{ publicIpInfo.country }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.region">
            <span class="pip-label">{{ t('networkInfo.publicIpRegion') }}</span>
            <span class="pip-value">{{ publicIpInfo.region }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.city">
            <span class="pip-label">{{ t('networkInfo.publicIpCity') }}</span>
            <span class="pip-value">{{ publicIpInfo.city }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.org">
            <span class="pip-label">{{ t('networkInfo.publicIpOrg') }}</span>
            <span class="pip-value">{{ publicIpInfo.org }}</span>
          </div>
          <div class="public-ip-item" v-if="publicIpInfo.timezone">
            <span class="pip-label">{{ t('networkInfo.publicIpTimezone') }}</span>
            <span class="pip-value">{{ publicIpInfo.timezone }}</span>
          </div>
        </div>
      </div>

      <!-- DNS 服务器 -->
      <div class="info-section">
        <h3 class="section-title">{{ t('networkInfo.dnsServers') }}</h3>
        <div class="dns-list">
          <div v-for="(dns, idx) in networkInfo.dns_servers" :key="idx" class="dns-item">
            <span class="dns-icon">📡</span>
            <span class="dns-value">{{ dns }}</span>
          </div>
          <div v-if="networkInfo.dns_servers.length === 0" class="empty-hint">
            {{ t('networkInfo.unknown') }}
          </div>
        </div>
      </div>

      <!-- 网络接口 -->
      <div class="info-section">
        <h3 class="section-title">{{ t('networkInfo.interfaces') }}</h3>
        <div v-if="networkInfo.interfaces.length === 0" class="empty-hint">
          {{ t('networkInfo.noInterfaces') }}
        </div>
        <table v-else class="interfaces-table">
          <thead>
            <tr>
              <th>{{ t('networkInfo.interfaceName') }}</th>
              <th>{{ t('networkInfo.interfaceIp') }}</th>
              <th>{{ t('networkInfo.interfaceType') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(iface, idx) in networkInfo.interfaces" :key="idx">
              <td>{{ iface.name }}</td>
              <td class="mono">{{ iface.ip }}</td>
              <td>{{ iface.interface_type }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </div>
</template>

<style lang="scss" scoped>
.network-info-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 12px;
  height: 100%;
  overflow-y: auto;
}

.view-header {
  display: flex;
  justify-content: space-between;
  align-items: center;

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

.refresh-btn {
  padding: 8px 16px;
  background: var(--accent-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) {
    background: var(--accent-color-hover);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
  font-size: 13px;
}

.error-state {
  padding: 12px 16px;
  background: rgba(244, 67, 54, 0.1);
  border: 1px solid rgba(244, 67, 54, 0.3);
  border-radius: 8px;
  color: var(--error-color);
  font-size: 13px;

  p { margin: 0; }
}

.info-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.info-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
}

.card-icon {
  font-size: 24px;
}

.card-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.card-label {
  font-size: 11px;
  color: var(--text-muted);
}

.card-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);

  &.mono {
    font-family: monospace;
  }
}

.info-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 12px 0;
}

.dns-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.dns-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--hover-bg);
  border-radius: 6px;
  font-family: monospace;
  font-size: 13px;
  color: var(--text-primary);
}

.dns-icon {
  font-size: 12px;
}

.dns-value {
  font-weight: 500;
}

.empty-hint {
  color: var(--text-muted);
  font-size: 13px;
}

.interfaces-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;

  th {
    padding: 8px 12px;
    text-align: left;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border-color);
  }

  td {
    padding: 8px 12px;
    font-size: 13px;
    color: var(--text-primary);
    border-bottom: 1px solid var(--border-color);

    &.mono {
      font-family: monospace;
    }
  }

  tr:hover {
    background: var(--hover-bg);
  }
}

.public-ip-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.public-ip-loading {
  color: var(--text-muted);
  font-size: 13px;
  padding: 8px 0;
}

.public-ip-error {
  color: var(--error-color);
  font-size: 13px;
  padding: 8px 0;
}

.public-ip-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 12px;
}

.public-ip-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 14px;
  background: var(--hover-bg);
  border-radius: 8px;

  &.highlight {
    background: rgba(var(--accent-color-rgb, 59, 130, 246), 0.1);
    border: 1px solid rgba(var(--accent-color-rgb, 59, 130, 246), 0.2);
  }
}

.pip-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.pip-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);

  &.mono {
    font-family: monospace;
  }
}
</style>
