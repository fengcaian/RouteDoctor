<script setup lang="ts">
import { ref, computed, onMounted, onActivated } from 'vue'
import { useI18n } from 'vue-i18n'
import { useHistoryStore, type HistorySession } from '@/stores/historyStore'
import ConfirmDialog from '@/components/common/ConfirmDialog.vue'
import HistoryDetail from '@/components/history/HistoryDetail.vue'
import { useToast } from '@/composables/useToast'
import { useExport } from '@/composables/useExport'

const { t } = useI18n()
const historyStore = useHistoryStore()
const toast = useToast()
const { exportHistoryJSON } = useExport()

// Filter state
const selectedType = ref<'all' | 'ping' | 'traceroute' | 'bandwidth'>('all')
const searchTarget = ref('')

// 详情弹窗状态
const showDetail = ref(false)
const selectedRecord = ref<HistorySession | null>(null)

// 首次挂载和每次从 keep-alive 激活时都加载历史记录
onMounted(() => {
  historyStore.loadHistory()
})

onActivated(() => {
  historyStore.loadHistory()
})

// Filtered records
const filteredRecords = computed(() => {
  return historyStore.records.filter(record => {
    const matchesType = selectedType.value === 'all' || record.test_type === selectedType.value
    const matchesSearch = searchTarget.value === '' ||
      record.target.toLowerCase().includes(searchTarget.value.toLowerCase())
    return matchesType && matchesSearch
  })
})

// Format date
function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString('zh-CN')
}

// Format duration
function formatDuration(start: number, end: number): string {
  const duration = end - start
  if (duration < 1000) return `${duration}ms`
  if (duration < 60000) return `${(duration / 1000).toFixed(1)}s`
  return `${Math.floor(duration / 60000)}m ${Math.floor((duration % 60000) / 1000)}s`
}

// Get statistics summary based on test type
function getSummary(record: HistorySession): string {
  try {
    const data = record.data
    if (record.test_type === 'ping' && data.statistics) {
      const stats = data.statistics
      return `${stats.sent} ${t('history.packets')}, ${stats.loss_rate}% ${t('history.loss')}, Avg ${stats.avg_ms.toFixed(1)}ms`
    }
    if (record.test_type === 'bandwidth') {
      return `${t('bandwidth.download')}: ${data.download_speed_mbps.toFixed(2)} Mbps, ${t('bandwidth.upload')}: ${data.upload_speed_mbps.toFixed(2)} Mbps`
    }
    if (record.test_type === 'traceroute') {
      const hops = data.hops?.length || 0
      const completed = data.completed ? t('history.completed') : t('history.incomplete')
      return `${hops} ${t('history.hops')}, ${completed}`
    }
  } catch (e) {
    console.error('Failed to parse summary:', e)
  }
  return '--'
}

// Get icon for test type
function getTypeIcon(type: string): string {
  switch (type) {
    case 'ping': return '📶'
    case 'traceroute': return '🛤️'
    case 'bandwidth': return '⚡'
    default: return '📋'
  }
}

// 点击记录查看详情
function openDetail(record: HistorySession) {
  selectedRecord.value = record
  showDetail.value = true
}

// 清除所有历史记录
const showClearConfirm = ref(false)

function handleClearHistory() {
  showClearConfirm.value = true
}

async function confirmClear() {
  showClearConfirm.value = false
  await historyStore.clearAllHistory()
  toast.success(t('history.clearSuccess'))
}
</script>

