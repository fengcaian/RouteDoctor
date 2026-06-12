import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ContinuousTraceHopResult, PathDiscovered } from '@/composables/useContinuousTrace'
import type { GeoInfo } from '@/types'

export interface HopInfo {
  hop_number: number
  ip: string | null
  hostname: string | null
  geo?: GeoInfo | null
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
  samples: HopSample[]  // 时间序列数据（滑动窗口，用于绘图）

  // ===== 会话级累计统计 (PingPlotter Pro 风格) =====
  // 不受滑动窗口影响，反映从开始监控至今的真实情况
  cumulativeSent: number       // 累计发送数
  cumulativeLost: number       // 累计丢包数
  cumulativeLatencySum: number // 累计延迟总和（用于算 avg，避免存储所有点）
  cumulativeMin: number        // 会话最小延迟
  cumulativeMax: number        // 会话最大延迟
  cumulativeValidCount: number // 累计有效（非超时）样本数
}

const DEFAULT_MAX_SAMPLES_PER_HOP = 1800  // 默认上限：1 小时 @ 2s 间隔。实际值由 setMaxSamples 配置

export const useContinuousTraceStore = defineStore('continuousTrace', () => {
  // 状态
  const isRunning = ref(false)
  const target = ref('')
  const hops = ref<HopInfo[]>([])
  const hopHistories = ref<Map<number, HopHistory>>(new Map())
  const isDiscovering = ref(false)

  // 只读历史模式：true 时表示当前展示的是从数据库加载的旧会话，禁用启动/配置等操作
  const isHistoricalView = ref(false)
  const loadedSessionId = ref<number | null>(null)
  const loadedSessionStartedAt = ref<number | null>(null)
  const loadedSessionEndedAt = ref<number | null>(null)
  const loadedSessionStatus = ref<string | null>(null)
  const loadedProbeMethod = ref<string | null>(null)

  // 滑动窗口大小（仅影响绘图，不影响累计统计）
  // 由调用方根据 windowMinutes / pingIntervalMs 计算后通过 setMaxSamples 注入
  const maxSamplesPerHop = ref(DEFAULT_MAX_SAMPLES_PER_HOP)

  function setMaxSamples(n: number) {
    maxSamplesPerHop.value = Math.max(60, Math.floor(n))
  }

  // 设置路径发现结果
  // 行为：如果某跳 (ip 相同) 已经存在历史，保留其 samples 和累计统计；
  //      只有新跳或 ip 变化的跳才会重置数据。
  //      这样"停止 → 重新开始监控同一目标"时，旧数据不会被清空，
  //      只是 samples 会出现一段时间的空隙——和 PingPlotter 行为一致。
  function setPath(data: PathDiscovered) {
    target.value = data.target
    hops.value = data.hops.map(h => ({
      hop_number: h.hop_number,
      ip: h.ip,
      hostname: h.hostname
    }))

    const newHistories = new Map<number, HopHistory>()
    for (const hop of data.hops) {
      if (!hop.ip) continue
      const existing = hopHistories.value.get(hop.hop_number)
      // 复用条件：跳号相同 + IP 相同
      if (existing && existing.ip === hop.ip) {
        // 仅更新可能变化的元数据
        existing.hostname = hop.hostname ?? existing.hostname
        newHistories.set(hop.hop_number, existing)
      } else {
        // 新跳 / IP 变了：重新初始化
        newHistories.set(hop.hop_number, {
          hop_number: hop.hop_number,
          ip: hop.ip,
          hostname: hop.hostname,
          samples: [],
          cumulativeSent: 0,
          cumulativeLost: 0,
          cumulativeLatencySum: 0,
          cumulativeMin: Infinity,
          cumulativeMax: 0,
          cumulativeValidCount: 0
        })
      }
    }
    hopHistories.value = newHistories

    isDiscovering.value = false
  }

  // 添加单跳 Ping 结果
  function addHopResult(result: ContinuousTraceHopResult) {
    // 防御性：只要有 ping 结果到达，就肯定已经过了路径发现阶段。
    // 即使 path-discovered 事件丢失/竞态，这里也能兜底关闭 spinner。
    if (isDiscovering.value) {
      isDiscovering.value = false
    }
    const history = hopHistories.value.get(result.hop_number)
    if (history) {
      history.samples.push({
        timestamp: result.timestamp,
        seq: result.seq,
        latency_ms: result.latency_ms,
        is_timeout: result.is_timeout
      })

      // 限制最大采样数（仅滑动窗口，用于绘图）
      if (history.samples.length > maxSamplesPerHop.value) {
        history.samples.splice(0, history.samples.length - maxSamplesPerHop.value)
      }

      // 更新会话级累计统计（独立于滑动窗口，反映会话全程）
      history.cumulativeSent++
      if (result.is_timeout) {
        history.cumulativeLost++
      } else if (result.latency_ms != null) {
        history.cumulativeLatencySum += result.latency_ms
        history.cumulativeValidCount++
        if (result.latency_ms < history.cumulativeMin) history.cumulativeMin = result.latency_ms
        if (result.latency_ms > history.cumulativeMax) history.cumulativeMax = result.latency_ms
      }

      // 触发响应式更新
      hopHistories.value = new Map(hopHistories.value)
    }
  }

  // 获取指定跳的统计数据（基于会话累计，不受滑动窗口影响）
  function getHopStats(hopNumber: number) {
    const history = hopHistories.value.get(hopNumber)
    if (!history || history.cumulativeSent === 0) {
      return { avg: 0, min: 0, max: 0, loss: 0, count: 0 }
    }

    const validCount = history.cumulativeValidCount
    return {
      avg: validCount > 0 ? history.cumulativeLatencySum / validCount : 0,
      min: validCount > 0 ? history.cumulativeMin : 0,
      max: history.cumulativeMax,
      loss: (history.cumulativeLost / history.cumulativeSent) * 100,
      count: history.cumulativeSent
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
    // 切换到不同目标时清空历史并显示 spinner（路径未知，需要 traceroute）；
    // 同一目标重启时保留数据，且不显示 spinner——
    // 因为旧路径仍可见，发现新路径只是后台行为，不应让用户感觉"在等待"
    if (target.value !== targetAddr) {
      hops.value = []
      hopHistories.value = new Map()
      isDiscovering.value = true
    } else {
      isDiscovering.value = false
    }
    target.value = targetAddr
  }

  function stopMonitoring() {
    isRunning.value = false
    isDiscovering.value = false
  }

  /** Update a hop with reverse-DNS hostname and/or GeoIP info (delivered asynchronously by the backend). */
  function updateHopGeo(hopNumber: number, geo: GeoInfo | null, hostname?: string | null) {
    const hopIdx = hops.value.findIndex(h => h.hop_number === hopNumber)
    if (hopIdx >= 0) {
      const next = [...hops.value]
      next[hopIdx] = {
        ...next[hopIdx],
        geo: geo ?? next[hopIdx].geo,
        hostname: hostname ?? next[hopIdx].hostname,
      }
      hops.value = next
    }

    const hist = hopHistories.value.get(hopNumber)
    if (hist && hostname && !hist.hostname) {
      hist.hostname = hostname
      hopHistories.value = new Map(hopHistories.value)
    }
  }

  function resetStore() {
    isRunning.value = false
    isDiscovering.value = false
    target.value = ''
    hops.value = []
    hopHistories.value = new Map()
    isHistoricalView.value = false
    loadedSessionId.value = null
    loadedSessionStartedAt.value = null
    loadedSessionEndedAt.value = null
    loadedSessionStatus.value = null
    loadedProbeMethod.value = null
  }

  /**
   * 加载历史会话到 store（只读模式）
   * 把数据库取出来的样本回灌到 hopHistories 里，复用现有图表/统计计算。
   * 注意：滑动窗口不再生效——历史模式下要看完整数据，所以需要先扩大 maxSamplesPerHop。
   */
  function loadHistoricalSession(
    session: {
      id: number
      target: string
      started_at: number
      ended_at: number | null
      status: string
      probe_method?: string
    },
    hopRows: Array<{
      hop_number: number
      ip: string | null
      hostname: string | null
      geo_json: string | null
    }>,
    samples: Array<{
      hop_number: number
      seq: number
      latency_ms: number | null
      is_timeout: boolean
      timestamp: number
    }>
  ) {
    // 重置当前状态（与实时监控完全隔离）
    resetStore()

    isRunning.value = false
    isDiscovering.value = false
    loadedSessionId.value = session.id
    loadedSessionStartedAt.value = session.started_at
    loadedSessionEndedAt.value = session.ended_at
    loadedSessionStatus.value = session.status
    loadedProbeMethod.value = session.probe_method ?? null
    target.value = session.target

    // 历史模式下扩容滑动窗口，确保所有样本都能呈现
    setMaxSamples(Math.max(samples.length, 60))

    // 填充 hops 列表
    hops.value = hopRows.map(h => {
      let geo: GeoInfo | null = null
      if (h.geo_json) {
        try { geo = JSON.parse(h.geo_json) as GeoInfo } catch { /* ignore */ }
      }
      return {
        hop_number: h.hop_number,
        ip: h.ip,
        hostname: h.hostname,
        geo
      }
    })

    // 初始化 hopHistories
    const histories = new Map<number, HopHistory>()
    for (const h of hopRows) {
      if (h.ip) {
        histories.set(h.hop_number, {
          hop_number: h.hop_number,
          ip: h.ip,
          hostname: h.hostname,
          samples: [],
          cumulativeSent: 0,
          cumulativeLost: 0,
          cumulativeLatencySum: 0,
          cumulativeMin: Infinity,
          cumulativeMax: 0,
          cumulativeValidCount: 0
        })
      }
    }

    // 回灌样本：保持 timestamp 升序（后端已 ORDER BY timestamp ASC）
    for (const s of samples) {
      const hist = histories.get(s.hop_number)
      if (!hist) continue
      hist.samples.push({
        timestamp: s.timestamp,
        seq: s.seq,
        latency_ms: s.latency_ms,
        is_timeout: s.is_timeout
      })
      hist.cumulativeSent++
      if (s.is_timeout) {
        hist.cumulativeLost++
      } else if (s.latency_ms != null) {
        hist.cumulativeLatencySum += s.latency_ms
        hist.cumulativeValidCount++
        if (s.latency_ms < hist.cumulativeMin) hist.cumulativeMin = s.latency_ms
        if (s.latency_ms > hist.cumulativeMax) hist.cumulativeMax = s.latency_ms
      }
    }

    hopHistories.value = histories

    // 数据全部就位后再切换历史模式标志位，下游 watcher 这时能拿到完整 hopHistories
    // 进而把 X 轴对齐到数据范围。如果先翻 isHistoricalView 再填数据,watcher 触发时
    // hopHistories 还是空的,会导致视口对齐失败,RAF tick 把 viewEnd 推到当前时间。
    isHistoricalView.value = true
  }

  /** 退出历史只读模式，回到空白状态准备新的实时监控 */
  function exitHistoricalView() {
    resetStore()
  }

  return {
    isRunning,
    target,
    hops,
    hopHistories,
    isDiscovering,
    isHistoricalView,
    loadedSessionId,
    loadedSessionStartedAt,
    loadedSessionEndedAt,
    loadedSessionStatus,
    loadedProbeMethod,
    maxSamplesPerHop,
    setMaxSamples,
    setPath,
    addHopResult,
    getHopStats,
    getHeatmapData,
    startMonitoring,
    stopMonitoring,
    updateHopGeo,
    resetStore,
    loadHistoricalSession,
    exitHistoricalView
  }
})
