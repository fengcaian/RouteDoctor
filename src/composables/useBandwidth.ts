import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { BandwidthResult, BandwidthProgress } from '@/types'

export function useBandwidth() {
  const isRunning = ref(false)
  const error = ref<string | null>(null)

  // Start bandwidth test
  async function startBandwidthTest(): Promise<void> {
    error.value = null
    try {
      await invoke('start_bandwidth_test')
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  // Stop bandwidth test
  async function stopBandwidthTest(): Promise<void> {
    try {
      await invoke('stop_bandwidth_test')
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  return {
    isRunning,
    error,
    startBandwidthTest,
    stopBandwidthTest
  }
}

// Listen to bandwidth test progress
export function useBandwidthListener(
  onProgress: (progress: BandwidthProgress) => void,
  onComplete?: (result: BandwidthResult) => void
) {
  let unlistenProgress: (() => void) | null = null
  let unlistenComplete: (() => void) | null = null

  async function startListening() {
    unlistenProgress = await listen<BandwidthProgress>('bandwidth-progress', (event) => {
      onProgress(event.payload)
    })

    if (onComplete) {
      unlistenComplete = await listen<BandwidthResult>('bandwidth-complete', (event) => {
        onComplete(event.payload)
      })
    }
  }

  onMounted(() => {
    startListening()
  })

  onUnmounted(() => {
    unlistenProgress?.()
    unlistenComplete?.()
  })
}