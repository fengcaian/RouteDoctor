<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { usePingStore } from '@/stores'
import type { PingTab } from '@/types'

const { t } = useI18n()
const pingStore = usePingStore()

// Props 定义
defineProps<{
  tabs: PingTab[]
  activeTabId: string
}>()

// 事件定义
const emit = defineEmits<{
  (e: 'select', tabId: string): void
  (e: 'add'): void
  (e: 'close', tabId: string): void
}>()

// 标签页状态类型
type TabStatus = 'running' | 'stopped' | 'timeout'

/**
 * 获取标签页运行状态
 * 绿色=运行中、灰色=已停止、红色=超时
 */
function getTabStatus(target: string): TabStatus {
  if (!target || !pingStore.isRunning(target)) return 'stopped'
  const latest = pingStore.getLatestResult(target)
  if (latest?.is_timeout) return 'timeout'
  return 'running'
}

/**
 * 获取标签页最新延迟文本
 */
function getTabLatency(target: string): string {
  if (!target) return ''
  const latest = pingStore.getLatestResult(target)
  if (!latest) return ''
  if (latest.is_timeout) return t('ping.tabTimeout')
  return `${latest.latency_ms?.toFixed(1)}ms`
}

/**
 * 获取标签页显示文本
 * 空目标显示"新标签页"
 */
function getTabLabel(target: string): string {
  return target || t('ping.newTab')
}

/**
 * 关闭标签页，阻止事件冒泡到 select
 */
function handleClose(event: Event, tabId: string) {
  event.stopPropagation()
  emit('close', tabId)
}
</script>

<template>
  <div class="ping-tab-bar">
    <div class="tabs-scroll-container">
      <div class="tabs-list">
        <!-- 标签页列表 -->
        <div
          v-for="tab in tabs"
          :key="tab.id"
          class="tab-item"
          :class="{ active: tab.id === activeTabId }"
          @click="emit('select', tab.id)"
          :title="getTabLabel(tab.target)"
        >
          <!-- 运行状态指示器 -->
          <span
            class="status-dot"
            :class="getTabStatus(tab.target)"
          />
          <!-- 目标地址文本 -->
          <span class="tab-label">{{ getTabLabel(tab.target) }}</span>
          <!-- 最新延迟值 -->
          <span
            v-if="getTabLatency(tab.target)"
            class="tab-latency"
            :class="{ timeout: getTabStatus(tab.target) === 'timeout' }"
          >
            {{ getTabLatency(tab.target) }}
          </span>
          <!-- 关闭按钮 -->
          <button
            class="close-btn"
            :title="t('ping.closeTab')"
            @click="handleClose($event, tab.id)"
          >
            ×
          </button>
        </div>
      </div>
    </div>
    <!-- 新增标签页按钮 -->
    <button
      class="add-btn"
      :title="t('ping.addTab')"
      @click="emit('add')"
    >
      +
    </button>
  </div>
</template>

<style scoped>
.ping-tab-bar {
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 6px 8px;
  min-height: 42px;
}

/* 横向滚动容器 */
.tabs-scroll-container {
  flex: 1;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
  scrollbar-color: var(--border-color) transparent;
}

.tabs-scroll-container::-webkit-scrollbar {
  height: 3px;
}

.tabs-scroll-container::-webkit-scrollbar-track {
  background: transparent;
}

.tabs-scroll-container::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 2px;
}

.tabs-list {
  display: flex;
  gap: 4px;
  white-space: nowrap;
}

/* 单个标签页 */
.tab-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  background: transparent;
  border: 1px solid transparent;
  flex-shrink: 0;
  max-width: 200px;
}

.tab-item:hover {
  background: var(--hover-bg);
}

.tab-item.active {
  background: var(--input-bg);
  border-color: var(--accent-color);
}

/* 运行状态指示器圆点 */
.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  transition: background-color 0.3s ease;
}

/* 绿色=运行中 */
.status-dot.running {
  background-color: #4CAF50;
  box-shadow: 0 0 4px rgba(76, 175, 80, 0.5);
}

/* 灰色=已停止 */
.status-dot.stopped {
  background-color: #666;
}

/* 红色=超时 */
.status-dot.timeout {
  background-color: #f44336;
  box-shadow: 0 0 4px rgba(244, 67, 54, 0.5);
}

/* 目标地址文本 */
.tab-label {
  font-size: 12px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100px;
}

/* 最新延迟值 */
.tab-latency {
  font-size: 10px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.tab-latency.timeout {
  color: #f44336;
}

/* 关闭按钮 */
.close-btn {
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text-muted);
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  opacity: 0;
  transition: all 0.15s ease;
}

.tab-item:hover .close-btn {
  opacity: 1;
}

.close-btn:hover {
  background: var(--border-color);
  color: var(--text-primary);
}

/* 新增标签页按钮 */
.add-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--border-color);
  background: transparent;
  color: var(--text-muted);
  border-radius: 8px;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.2s ease;
}

.add-btn:hover {
  background: var(--hover-bg);
  color: var(--text-primary);
  border-color: var(--accent-color);
}
</style>
