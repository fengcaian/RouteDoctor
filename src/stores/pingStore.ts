import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { PingResult, PingStatistics, TargetConfig } from '@/types'

export const usePingStore = defineStore('ping', () => {
  // State
  const targets = ref<Map<string, TargetConfig>>(new Map())
  const results = ref<Map<string, PingResult[]>>(new Map())
  const statistics = ref<Map<string, PingStatistics>>(new Map())
  const runningTargets = ref<Set<string>>(new Set())

  // Actions
  function addTarget(config: TargetConfig) {
    targets.value.set(config.target, config)
    results.value.set(config.target, [])
    statistics.value.set(config.target, createEmptyStats())
  }

  function removeTarget(target: string) {
    targets.value.delete(target)
    results.value.delete(target)
    statistics.value.delete(target)
    runningTargets.value.delete(target)
  }

  function addResult(result: PingResult) {
    const targetResults = results.value.get(result.target) || []
    targetResults.push(result)
    // Keep only last 1000 results per target
    if (targetResults.length > 1000) {
      targetResults.shift()
    }
    // 直接修改原数组，避免不必要的复制
    results.value.set(result.target, targetResults)
    updateStatistics(result.target)

    // 调试：定期打印内存状态
    if (targetResults.length % 500 === 0) {
      console.log(`[PingStore] Target ${result.target}: ${targetResults.length} results in memory`)
    }
  }

  function updateStatistics(target: string) {
    const targetResults = results.value.get(target) || []
    const stats = calculateStats(targetResults)
    statistics.value.set(target, stats)
  }

  function setRunning(target: string, running: boolean) {
    if (running) {
      runningTargets.value.add(target)
    } else {
      runningTargets.value.delete(target)
    }
  }

  function clearResults(target: string) {
    // 创建新数组以触发响应式更新
    results.value.set(target, [])
    // 重新设置整个 Map 以确保响应式更新
    results.value = new Map(results.value)
    statistics.value.set(target, createEmptyStats())
    statistics.value = new Map(statistics.value)
  }

  function isRunning(target: string): boolean {
    return runningTargets.value.has(target)
  }

  function getResults(target: string): PingResult[] {
    return results.value.get(target) || []
  }

  function getStatistics(target: string): PingStatistics {
    return statistics.value.get(target) || createEmptyStats()
  }

  function getConfig(target: string): TargetConfig | undefined {
    return targets.value.get(target)
  }

  // 获取指定目标的最新一条 Ping 结果（用于 TabBar 显示延迟值和超时状态）
  function getLatestResult(target: string): PingResult | undefined {
    const targetResults = results.value.get(target)
    if (!targetResults || targetResults.length === 0) return undefined
    return targetResults[targetResults.length - 1]
  }

  function resetStore() {
    targets.value.clear()
    results.value.clear()
    statistics.value.clear()
    runningTargets.value.clear()
  }

  return {
    targets,
    results,
    statistics,
    runningTargets,
    addTarget,
    removeTarget,
    addResult,
    setRunning,
    clearResults,
    isRunning,
    getResults,
    getStatistics,
    getConfig,
    getLatestResult,
    resetStore
  }
})

// Helper functions
function createEmptyStats(): PingStatistics {
  return {
    sent: 0,
    received: 0,
    lost: 0,
    loss_rate: 0,
    min_ms: 0,
    max_ms: 0,
    avg_ms: 0,
    jitter_ms: 0,
    std_dev_ms: 0
  }
}

function calculateStats(results: PingResult[]): PingStatistics {
  if (results.length === 0) {
    return createEmptyStats()
  }

  const sent = results.length
  const timeouts = results.filter(r => r.is_timeout).length
  const received = sent - timeouts
  const latencies = results
    .filter(r => !r.is_timeout && r.latency_ms !== null)
    .map(r => r.latency_ms as number)

  const loss_rate = sent > 0 ? (timeouts / sent) * 100 : 0

  let min_ms = 0
  let max_ms = 0
  let avg_ms = 0
  let std_dev_ms = 0

  if (latencies.length > 0) {
    min_ms = Math.min(...latencies)
    max_ms = Math.max(...latencies)
    avg_ms = latencies.reduce((a, b) => a + b, 0) / latencies.length

    // Calculate standard deviation
    const squaredDiffs = latencies.map(l => Math.pow(l - avg_ms, 2))
    std_dev_ms = Math.sqrt(squaredDiffs.reduce((a, b) => a + b, 0) / latencies.length)
  }

  // Calculate jitter (average difference between consecutive latencies)
  let jitter_ms = 0
  if (latencies.length > 1) {
    let totalJitter = 0
    for (let i = 1; i < latencies.length; i++) {
      totalJitter += Math.abs(latencies[i] - latencies[i - 1])
    }
    jitter_ms = totalJitter / (latencies.length - 1)
  }

  return {
    sent,
    received,
    lost: timeouts,
    loss_rate: Math.round(loss_rate * 100) / 100,
    min_ms: Math.round(min_ms * 100) / 100,
    max_ms: Math.round(max_ms * 100) / 100,
    avg_ms: Math.round(avg_ms * 100) / 100,
    jitter_ms: Math.round(jitter_ms * 100) / 100,
    std_dev_ms: Math.round(std_dev_ms * 100) / 100
  }
}