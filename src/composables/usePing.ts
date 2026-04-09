import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { PingResult, TargetConfig } from '@/types'

export function usePing() {
  const isRunning = ref(false)
  const error = ref<string | null>(null)

  // Start continuous ping
  async function startPing(config: TargetConfig): Promise<void> {
    error.value = null
    try {
      await invoke('start_ping', {
        target: config.target,
        intervalMs: config.interval_ms,
        timeoutMs: config.timeout_ms,
        packetSize: config.packet_size
      })
      isRunning.value = true
    } catch (e) {
      error.value = String(e)
      console.error('start_ping error:', e)
      throw e
    }
  }

  // Stop ping
  async function stopPing(target: string): Promise<void> {
    try {
      await invoke('stop_ping', { target })
      isRunning.value = false
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  // Single ping
  async function pingOnce(target: string, timeoutMs: number = 3000, packetSize: number = 64): Promise<PingResult> {
    error.value = null
    try {
      return await invoke<PingResult>('ping_once', { target, timeoutMs, packetSize })
    } catch (e) {
      error.value = String(e)
      console.error('ping_once error:', e)
      throw e
    }
  }

  return {
    isRunning,
    error,
    startPing,
    stopPing,
    pingOnce
  }
}

// Stop all active ping sessions
export async function stopAllPings(): Promise<void> {
  try {
    await invoke('stop_all_pings')
    console.log('All ping sessions stopped')
  } catch (e) {
    console.error('stop_all_pings error:', e)
  }
}

// 全局事件监听器引用，避免重复注册
let unlistenGlobal: (() => void) | null = null
let unlistenStatsGlobal: (() => void) | null = null
let listenerRefCount = 0

// Listen to ping results (单例模式，避免重复注册)
export function usePingListener(
  onResult: (result: PingResult) => void,
  onStats?: (stats: any) => void
) {
  let isUnsubscribed = false

  onMounted(async () => {
    listenerRefCount++

    // 只在第一次使用时注册监听器
    if (listenerRefCount === 1) {
      try {
        unlistenGlobal = await listen<PingResult>('ping-result', (event) => {
          if (!isUnsubscribed) {
            onResult(event.payload)
          }
        })

        if (onStats) {
          unlistenStatsGlobal = await listen('ping-stats', (event) => {
            if (!isUnsubscribed) {
              onStats(event.payload)
            }
          })
        }
        console.log('Ping listener started (global)')
      } catch (e) {
        console.error('Failed to start ping listener:', e)
      }
    }
  })

  onUnmounted(() => {
    isUnsubscribed = true
    listenerRefCount--

    // 当没有组件使用时才取消注册
    if (listenerRefCount === 0) {
      unlistenGlobal?.()
      unlistenStatsGlobal?.()
      unlistenGlobal = null
      unlistenStatsGlobal = null
      console.log('Ping listener stopped (global)')
    }
  })
}