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
  // 保存 download 阶段完成时的速度，供仪表盘在 upload 阶段持续显示
  const downloadPhaseSpeed = ref(0)
  // Actions
  function setRunning(running: boolean) {
    isRunning.value = running
    if (running) {
      downloadPhaseSpeed.value = 0
      progress.value = {
        phase: 'idle',
        progress: 0,
        current_speed_mbps: 0,
        bytes_transferred: 0
      }
    }
  }

  function updateProgress(data: BandwidthProgress) {
    // 持续记录 download 阶段的最新速度
    if (data.phase === 'download' && data.current_speed_mbps > 0) {
      downloadPhaseSpeed.value = data.current_speed_mbps
    }
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
    downloadPhaseSpeed.value = 0
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
    downloadPhaseSpeed,
    setRunning,
    updateProgress,
    setResult,
    clearHistory,
    resetStore
  }
})