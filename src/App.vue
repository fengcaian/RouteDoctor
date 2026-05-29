<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import AppSidebar from '@/components/common/AppSidebar.vue'
import StatusBar from '@/components/common/StatusBar.vue'
import ToastContainer from '@/components/common/ToastContainer.vue'
import { stopAllPings } from '@/composables/usePing'
import { usePingStore, useTracerouteStore, useBandwidthStore } from '@/stores'
import { useToast } from '@/composables/useToast'

const pingStore = usePingStore()
const tracerouteStore = useTracerouteStore()
const bandwidthStore = useBandwidthStore()
const { info } = useToast()

let unlistenFirstMinimize: (() => void) | null = null

// 初始化：清理所有后台任务
onMounted(async () => {
  // 停止后端所有 ping 任务
  await stopAllPings()

  // 清理前端状态
  pingStore.resetStore()
  tracerouteStore.resetStore()
  bandwidthStore.resetStore()

  // 监听首次最小化提示
  unlistenFirstMinimize = await listen<string>('first-minimize', (event) => {
    info(event.payload, 5000)
  })

  console.log('App initialized - all sessions cleared')
})

onUnmounted(() => {
  unlistenFirstMinimize?.()
})
</script>

<template>
  <div class="app-container" :data-theme="'dark'">
    <AppSidebar />
    <div class="main-area">
      <main class="content">
        <router-view v-slot="{ Component }">
          <keep-alive>
            <component :is="Component" />
          </keep-alive>
        </router-view>
      </main>
      <StatusBar />
    </div>
    <ToastContainer />
  </div>
</template>

<style lang="scss" scoped>
.app-container {
  display: flex;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}

.main-area {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-width: 0;
}

.content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 20px;
  background: var(--main-bg);
  min-width: 0;
}
</style>