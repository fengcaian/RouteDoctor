<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import type { DnsQueryResult } from '@/types'

const { t } = useI18n()

const domain = ref('')
const recordType = ref('ALL')
const isQuerying = ref(false)
const result = ref<DnsQueryResult | null>(null)
const error = ref('')

const recordTypes = ['ALL', 'A', 'AAAA', 'CNAME', 'MX', 'NS', 'TXT']

async function handleQuery() {
  if (!domain.value.trim()) return
  isQuerying.value = true
  error.value = ''
  result.value = null

  try {
    result.value = await invoke<DnsQueryResult>('dns_lookup', {
      domain: domain.value.trim(),
      recordType: recordType.value,
    })
  } catch (e: any) {
    error.value = typeof e === 'string' ? e : e.message || JSON.stringify(e)
  } finally {
    isQuerying.value = false
  }
}
</script>

<template>
  <div class="dns-view">
    <div class="view-header">
      <h2>{{ t('dns.title') }}</h2>
      <p class="subtitle">{{ t('dns.subtitle') }}</p>
    </div>

    <div class="dns-config">
      <div class="config-row">
        <div class="config-field domain-field">
          <label class="config-label">{{ t('dns.domain') }}</label>
          <input
            v-model="domain"
            type="text"
            class="config-input"
            :placeholder="t('dns.domainPlaceholder')"
            :disabled="isQuerying"
            @keyup.enter="handleQuery"
          />
        </div>
        <div class="config-field type-field">
          <label class="config-label">{{ t('dns.recordType') }}</label>
          <select v-model="recordType" class="config-input" :disabled="isQuerying">
            <option v-for="rt in recordTypes" :key="rt" :value="rt">{{ rt }}</option>
          </select>
        </div>
        <div class="config-field action-field">
          <label class="config-label">&nbsp;</label>
          <button
            class="query-btn"
            @click="handleQuery"
            :disabled="isQuerying || !domain.trim()"
          >
            {{ isQuerying ? t('dns.querying') : t('dns.query') }}
          </button>
        </div>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="error" class="error-state">
      <p>{{ error }}</p>
    </div>

    <!-- 查询结果 -->
    <div v-if="result" class="dns-results">
      <div class="result-header">
        <span class="result-domain">{{ result.domain }}</span>
        <span class="query-time">{{ t('dns.queryTime') }}: {{ result.query_time_ms.toFixed(1) }} ms</span>
      </div>

      <div v-if="result.records.length === 0" class="no-results">
        {{ t('dns.noResults') }}
      </div>

      <table v-else class="records-table">
        <thead>
          <tr>
            <th>{{ t('dns.type') }}</th>
            <th>{{ t('dns.value') }}</th>
            <th>{{ t('dns.ttl') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(record, idx) in result.records" :key="idx">
            <td class="type-cell">{{ record.record_type }}</td>
            <td class="value-cell">{{ record.value }}</td>
            <td class="ttl-cell">{{ record.ttl }}s</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 空状态 -->
    <div v-if="!result && !error && !isQuerying" class="empty-state">
      <div class="empty-icon">🔍</div>
      <p>{{ t('dns.emptyHint') }}</p>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.dns-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 12px;
  height: 100%;
  overflow-y: auto;
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

.dns-config {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 12px 16px;
}

.config-row {
  display: flex;
  gap: 12px;
  align-items: flex-end;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 4px;

  &.domain-field { flex: 1; }
  &.type-field { width: 120px; }
  &.action-field { width: auto; }
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
  }
}

select.config-input {
  cursor: pointer;
}

.query-btn {
  padding: 8px 20px;
  background: var(--accent-color);
  border: none;
  border-radius: 8px;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;

  &:hover:not(:disabled) {
    background: var(--accent-color-hover);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
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

.dns-results {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border-color);
}

.result-domain {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: monospace;
}

.query-time {
  font-size: 12px;
  color: var(--text-muted);
}

.no-results {
  text-align: center;
  color: var(--text-muted);
  padding: 20px;
  font-size: 13px;
}

.records-table {
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
  }

  .type-cell {
    width: 80px;
    font-weight: 600;
    color: var(--accent-color);
  }

  .value-cell {
    font-family: monospace;
    word-break: break-all;
  }

  .ttl-cell {
    width: 80px;
    color: var(--text-muted);
  }

  tr:hover {
    background: var(--hover-bg);
  }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
  gap: 8px;

  .empty-icon {
    font-size: 40px;
    opacity: 0.5;
  }

  p {
    font-size: 13px;
    margin: 0;
  }
}
</style>
