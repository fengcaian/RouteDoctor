<script setup lang="ts">
import { onMounted } from 'vue'
import AppSidebar from '@/components/common/AppSidebar.vue'
import StatusBar from '@/components/common/StatusBar.vue'
import ToastContainer from '@/components/common/ToastContainer.vue'
import { stopAllPings } from '@/composables/usePing'
import { usePingStore, useTracerouteStore, useBandwidthStore } from '@/stores'

const pingStore = usePingStore()
const tracerouteStore = useTracerouteStore()
const bandwidthStore = useBandwidthStore()

// 初始化：清理所有后台任务
onMounted(async () => {
  // 停止后端所有 ping 任务
  await stopAllPings()

  // 清理前端状态
  pingStore.resetStore()
  tracerouteStore.resetStore()
  bandwidthStore.resetStore()

  console.log('App initialized - all sessions cleared')
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