<template>
  <div class="history-view">
    <ConfirmDialog
      :visible="showClearConfirm"
      :title="$t('history.clearConfirmTitle')"
      :message="$t('history.clearConfirmMessage')"
      :confirmText="$t('common.clear')"
      :cancelText="$t('common.cancel')"
      @confirm="confirmClear"
      @cancel="showClearConfirm = false"
    />
    <HistoryDetail
      :visible="showDetail"
      :record="selectedRecord"
      @close="showDetail = false"
    />
    <div class="view-header">
      <div>
        <h2>{{ $t('history.title') }}</h2>
        <p class="subtitle">{{ $t('history.subtitle') }}</p>
      </div>
      <button
        class="export-btn"
        @click="exportHistoryJSON(historyStore.records)"
        :disabled="historyStore.isLoading || historyStore.records.length === 0"
      >
        {{ $t('history.export') }}
      </button>
      <button
        class="clear-history-btn"
        @click="handleClearHistory"
        :disabled="historyStore.isLoading || historyStore.records.length === 0"
      >
        {{ $t('history.clearHistory') }}
      </button>
    </div>

    <!-- Filter controls -->
    <div class="filter-controls">
      <div class="search-box">
        <input
          v-model="searchTarget"
          type="text"
          :placeholder="$t('history.searchPlaceholder')"
          class="search-input"
        />
        <button
          v-if="searchTarget"
          class="search-clear-btn"
          @click="searchTarget = ''"
        >✕</button>
      </div>
      <div class="type-filters">
        <button
          :class="['filter-btn', { active: selectedType === 'all' }]"
          @click="selectedType = 'all'"
        >
          {{ $t('history.all') }}
        </button>
        <button
          :class="['filter-btn', { active: selectedType === 'ping' }]"
          @click="selectedType = 'ping'"
        >
          📶 Ping
        </button>
        <button
          :class="['filter-btn', { active: selectedType === 'traceroute' }]"
          @click="selectedType = 'traceroute'"
        >
          🛤️ Trace
        </button>
        <button
          :class="['filter-btn', { active: selectedType === 'bandwidth' }]"
          @click="selectedType = 'bandwidth'"
        >
          ⚡ Bandwidth
        </button>
      </div>
    </div>

    <!-- Loading state -->
    <div v-if="historyStore.isLoading" class="loading-state">
      <p>{{ $t('history.loadingHistory') }}</p>
    </div>

    <!-- Error state -->
    <div v-else-if="historyStore.error" class="error-state">
      <p>{{ historyStore.error }}</p>
    </div>

    <!-- Empty state -->
    <div v-else-if="filteredRecords.length === 0" class="empty-state">
      <div class="empty-icon">📋</div>
      <p v-if="historyStore.records.length === 0">
        {{ $t('history.noRecords') }}<br>
        <span class="hint">{{ $t('history.noRecordsHint') }}</span>
      </p>
      <p v-else>
        {{ $t('history.noMatch') }}<br>
        <span class="hint">{{ $t('history.noMatchHint') }}</span>
      </p>
    </div>

    <!-- History list -->
    <div v-else class="history-list-wrapper">
      <div class="history-list">
        <div
          v-for="record in filteredRecords"
          :key="record.id"
          class="history-item"
          @click="openDetail(record)"
        >
          <div class="item-header">
            <span class="type-icon">{{ getTypeIcon(record.test_type) }}</span>
            <span class="test-type">{{ record.test_type }}</span>
            <span class="target-name">{{ record.target }}</span>
            <span class="item-duration">{{ formatDuration(record.start_time, record.end_time) }}</span>
          </div>
          <div class="item-summary">
            {{ getSummary(record) }}
          </div>
          <div class="item-time">
            {{ formatDate(record.start_time) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.history-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  height: 100%;
  overflow: hidden;
}

.view-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  gap: 8px;

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

  .export-btn {
    padding: 8px 16px;
    background: var(--accent-color);
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    transition: all 0.2s;

    &:hover:not(:disabled) {
      background: var(--accent-color-hover);
    }

    &:disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }
  }

  .clear-history-btn {
    padding: 8px 16px;
    background: transparent;
    color: var(--error-color, #ef4444);
    border: 1px solid var(--error-color, #ef4444);
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    transition: all 0.2s;

    &:hover:not(:disabled) {
      background: var(--error-color, #ef4444);
      color: white;
    }

    &:disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }
  }
}

.filter-controls {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
  flex-shrink: 0;

  .search-box {
    flex: 1;
    min-width: 200px;
    position: relative;
  }

  .search-input {
    width: 100%;
    padding: 8px 32px 8px 12px;
    background: var(--input-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 14px;

    &::placeholder {
      color: var(--text-muted);
    }

    &:focus {
      outline: none;
      border-color: var(--primary-color);
    }
  }

  .search-clear-btn {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    background: var(--border-color);
    color: var(--text-muted);
    border-radius: 50%;
    cursor: pointer;
    font-size: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;

    &:hover {
      background: var(--error-color);
      color: white;
    }
  }

  .type-filters {
    display: flex;
    gap: 8px;
  }

  .filter-btn {
    padding: 6px 12px;
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s;

    &:hover {
      border-color: var(--primary-color);
      color: var(--text-primary);
    }

    &.active {
      background: var(--primary-color);
      border-color: var(--primary-color);
      color: white;
    }
  }
}

.loading-state,
.error-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  text-align: center;
  color: var(--text-muted);

  .empty-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }

  .hint {
    font-size: 13px;
    color: var(--text-secondary);
    margin-top: 8px;
  }
}

.error-state {
  color: var(--error-color, #ef4444);
}

.history-list-wrapper {
  flex: 1;
  overflow-y: auto;
  min-height: 0;

  &::-webkit-scrollbar {
    width: 10px;
  }

  &::-webkit-scrollbar-track {
    border-radius: 0 12px 12px 0;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 12px;
  }

  &::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.history-item {
  background: var(--card-bg);
  border-radius: 10px;
  border: 1px solid var(--border-color);
  padding: 16px;
  transition: border-color 0.2s;
  cursor: pointer;

  &:hover {
    border-color: var(--primary-color);
    background: var(--hover-bg);
  }

  .item-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;

    .type-icon {
      font-size: 18px;
    }

    .test-type {
      font-size: 12px;
      font-weight: 600;
      text-transform: uppercase;
      color: var(--primary-color);
      background: rgba(var(--primary-color-rgb, 99, 102, 241), 0.1);
      padding: 2px 8px;
      border-radius: 4px;
    }

    .target-name {
      font-weight: 600;
      color: var(--text-primary);
      font-family: monospace;
      font-size: 14px;
    }

    .item-duration {
      margin-left: auto;
      font-size: 13px;
      color: var(--text-muted);
      font-weight: 500;
    }
  }

  .item-summary {
    font-size: 13px;
    color: var(--text-secondary);
    margin-bottom: 8px;
    font-family: monospace;
  }

  .item-time {
    font-size: 12px;
    color: var(--text-muted);
  }
}
</style>
