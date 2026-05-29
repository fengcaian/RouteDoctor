<script setup lang="ts">
import { ref, computed, onActivated, onDeactivated, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import PingTabBar from '@/components/ping/PingTabBar.vue'
import PingChart from '@/components/ping/PingChart.vue'
import PingStats from '@/components/ping/PingStats.vue'
import PingConfig from '@/components/ping/PingConfig.vue'
import PingTable from '@/components/ping/PingTable.vue'
import { usePingStore } from '@/stores'
import { usePing, usePingListener } from '@/composables'
import { useToast } from '@/composables/useToast'
import { useAlertStore } from '@/stores/alertStore'
import type { TargetConfig, PingResult, PingTab } from '@/types'

const { t } = useI18n()
const pingStore = usePingStore()
const { startPing, stopPing } = usePing()
const toast = useToast()
const alertStore = useAlertStore()

// ==================== 任务 5.1：内部状态管理 ====================

// 标签页列表和活跃标签页 ID
const tabs = ref<PingTab[]>([])
const activeTabId = ref<string>('')

// 当前活跃标签页对象
const activeTab = computed<PingTab | undefined>(() =>
  tabs.value.find(tab => tab.id === activeTabId.value)
)

// 当前活跃目标地址
const activeTarget = computed<string>(() =>
  activeTab.value?.target ?? ''
)

// 是否有正在运行的目标（用于控制"全部停止"按钮显隐）
const hasRunningTargets = computed<boolean>(() =>
  tabs.value.some(tab => tab.target && pingStore.isRunning(tab.target))
)

// ==================== 任务 5.2：标签页管理方法 ====================

/**
 * 创建默认标签页（目标为 8.8.8.8）
 */
function createDefaultTab(): PingTab {
  const tab: PingTab = {
    id: crypto.randomUUID(),
    target: '8.8.8.8'
  }
  return tab
}

/**
 * 新增空白标签页，设为活跃
 */
function addTab(): void {
  const tab: PingTab = {
    id: crypto.randomUUID(),
    target: ''
  }
  tabs.value.push(tab)
  activeTabId.value = tab.id
}

/**
 * 关闭标签页：停止 Ping 会话，从 store 移除数据，从 tabs 中移除
 * 若关闭最后一个标签页，自动创建新的默认标签页
 */
async function closeTab(tabId: string): Promise<void> {
  const tabIndex = tabs.value.findIndex(tab => tab.id === tabId)
  if (tabIndex === -1) return

  const tab = tabs.value[tabIndex]

  // 停止该目标的 Ping 会话并从 store 移除数据
  if (tab.target) {
    if (pingStore.isRunning(tab.target)) {
      pingStore.setRunning(tab.target, false)
      try {
        await stopPing(tab.target)
      } catch (e) {
        console.error('停止 Ping 会话失败:', e)
      }
    }
    pingStore.removeTarget(tab.target)
  }

  // 从 tabs 中移除
  tabs.value.splice(tabIndex, 1)

  // 若关闭的是最后一个标签页，自动创建新的默认标签页
  if (tabs.value.length === 0) {
    const defaultTab = createDefaultTab()
    tabs.value.push(defaultTab)
    activeTabId.value = defaultTab.id
    return
  }

  // 若关闭的是当前活跃标签页，切换到相邻标签页
  if (activeTabId.value === tabId) {
    const newIndex = Math.min(tabIndex, tabs.value.length - 1)
    activeTabId.value = tabs.value[newIndex].id
  }
}

/**
 * 切换活跃标签页
 */
function selectTab(tabId: string): void {
  activeTabId.value = tabId
}

// ==================== 任务 5.3：Ping 操作处理方法 ====================

/**
 * 开始 Ping：更新当前标签页的 target 字段，调用 store 和后端启动 Ping
 */
async function handleStart(config: TargetConfig): Promise<void> {
  // 更新当前标签页的 target 字段
  const tab = activeTab.value
  if (tab) {
    tab.target = config.target
  }

  // 确保 store 中有该目标的数据
  if (!pingStore.getConfig(config.target)) {
    pingStore.addTarget(config)
  }

  pingStore.setRunning(config.target, true)
  try {
    await startPing(config)
  } catch (e: any) {
    // 启动失败，回退运行状态并通知用户
    pingStore.setRunning(config.target, false)
    toast.error(`Ping 启动失败: ${typeof e === 'string' ? e : e.message || '未知错误'}`)
  }
}

/**
 * 停止 Ping
 */
function handleStop(target: string): void {
  pingStore.setRunning(target, false)
  stopPing(target)
}

/**
 * 清除结果
 */
function handleClear(target: string): void {
  pingStore.clearResults(target)
}

/**
 * 全部停止：遍历所有运行中的目标，逐一停止 Ping 会话
 */
async function stopAllRunning(): Promise<void> {
  const runningTabs = tabs.value.filter(
    tab => tab.target && pingStore.isRunning(tab.target)
  )

  for (const tab of runningTabs) {
    pingStore.setRunning(tab.target, false)
    try {
      await stopPing(tab.target)
    } catch (e) {
      console.error(`停止目标 ${tab.target} 的 Ping 会话失败:`, e)
    }
  }
}

// ==================== 任务 5.5：keep-alive 生命周期 ====================

// 标记是否已初始化，避免重复创建默认标签页
let initialized = false

/**
 * 初始化：创建默认标签页（仅在 tabs 为空时）
 */
function initializeTabs(): void {
  if (tabs.value.length === 0) {
    const defaultTab = createDefaultTab()
    tabs.value.push(defaultTab)
    activeTabId.value = defaultTab.id
  }
}

// onMounted：首次挂载时初始化
onMounted(() => {
  if (!initialized) {
    initializeTabs()
    initialized = true
  }
})

// onActivated：从 keep-alive 缓存恢复时触发
// 仅在 tabs 为空时创建默认标签页（正常情况下 keep-alive 会保留状态）
onActivated(() => {
  if (!initialized) {
    initializeTabs()
    initialized = true
  }
})

// onDeactivated：离开页面时保持 Ping 会话继续运行
// 用户切换页面后 Ping 任务在后台继续，数据不会丢失
onDeactivated(() => {
  // 不做任何清理，保持 keep-alive 缓存状态
  // Ping 会话在后台继续运行，用户返回时可以看到最新数据
})

// 监听 Ping 结果事件，按 target 路由到 store，并检查告警
usePingListener(
  (result: PingResult) => {
    pingStore.addResult(result)
    // 检查告警规则
    alertStore.checkPingResult(result.target, result.latency_ms, result.is_timeout)
    // 同时检查丢包率告警
    const stats = pingStore.getStatistics(result.target)
    if (stats) {
      alertStore.checkLossRate(result.target, stats.loss_rate)
    }
  }
)
</script>

<template>
  <div class="ping-view">
    <div class="view-header">
      <div class="header-row">
        <div>
          <h2>{{ $t('ping.title') }}</h2>
          <p class="subtitle">{{ $t('ping.subtitle') }}</p>
        </div>
        <!-- 全部停止按钮，仅在有运行中的目标时显示 -->
        <button
          v-if="hasRunningTargets"
          class="stop-all-btn"
          @click="stopAllRunning"
        >
          {{ t('ping.stopAll') }}
        </button>
      </div>
    </div>

    <!-- 任务 5.4：标签栏 -->
    <PingTabBar
      :tabs="tabs"
      :active-tab-id="activeTabId"
      @select="selectTab"
      @add="addTab"
      @close="closeTab"
    />

    <div class="ping-content">
      <div class="ping-main">
        <PingConfig
          :target="activeTarget"
          :is-running="pingStore.isRunning(activeTarget)"
          @start="handleStart"
          @stop="handleStop"
          @clear="handleClear"
        />

        <PingChart :target="activeTarget" />

        <PingStats :target="activeTarget" />

        <PingTable :target="activeTarget" />
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  min-width: 0;
}

.view-header {
  margin-bottom: 8px;
  flex-shrink: 0;

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

.header-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
}

/* 全部停止按钮 */
.stop-all-btn {
  padding: 6px 14px;
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  background: var(--error-color, #f44336);
  color: white;
  transition: all 0.2s ease;
  white-space: nowrap;
  flex-shrink: 0;

  &:hover {
    background: var(--error-color-hover, #d32f2f);
    box-shadow: 0 2px 8px rgba(244, 67, 54, 0.3);
  }
}

.ping-content {
  display: flex;
  gap: 12px;
  min-width: 0;
}

.ping-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}
</style>
