import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { BandwidthResult, BandwidthProgress } from '@/types'

export const useBandwidthStore = defineStore('bandwidth', () => {
  // State
  const lastResult = ref<BandwidthResult | null>(null)
  const progress = ref<BandwidthProgress>({
    phase: 'idle',
    progress: 0,
    current_speed_mbps: 0,
    bytes_transferred: 0
  })
  const isRunning = ref(false)
  const history = ref<BandwidthResult[]>([])

  // Actions
  function setRunning(running: boolean) {
    isRunning.value = running
    if (running) {
      progress.value = {
        phase: 'idle',
        progress: 0,
        current_speed_mbps: 0,
        bytes_transferred: 0
      }
    }
  }

  function updateProgress(data: BandwidthProgress) {
    progress.value = data
  }

  function setResult(result: BandwidthResult) {
    lastResult.value = result
    history.value.push(result)
    // Keep only last 100 results
    if (history.value.length > 100) {
      history.value.shift()
    }
  }

  function clearHistory() {
    history.value = []
  }

  function resetStore() {
    lastResult.value = null
    progress.value = {
      phase: 'idle',
      progress: 0,
      current_speed_mbps: 0,
      bytes_transferred: 0
    }
    isRunning.value = false
    history.value = []
  }

  return {
    lastResult,
    progress,
    isRunning,
    history,
    setRunning,
    updateProgress,
    setResult,
    clearHistory,
    resetStore
  }
})