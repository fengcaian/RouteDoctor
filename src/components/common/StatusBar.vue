<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { usePingStore } from '@/stores'

const { t } = useI18n()
const pingStore = usePingStore()

const runningCount = computed(() => pingStore.runningTargets.size)
const statusText = computed(() => {
  if (runningCount.value === 0) {
    return t('statusBar.ready')
  }
  return t('statusBar.targetsRunning', { count: runningCount.value })
})

const currentTime = ref(new Date().toLocaleTimeString())
let timer: ReturnType<typeof setInterval> | null = null

onMounted(() => {
  timer = setInterval(() => {
    currentTime.value = new Date().toLocaleTimeString()
  }, 1000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <footer class="status-bar">
    <div class="status-left">
      <span class="status-indicator" :class="{ active: runningCount > 0 }"></span>
      <span class="status-text">{{ statusText }}</span>
    </div>

    <div class="status-center">
      <span class="app-name">RouteDoctor v0.1.0</span>
    </div>

    <div class="status-right">
      <span class="current-time">{{ currentTime }}</span>
    </div>
  </footer>
</template>

<style lang="scss" scoped>
.status-bar {
  height: 30px;
  background: var(--statusbar-bg);
  border-top: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.status-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);
  transition: background 0.3s ease;

  &.active {
    background: var(--success-color);
    animation: pulse 1.5s infinite;
  }
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

.status-text {
  color: var(--text-secondary);
}

.status-center {
  display: flex;
  align-items: center;
}

.app-name {
  color: var(--text-muted);
}

.status-right {
  display: flex;
  align-items: center;
}

.current-time {
  color: var(--text-secondary);
}
</style>