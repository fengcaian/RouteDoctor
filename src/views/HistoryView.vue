<script setup lang="ts">
import { ref, computed, onMounted, onActivated } from 'vue'
import { useHistoryStore, type HistorySession } from '@/stores/historyStore'
import ConfirmDialog from '@/components/common/ConfirmDialog.vue'

const historyStore = useHistoryStore()

// Filter state
const selectedType = ref<'all' | 'ping' | 'traceroute' | 'bandwidth'>('all')
const searchTarget = ref('')

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
      return `${stats.sent} packets, ${stats.loss_rate}% loss, Avg ${stats.avg_ms.toFixed(1)}ms`
    }
    if (record.test_type === 'bandwidth') {
      return `Down: ${data.download_speed_mbps.toFixed(2)} Mbps, Up: ${data.upload_speed_mbps.toFixed(2)} Mbps`
    }
    if (record.test_type === 'traceroute') {
      const hops = data.hops?.length || 0
      const completed = data.completed ? 'Completed' : 'Incomplete'
      return `${hops} hops, ${completed}`
    }
  } catch (e) {
    console.error('Failed to parse summary:', e)
  }
  return 'No summary available'
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

// Clear filter
function clearFilter() {
  selectedType.value = 'all'
  searchTarget.value = ''
}

// 清除所有历史记录
const showClearConfirm = ref(false)

function handleClearHistory() {
  showClearConfirm.value = true
}

async function confirmClear() {
  showClearConfirm.value = false
  await historyStore.clearAllHistory()
}
</script>

<template>
  <div class="history-view">
    <ConfirmDialog
      :visible="showClearConfirm"
      title="清除历史记录"
      message="是否清除所有历史记录？此操作不可撤销。"
      confirmText="清除"
      cancelText="取消"
      @confirm="confirmClear"
      @cancel="showClearConfirm = false"
    />
    <div class="view-header">
      <div>
        <h2>History</h2>
        <p class="subtitle">View and export previous test results</p>
      </div>
      <button
        class="clear-history-btn"
        @click="handleClearHistory"
        :disabled="historyStore.isLoading || historyStore.records.length === 0"
      >
        清除历史
      </button>
    </div>

    <!-- Filter controls -->
    <div class="filter-controls">
      <div class="search-box">
        <input
          v-model="searchTarget"
          type="text"
          placeholder="Search target..."
          class="search-input"
        />
      </div>
      <div class="type-filters">
        <button
          :class="['filter-btn', { active: selectedType === 'all' }]"
          @click="selectedType = 'all'"
        >
          All
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
      <button v-if="selectedType !== 'all' || searchTarget" class="clear-btn" @click="clearFilter">
        Clear
      </button>
    </div>

    <!-- Loading state -->
    <div v-if="historyStore.isLoading" class="loading-state">
      <p>Loading history...</p>
    </div>

    <!-- Error state -->
    <div v-else-if="historyStore.error" class="error-state">
      <p>{{ historyStore.error }}</p>
    </div>

    <!-- Empty state -->
    <div v-else-if="filteredRecords.length === 0" class="empty-state">
      <div class="empty-icon">📋</div>
      <p v-if="historyStore.records.length === 0">
        No history records yet.<br>
        <span class="hint">Run a Ping, Traceroute, or Bandwidth test to create history.</span>
      </p>
      <p v-else>
        No records match your filter.<br>
        <span class="hint">Try adjusting your search or filter.</span>
      </p>
    </div>

    <!-- History list -->
    <div v-else class="history-list-wrapper">
      <div class="history-list">
        <div
          v-for="record in filteredRecords"
          :key="record.id"
          class="history-item"
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
  }

  .search-input {
    width: 100%;
    padding: 8px 12px;
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

  .clear-btn {
    padding: 6px 12px;
    background: transparent;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s;

    &:hover {
      border-color: var(--text-muted);
      color: var(--text-primary);
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

  &:hover {
    border-color: var(--primary-color);
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
