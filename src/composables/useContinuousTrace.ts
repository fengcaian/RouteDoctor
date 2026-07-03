import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { GeoInfo, PathChangeEvent } from '@/types'

// 持续路径监控的单跳结果（每一轮每一跳一条）
export interface ContinuousTraceHopResult {
  target: string
  hop_number: number
  /** 该轮该跳响应的 IP；超时或未探测到时为 null */
  hop_ip: string | null
  latency_ms: number | null
  is_timeout: boolean
  timestamp: number
  /** 轮次编号（1-based，每次完整 traceroute 递增） */
  seq: number
}

// 路径发现快照（首轮和后续每轮的路径都用这个结构）
export interface PathSnapshot {
  target: string
  hops: {
    hop_number: number
    ip: string | null
    hostname: string | null
  }[]
  /** 该快照对应的轮次编号 */
  round_seq: number
}

/** 首轮完成时 emit 的初始路径快照（与 PathSnapshot 结构相同，语义化别名） */
export type PathDiscovered = PathSnapshot

/** 后续每轮 emit 的路径快照，前端应做增量 merge（不是覆盖） */
export type PathUpdate = PathSnapshot

// 单跳 GeoIP 事件
export interface ContinuousTraceHopGeo {
  target: string
  hop_number: number
  ip: string
  geo: GeoInfo | null
  hostname: string | null
}

export function useContinuousTrace() {
  async function startContinuousTrace(
    target: string,
    maxHops: number = 30,
    timeoutMs: number = 3000,
    pingIntervalMs: number = 2000,
    probeMethod: string = 'icmp',
    persist: boolean = true,
    /** TCP 探测的目标端口，仅在 probeMethod='tcp' 时使用（默认 80，与 PingPlotter 一致） */
    tcpPort?: number
  ): Promise<void> {
    await invoke('start_continuous_trace', {
      target,
      maxHops,
      timeoutMs,
      pingIntervalMs,
      probeMethod,
      persist,
      tcpPort
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
  onStopped?: (target: string) => void,
  onHopGeo?: (data: ContinuousTraceHopGeo) => void,
  /** 兼容占位，当前后端不再 emit path-changed（用 onPathUpdate 替代）。保留参数向后兼容。 */
  onPathChanged?: (data: PathChangeEvent) => void,
  /** 后续每轮的路径快照（mtr 风格）；前端应做增量 merge */
  onPathUpdate?: (data: PathUpdate) => void
) {
  let unlistenPath: (() => void) | null = null
  let unlistenHop: (() => void) | null = null
  let unlistenError: (() => void) | null = null
  let unlistenStopped: (() => void) | null = null
  let unlistenGeo: (() => void) | null = null
  let unlistenChanged: (() => void) | null = null
  let unlistenUpdate: (() => void) | null = null

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

    if (onHopGeo) {
      unlistenGeo = await listen<ContinuousTraceHopGeo>('continuous-trace-hop-geo', (event) => {
        onHopGeo(event.payload)
      })
    }

    if (onPathChanged) {
      unlistenChanged = await listen<PathChangeEvent>('path-changed', (event) => {
        onPathChanged(event.payload)
      })
    }

    if (onPathUpdate) {
      unlistenUpdate = await listen<PathUpdate>('continuous-trace-path-update', (event) => {
        onPathUpdate(event.payload)
      })
    }
  })

  onUnmounted(() => {
    unlistenPath?.()
    unlistenHop?.()
    unlistenError?.()
    unlistenStopped?.()
    unlistenGeo?.()
    unlistenChanged?.()
    unlistenUpdate?.()
  })
}
