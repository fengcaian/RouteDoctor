import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { TracerouteResult, HopResult, ProbeMethod } from '@/types'

export function useTraceroute() {
  const isRunning = ref(false)
  const error = ref<string | null>(null)

  // Run traceroute
  async function runTraceroute(
    target: string,
    maxHops: number = 30,
    timeoutMs: number = 2000,
    probeMethod: ProbeMethod = 'icmp'
  ): Promise<void> {
    error.value = null
    isRunning.value = true
    try {
      await invoke('run_traceroute', {
        target,
        maxHops,
        timeoutMs,
        probeMethod
      })
    } catch (e) {
      error.value = String(e)
      console.error('run_traceroute error:', e)
      isRunning.value = false
      throw e
    }
  }

  // Stop traceroute
  async function stopTraceroute(target: string): Promise<void> {
    try {
      await invoke('stop_traceroute', { target })
      isRunning.value = false
    } catch (e) {
      error.value = String(e)
      console.error('stop_traceroute error:', e)
      throw e
    }
  }

  return {
    isRunning,
    error,
    runTraceroute,
    stopTraceroute
  }
}

// Listen to traceroute results
export function useTracerouteListener(
  onHop: (hop: HopResult & { target: string }) => void,
  onComplete?: (result: TracerouteResult) => void
) {
  let unlistenHop: (() => void) | null = null
  let unlistenComplete: (() => void) | null = null

  async function startListening() {
    unlistenHop = await listen<HopResult & { target: string }>('trace-hop', (event) => {
      console.log('Received trace-hop:', event.payload)
      onHop(event.payload)
    })

    if (onComplete) {
      unlistenComplete = await listen<TracerouteResult>('trace-complete', (event) => {
        console.log('Received trace-complete:', event.payload)
        onComplete(event.payload)
      })
    }
    console.log('Traceroute listener started')
  }

  onMounted(() => {
    startListening()
  })

  onUnmounted(() => {
    unlistenHop?.()
    unlistenComplete?.()
  })
}