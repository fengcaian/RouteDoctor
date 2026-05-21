import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ContinuousTraceHopResult, PathDiscovered } from '@/composables/useContinuousTrace'

export interface HopInfo {
  hop_number: number
  ip: string | null
  hostname: string | null
}

export interface HopSample {
  timestamp: number
  seq: number
  latency_ms: number | null
  is_timeout: boolean
}

// 每一跳的历史数据
export interface HopHistory {
  hop_number: number
  ip: string
  hostname: string | null
  samples: HopSample[]  // 时间序列数据
}

const MAX_SAMPLES_PER_HOP = 300  // 每跳最多保留 300 个采样点（10 分钟 @ 2s 间隔）

export const useContinuousTraceStore = defineStore('continuousTrace', () => {
  // 状态
  const isRunning = ref(false)
  const target = ref('')
  const hops = ref<HopInfo[]>([])
  const hopHistories = ref<Map<number, HopHistory>>(new Map())
  const isDiscovering = ref(false)

  // 设置路径发现结果
  function setPath(data: PathDiscovered) {
    target.value = data.target
    hops.value = data.hops.map(h => ({
      hop_number: h.hop_number,
      ip: h.ip,
      hostname: h.hostname
    }))

    // 初始化每一跳的历史数据
    hopHistories.value = new Map()
    for (const hop of data.hops) {
      if (hop.ip) {
        hopHistories.value.set(hop.hop_number, {
          hop_number: hop.hop_number,
          ip: hop.ip,
          hostname: hop.hostname,
          samples: []
        })
      }
    }

    isDiscovering.value = false
  }

  // 添加单跳 Ping 结果
  function addHopResult(result: ContinuousTraceHopResult) {
    const history = hopHistories.value.get(result.hop_number)
    if (history) {
      history.samples.push({
        timestamp: result.timestamp,
        seq: result.seq,
        latency_ms: result.latency_ms,
        is_timeout: result.is_timeout
      })

      // 限制最大采样数
      if (history.samples.length > MAX_SAMPLES_PER_HOP) {
        history.samples.shift()
      }

      // 触发响应式更新
      hopHistories.value = new Map(hopHistories.value)
    }
  }

  // 获取指定跳的统计数据
  function getHopStats(hopNumber: number) {
    const history = hopHistories.value.get(hopNumber)
    if (!history || history.samples.length === 0) {
      return { avg: 0, min: 0, max: 0, loss: 0, count: 0 }
    }

    const samples = history.samples
    const validLatencies = samples
      .filter(s => !s.is_timeout && s.latency_ms !== null)
      .map(s => s.latency_ms as number)

    const timeouts = samples.filter(s => s.is_timeout).length

    return {
      avg: validLatencies.length > 0
        ? validLatencies.reduce((a, b) => a + b, 0) / validLatencies.length
        : 0,
      min: validLatencies.length > 0 ? Math.min(...validLatencies) : 0,
      max: validLatencies.length > 0 ? Math.max(...validLatencies) : 0,
      loss: (timeouts / samples.length) * 100,
      count: samples.length
    }
  }

  // 获取热力图数据（二维数组：[时间索引][跳数索引] = 延迟值）
  function getHeatmapData(): { times: string[], hops: string[], data: [number, number, number | null][] } {
    const hopList = Array.from(hopHistories.value.values()).sort((a, b) => a.hop_number - b.hop_number)
    if (hopList.length === 0) return { times: [], hops: [], data: [] }

    // 获取所有唯一的时间点（使用 seq 作为索引）
    const allSeqs = new Set<number>()
    for (const hop of hopList) {
      for (const sample of hop.samples) {
        allSeqs.add(sample.seq)
      }
    }
    const sortedSeqs = Array.from(allSeqs).sort((a, b) => a - b)

    // 只取最近 60 个采样点
    const recentSeqs = sortedSeqs.slice(-60)

    // 构建时间标签
    const times: string[] = recentSeqs.map(seq => {
      const sample = hopList[0]?.samples.find(s => s.seq === seq)
      if (sample) {
        const date = new Date(sample.timestamp)
        return `${date.getHours().toString().padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}:${date.getSeconds().toString().padStart(2, '0')}`
      }
      return ''
    })

    // 构建跳标签
    const hopLabels: string[] = hopList.map(h => `${h.hop_number}. ${h.ip}`)

    // 构建热力图数据 [timeIndex, hopIndex, value]
    const data: [number, number, number | null][] = []
    for (let hopIdx = 0; hopIdx < hopList.length; hopIdx++) {
      const hop = hopList[hopIdx]
      for (let timeIdx = 0; timeIdx < recentSeqs.length; timeIdx++) {
        const seq = recentSeqs[timeIdx]
        const sample = hop.samples.find(s => s.seq === seq)
        const value = sample
          ? (sample.is_timeout ? null : sample.latency_ms)
          : null
        data.push([timeIdx, hopIdx, value])
      }
    }

    return { times, hops: hopLabels, data }
  }

  function startMonitoring(targetAddr: string) {
    isRunning.value = true
    isDiscovering.value = true
    target.value = targetAddr
    hops.value = []
    hopHistories.value = new Map()
  }

  function stopMonitoring() {
    isRunning.value = false
    isDiscovering.value = false
  }

  function resetStore() {
    isRunning.value = false
    isDiscovering.value = false
    target.value = ''
    hops.value = []
    hopHistories.value = new Map()
  }

  return {
    isRunning,
    target,
    hops,
    hopHistories,
    isDiscovering,
    setPath,
    addHopResult,
    getHopStats,
    getHeatmapData,
    startMonitoring,
    stopMonitoring,
    resetStore
  }
})
