import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ContinuousTraceHopResult, PathDiscovered, PathUpdate } from '@/composables/useContinuousTrace'
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
  /** 该样本对应的 IP；老会话（迁移前）或超时样本为 null */
  ip: string | null
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

  // 当前实时监控使用的 probe method（icmp/udp/tcp）。用于识别"切换探测方式"这个事件——
  // ICMP 路径通常单 IP、UDP/TCP 因 ECMP 分岔常多 IP，数据模型差异较大，切换后应清空重来。
  const currentProbeMethod = ref<string | null>(null)

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
    // 竞态防御：停止/历史模式下丢弃延迟到达的首轮事件
    if (!isRunning.value || isHistoricalView.value) return

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
    // 停止/历史模式下丢弃延迟到达的事件：
    // 后端 stop 到 emit 之间存在几十~几百毫秒的传输延迟；如果用户已经停止甚至
    // 点了"清除数据"，此时接受新数据会导致 UI 数据"复活"。
    if (!isRunning.value || isHistoricalView.value) return

    // 防御性：只要有 ping 结果到达，就肯定已经过了路径发现阶段。
    // 即使 path-discovered 事件丢失/竞态，这里也能兜底关闭 spinner。
    if (isDiscovering.value) {
      isDiscovering.value = false
    }

    // mtr 风格：结果中的 hop_number 可能是当前 hops 里还没见过的新跳
    // （初始超时后续响应的情况）。若前端还没这一行且这轮有 IP，直接补上。
    ensureHopRow(result.hop_number, result.hop_ip)

    const history = hopHistories.value.get(result.hop_number)
    if (history) {
      history.samples.push({
        timestamp: result.timestamp,
        seq: result.seq,
        ip: result.hop_ip,
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

  /**
   * mtr 风格 merge：确保某跳存在于 hops 列表中，必要时创建 HopHistory。
   * - hops 里没有该跳 → 追加一行（IP 可能为 null，仅显示 * * *）
   * - hops 里有该跳但 IP 从 null → 有值 → 补上 IP + 创建 HopHistory
   * - hops 里有该跳且 IP 变了（ECMP/路径切换）→ 更新 IP，HopHistory 保留但换 IP 记录
   *   （phase 1 简化：不重置累计统计，把新 IP 视为"同一跳的观察 IP 变化"）
   * - IP 相同 → 无操作
   */
  function ensureHopRow(hopNumber: number, ip: string | null) {
    const existing = hops.value.find(h => h.hop_number === hopNumber)
    if (!existing) {
      hops.value = [
        ...hops.value,
        { hop_number: hopNumber, ip, hostname: null }
      ].sort((a, b) => a.hop_number - b.hop_number)
      if (ip) {
        hopHistories.value.set(hopNumber, createEmptyHistory(hopNumber, ip, null))
      }
      return
    }
    if (!existing.ip && ip) {
      // 之前超时无 IP，这一轮补上 IP
      existing.ip = ip
      hops.value = [...hops.value]
      if (!hopHistories.value.has(hopNumber)) {
        hopHistories.value.set(hopNumber, createEmptyHistory(hopNumber, ip, existing.hostname))
      }
      return
    }
    if (existing.ip && ip && existing.ip !== ip) {
      // IP 变化：更新显示的主 IP，历史记录里换 IP 但保留累计（phase 1 简化）
      existing.ip = ip
      hops.value = [...hops.value]
      const hist = hopHistories.value.get(hopNumber)
      if (hist) {
        hist.ip = ip
      } else {
        hopHistories.value.set(hopNumber, createEmptyHistory(hopNumber, ip, existing.hostname))
      }
    }
  }

  /** 处理后端 path-update 事件：批量 merge 本轮完整的路径快照 */
  function mergePath(data: PathUpdate) {
    // 与 addHopResult 同源的竞态防御：停止/历史模式下丢弃延迟事件
    if (!isRunning.value || isHistoricalView.value) return

    for (const h of data.hops) {
      ensureHopRow(h.hop_number, h.ip)
      if (h.hostname) {
        const existing = hops.value.find(x => x.hop_number === h.hop_number)
        if (existing && !existing.hostname) {
          existing.hostname = h.hostname
        }
        const hist = hopHistories.value.get(h.hop_number)
        if (hist && !hist.hostname) {
          hist.hostname = h.hostname
        }
      }
    }
    // 触发响应式
    hops.value = [...hops.value]
    hopHistories.value = new Map(hopHistories.value)
  }

  function createEmptyHistory(hopNumber: number, ip: string, hostname: string | null): HopHistory {
    return {
      hop_number: hopNumber,
      ip,
      hostname,
      samples: [],
      cumulativeSent: 0,
      cumulativeLost: 0,
      cumulativeLatencySum: 0,
      cumulativeMin: Infinity,
      cumulativeMax: 0,
      cumulativeValidCount: 0
    }
  }

  /**
   * 每个 IP 的分组统计。
   * PingPlotter Pro 风格：某跳因 ECMP / 路径抖动响应了多个 IP 时，
   * 用户可以看到每个 IP 独立的 avg/min/max/loss/count。
   */
  interface IpBreakdown {
    ip: string
    count: number
    lossCount: number
    latencySum: number
    validCount: number
    min: number
    max: number
    avg: number
    loss: number
    /** 该 IP 首次出现的样本 seq，用于排序（先出现的 IP 排前面） */
    firstSeq: number
  }

  /**
   * 按 IP 聚合某跳的 samples，返回每个 IP 的独立统计。
   * 只返回 ip !== null 的分组（超时样本不算独立 IP）。
   * 结果按"首次出现的 seq"升序排列，符合"发现顺序"直觉。
   */
  function getHopIpBreakdown(hopNumber: number): IpBreakdown[] {
    const history = hopHistories.value.get(hopNumber)
    if (!history || history.samples.length === 0) return []

    const groups = new Map<string, IpBreakdown>()
    for (const s of history.samples) {
      if (!s.ip) continue
      let g = groups.get(s.ip)
      if (!g) {
        g = {
          ip: s.ip,
          count: 0,
          lossCount: 0,
          latencySum: 0,
          validCount: 0,
          min: Infinity,
          max: 0,
          avg: 0,
          loss: 0,
          firstSeq: s.seq
        }
        groups.set(s.ip, g)
      }
      g.count++
      if (s.is_timeout) {
        g.lossCount++
      } else if (s.latency_ms != null) {
        g.latencySum += s.latency_ms
        g.validCount++
        if (s.latency_ms < g.min) g.min = s.latency_ms
        if (s.latency_ms > g.max) g.max = s.latency_ms
      }
    }

    // 计算派生统计
    const result: IpBreakdown[] = []
    for (const g of groups.values()) {
      g.avg = g.validCount > 0 ? g.latencySum / g.validCount : 0
      g.loss = g.count > 0 ? (g.lossCount / g.count) * 100 : 0
      if (g.min === Infinity) g.min = 0
      result.push(g)
    }
    // 按样本数降序（PingPlotter 一致：最主要的 IP 排前面）
    // 同样多样本时用 firstSeq 保持稳定的显示顺序
    result.sort((a, b) => b.count - a.count || a.firstSeq - b.firstSeq)
    return result
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

  function startMonitoring(targetAddr: string, probeMethod?: string) {
    isRunning.value = true
    // 清空条件：
    // 1. 目标变化（路径完全不同，必须重来）
    // 2. probe method 变化（ICMP↔UDP/TCP 的路径模型不同——ECMP 表现差异大——
    //    保留旧数据会造成 hop 混合，展开的多 IP 与新方法的观察冲突）
    const targetChanged = target.value !== targetAddr
    const methodChanged =
      !!probeMethod &&
      currentProbeMethod.value !== null &&
      currentProbeMethod.value !== probeMethod

    if (targetChanged || methodChanged) {
      hops.value = []
      hopHistories.value = new Map()
      isDiscovering.value = true
    } else {
      // 同目标同方法重启：保留数据（旧路径仍可见，发现新路径是后台行为）
      isDiscovering.value = false
    }
    target.value = targetAddr
    if (probeMethod) currentProbeMethod.value = probeMethod
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
    currentProbeMethod.value = null
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
        // 老会话（迁移前）ip 列可能不存在或为 null，兼容
        ip: (s as { ip?: string | null }).ip ?? null,
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
    currentProbeMethod,
    isHistoricalView,
    loadedSessionId,
    loadedSessionStartedAt,
    loadedSessionEndedAt,
    loadedSessionStatus,
    loadedProbeMethod,
    maxSamplesPerHop,
    setMaxSamples,
    setPath,
    mergePath,
    addHopResult,
    getHopStats,
    getHopIpBreakdown,
    getHeatmapData,
    startMonitoring,
    stopMonitoring,
    updateHopGeo,
    resetStore,
    loadHistoricalSession,
    exitHistoricalView
  }
})
