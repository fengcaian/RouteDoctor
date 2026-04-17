import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { TracerouteResult, HopResult, ProbeMethod } from '@/types'

export const useTracerouteStore = defineStore('traceroute', () => {
  // State - 使用 ref 并在更新时创建新 Map 以触发响应性
  const results = ref(new Map<string, TracerouteResult>())
  const runningTargets = ref(new Set<string>())
  const currentHop = ref(new Map<string, number>())

  // Actions
  function startTrace(target: string, probeMethod: ProbeMethod = 'icmp') {
    // 创建新的 Set/Map 以触发响应性
    const newRunning = new Set(runningTargets.value)
    newRunning.add(target)
    runningTargets.value = newRunning

    const newCurrentHop = new Map(currentHop.value)
    newCurrentHop.set(target, 0)
    currentHop.value = newCurrentHop

    const newResults = new Map(results.value)
    newResults.set(target, {
      target,
      target_ip: '',
      hops: [],
      completed: false,
      start_time: Date.now(),
      end_time: null,
      probe_method: probeMethod
    })
    results.value = newResults
  }

  function updateResult(target: string, data: Partial<TracerouteResult>) {
    const existing = results.value.get(target)
    if (existing) {
      const newResults = new Map(results.value)
      newResults.set(target, { ...existing, ...data })
      results.value = newResults
    }
  }

  function addHop(target: string, hop: HopResult) {
    const result = results.value.get(target)
    if (result) {
      const newHops = [...result.hops]
      const existingIndex = newHops.findIndex(h => h.hop_number === hop.hop_number)
      if (existingIndex >= 0) {
        newHops[existingIndex] = hop
      } else {
        newHops.push(hop)
      }

      // 更新当前跳数
      const newCurrentHop = new Map(currentHop.value)
      newCurrentHop.set(target, hop.hop_number)
      currentHop.value = newCurrentHop

      // 更新结果
      const newResults = new Map(results.value)
      newResults.set(target, { ...result, hops: newHops })
      results.value = newResults
    }
  }

  function completeTrace(target: string) {
    const newRunning = new Set(runningTargets.value)
    newRunning.delete(target)
    runningTargets.value = newRunning

    const newCurrentHop = new Map(currentHop.value)
    newCurrentHop.delete(target)
    currentHop.value = newCurrentHop

    const result = results.value.get(target)
    if (result) {
      const newResults = new Map(results.value)
      newResults.set(target, { ...result, completed: true, end_time: Date.now() })
      results.value = newResults
    }
  }

  function isRunning(target: string): boolean {
    return runningTargets.value.has(target)
  }

  function getCurrentHop(target: string): number {
    return currentHop.value.get(target) || 0
  }

  function getResult(target: string): TracerouteResult | undefined {
    return results.value.get(target)
  }

  function clearResult(target: string) {
    const newResults = new Map(results.value)
    newResults.delete(target)
    results.value = newResults

    const newCurrentHop = new Map(currentHop.value)
    newCurrentHop.delete(target)
    currentHop.value = newCurrentHop
  }

  function resetStore() {
    results.value.clear()
    runningTargets.value.clear()
    currentHop.value.clear()
  }

  return {
    results,
    runningTargets,
    currentHop,
    startTrace,
    updateResult,
    addHop,
    completeTrace,
    isRunning,
    getCurrentHop,
    getResult,
    clearResult,
    resetStore
  }
})