import { invoke } from '@tauri-apps/api/core'

/** 会话主表行（与 Rust 端 TraceSessionRow 对应） */
export interface TraceSessionRow {
  id: number
  target: string
  started_at: number
  ended_at: number | null
  ping_interval_ms: number
  timeout_ms: number
  probe_method: string
  /** 'running' | 'stopped' | 'crashed' */
  status: string
}

export interface TraceHopInfoRow {
  hop_number: number
  ip: string | null
  hostname: string | null
  geo_json: string | null
}

export interface TraceSampleRow {
  hop_number: number
  seq: number
  latency_ms: number | null
  is_timeout: boolean
  timestamp: number
}

/** 路径监控会话历史的查询封装 */
export function useTraceHistory() {
  async function listSessions(limit = 50): Promise<TraceSessionRow[]> {
    return await invoke<TraceSessionRow[]>('list_trace_sessions', { limit })
  }

  async function loadHops(sessionId: number): Promise<TraceHopInfoRow[]> {
    return await invoke<TraceHopInfoRow[]>('load_trace_hops', { sessionId })
  }

  /**
   * 加载会话样本。since 为可选时间戳过滤；不传时默认取最近 limit 条。
   * 注意：limit 要够大以覆盖整个会话。30 跳 × 0.5Hz × 24h ≈ 130 万条，超过单次拉取能力。
   * 实际使用时建议限制只拉最近几个小时。
   */
  async function loadSamples(
    sessionId: number,
    options: { since?: number; limit?: number } = {}
  ): Promise<TraceSampleRow[]> {
    return await invoke<TraceSampleRow[]>('load_trace_samples', {
      sessionId,
      since: options.since,
      limit: options.limit ?? 5000
    })
  }

  async function deleteSession(sessionId: number): Promise<void> {
    await invoke('delete_trace_session', { sessionId })
  }

  async function cleanupOldSessions(daysToKeep: number): Promise<void> {
    await invoke('cleanup_old_trace_sessions', { daysToKeep })
  }

  return {
    listSessions,
    loadHops,
    loadSamples,
    deleteSession,
    cleanupOldSessions
  }
}
