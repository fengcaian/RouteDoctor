import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// 持续路径监控的单跳结果
export interface ContinuousTraceHopResult {
  target: string
  hop_number: number
  hop_ip: string
  latency_ms: number | null
  is_timeout: boolean
  timestamp: number
  seq: number
}

// 路径发现结果
export interface PathDiscovered {
  target: string
  hops: {
    hop_number: number
    ip: string | null
    hostname: string | null
  }[]
}

export function useContinuousTrace() {
  async function startContinuousTrace(
    target: string,
    maxHops: number = 30,
    timeoutMs: number = 3000,
    pingIntervalMs: number = 2000,
    probeMethod: string = 'icmp'
  ): Promise<void> {
    await invoke('start_continuous_trace', {
      target,
      maxHops,
      timeoutMs,
      pingIntervalMs,
      probeMethod
    })
  }

  async function stopContinuousTrace(target: string): Promise<void> {
    await invoke('stop_continuous_trace', { target })
  }

  return {
    startContinuousTrace,
    stopContinuousTrace
  }
}

// 事件监听器
export function useContinuousTraceListener(
  onPathDiscovered: (data: PathDiscovered) => void,
  onHopResult: (data: ContinuousTraceHopResult) => void,
  onError?: (msg: string) => void,
  onStopped?: (target: string) => void
) {
  let unlistenPath: (() => void) | null = null
  let unlistenHop: (() => void) | null = null
  let unlistenError: (() => void) | null = null
  let unlistenStopped: (() => void) | null = null

  onMounted(async () => {
    unlistenPath = await listen<PathDiscovered>('continuous-trace-path-discovered', (event) => {
      onPathDiscovered(event.payload)
    })

    unlistenHop = await listen<ContinuousTraceHopResult>('continuous-trace-hop-result', (event) => {
      onHopResult(event.payload)
    })

    if (onError) {
      unlistenError = await listen<string>('continuous-trace-error', (event) => {
        onError(event.payload)
      })
    }

    if (onStopped) {
      unlistenStopped = await listen<string>('continuous-trace-stopped', (event) => {
        onStopped(event.payload)
      })
    }
  })

  onUnmounted(() => {
    unlistenPath?.()
    unlistenHop?.()
    unlistenError?.()
    unlistenStopped?.()
  })
}
