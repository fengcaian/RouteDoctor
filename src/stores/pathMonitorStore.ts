import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * 单跳的持续监控数据点
 */
export interface HopSample {
  timestamp: number
  latency: number | null  // null = 超时
}

/**
 * 单跳的完整监控状态
 */
export interface MonitoredHop {
  hopNumber: number
  ip: string | null
  hostname: string | null
  samples: HopSample[]       // 时间序列数据（最近 N 个采样）
  currentLatency: number | null
  avgLatency: number | null
  maxLatency: number | null
  minLatency: number | null
  lossRate: number            // 0-100
  totalSent: number
  totalLost: number
}

/**
 * 路径监控会话
 */
export interface PathMonitorSession {
  target: string
  targetIp: string
  hops: MonitoredHop[]
  isRunning: boolean
  startTime: number
  intervalMs: number
  totalSamples: number        // 总采样次数
}

// 最大保留采样数（每跳）
const MAX_SAMPLES_PER_HOP = 300  // 5 分钟 @ 1s 间隔

export const usePathMonitorStore = defineStore('pathMonitor', () => {
  const session = ref<PathMonitorSession | null>(null)

  /**
   * 初始化会话（Traceroute 完成后调用）
   */
  function initSession(target: string, targetIp: string, hops: { hopNumber: number, ip: string | null, hostname: string | null }[], intervalMs: number) {
    session.value = {
      target,
      targetIp,
      hops: hops.map(h => ({
        hopNumber: h.hopNumber,
        ip: h.ip,
        hostname: h.hostname,
        samples: [],
        currentLatency: null,
        avgLatency: null,
        maxLatency: null,
        minLatency: null,
        lossRate: 0,
        totalSent: 0,
        totalLost: 0
      })),
      isRunning: true,
      startTime: Date.now(),
      intervalMs,
      totalSamples: 0
    }
  }

  /**
   * 添加一轮采样结果（所有跳的结果同时到达）
   */
  function addSampleRound(results: { hopNumber: number, latency: number | null }[]) {
    if (!session.value) return

    const timestamp = Date.now()
    session.value.totalSamples++

    for (const result of results) {
      const hop = session.value.hops.find(h => h.hopNumber === result.hopNumber)
      if (!hop) continue

      // 添加采样
      hop.samples.push({ timestamp, latency: result.latency })
      if (hop.samples.length > MAX_SAMPLES_PER_HOP) {
        hop.samples.shift()
      }

      // 更新统计
      hop.totalSent++
      if (result.latency === null) {
        hop.totalLost++
      }

      hop.currentLatency = result.latency
      hop.lossRate = (hop.totalLost / hop.totalSent) * 100

      // 计算 min/max/avg（只基于有效值）
      const validLatencies = hop.samples
        .map(s => s.latency)
        .filter((l): l is number => l !== null)

      if (validLatencies.length > 0) {
        hop.avgLatency = validLatencies.reduce((a, b) => a + b, 0) / validLatencies.length
        hop.minLatency = Math.min(...validLatencies)
        hop.maxLatency = Math.max(...validLatencies)
      }
    }
  }

  /**
   * 停止监控
   */
  function stopSession() {
    if (session.value) {
      session.value.isRunning = false
    }
  }

  /**
   * 清除会话
   */
  function clearSession() {
    session.value = null
  }

  return {
    session,
    initSession,
    addSampleRound,
    stopSession,
    clearSession
  }
})
