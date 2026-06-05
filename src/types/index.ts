// Ping result for a single ping
export interface PingResult {
  seq: number
  target: string
  ip: string
  latency_ms: number | null  // null = timeout
  is_timeout: boolean
  timestamp: number
}

// Ping statistics
export interface PingStatistics {
  sent: number
  received: number
  lost: number
  loss_rate: number      // percentage (0-100)
  min_ms: number
  max_ms: number
  avg_ms: number
  jitter_ms: number      // average jitter
  std_dev_ms: number     // standard deviation
}

// Ping 标签页（多目标同时 Ping）
export interface PingTab {
  id: string          // 唯一标识符（crypto.randomUUID() 生成）
  target: string      // 目标地址（IP 或域名），新建时为空字符串
}

// Traceroute probe method
export type ProbeMethod = 'icmp' | 'udp' | 'tcp'

// Probe method descriptions - 使用 i18n key 引用
export const PROBE_METHOD_INFO: Record<ProbeMethod, { name: string; descKey: string; prosKeys: string[]; consKeys: string[] }> = {
  icmp: {
    name: 'ICMP',
    descKey: 'traceroute.probe.icmp.desc',
    prosKeys: ['traceroute.probe.icmp.pro1', 'traceroute.probe.icmp.pro2', 'traceroute.probe.icmp.pro3'],
    consKeys: ['traceroute.probe.icmp.con1', 'traceroute.probe.icmp.con2']
  },
  udp: {
    name: 'UDP',
    descKey: 'traceroute.probe.udp.desc',
    prosKeys: ['traceroute.probe.udp.pro1', 'traceroute.probe.udp.pro2', 'traceroute.probe.udp.pro3'],
    consKeys: ['traceroute.probe.udp.con1', 'traceroute.probe.udp.con2']
  },
  tcp: {
    name: 'TCP',
    descKey: 'traceroute.probe.tcp.desc',
    prosKeys: ['traceroute.probe.tcp.pro1', 'traceroute.probe.tcp.pro2'],
    consKeys: ['traceroute.probe.tcp.con1', 'traceroute.probe.tcp.con2', 'traceroute.probe.tcp.con3']
  }
}

// Traceroute result
export interface TracerouteResult {
  target: string
  target_ip: string
  hops: HopResult[]
  completed: boolean
  start_time: number
  end_time: number | null
  probe_method: ProbeMethod
}

// Single hop in traceroute
export interface HopResult {
  hop_number: number
  ip: string | null
  hostname: string | null
  latencies: (number | null)[]    // multiple probe latencies
  avg_latency: number | null
  packet_loss: number  // percentage
  geo?: GeoInfo | null  // populated asynchronously after the hop is emitted
}

// GeoIP information for an IP address
export interface GeoInfo {
  ip: string
  country: string | null
  country_code: string | null
  region: string | null
  city: string | null
  isp: string | null
  org: string | null
  asn: string | null
  as_name: string | null
  lat: number | null
  lon: number | null
}

// Bandwidth test result
export interface BandwidthResult {
  download_speed_mbps: number
  upload_speed_mbps: number
  latency_ms: number
  server: string
  timestamp: number
}

// Bandwidth test progress
export interface BandwidthProgress {
  phase: 'download' | 'upload' | 'idle'
  progress: number      // 0-100
  current_speed_mbps: number
  bytes_transferred: number
}

// History record
export interface HistoryRecord {
  id: number
  target: string
  test_type: 'ping' | 'traceroute' | 'bandwidth'
  start_time: number
  end_time: number
  data: string  // JSON string of result data
}

// Target configuration
export interface TargetConfig {
  target: string
  interval_ms: number
  timeout_ms: number
  count: number | null  // null = infinite
  packet_size: number
}

// Application settings
export interface AppSettings {
  theme: 'light' | 'dark' | 'system'
  maxHistoryDays: number
  defaultPingInterval: number
  defaultPingTimeout: number
  defaultTracerouteMaxHops: number
  minimizeToTray: boolean
  autostart: boolean
  /** 路径监控折线图保留多少分钟的数据用于绘制（滑动窗口） */
  traceWindowMinutes: number
  /** 是否启用持久化（实时落盘 + 会话恢复） */
  tracePersistEnabled: boolean
}

// Default settings
export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'dark',
  maxHistoryDays: 30,
  defaultPingInterval: 1000,
  defaultPingTimeout: 3000,
  defaultTracerouteMaxHops: 30,
  minimizeToTray: true,
  autostart: false,
  traceWindowMinutes: 60,
  tracePersistEnabled: true
}

// DNS 查询相关类型
export interface DnsRecord {
  record_type: string
  value: string
  ttl: number
}

export interface DnsQueryResult {
  domain: string
  records: DnsRecord[]
  query_time_ms: number
}

// 网络信息类型
export interface NetworkInterface {
  name: string
  ip: string
  interface_type: string
}

export interface NetworkInfo {
  local_ip: string | null
  interfaces: NetworkInterface[]
  default_gateway: string | null
  dns_servers: string[]
  hostname: string
}

export interface PublicIpInfo {
  ip: string
  city: string | null
  region: string | null
  country: string | null
  isp: string | null
  org: string | null
  timezone: string | null
}

// Path change event (from continuous trace)
export interface PathChangeEvent {
  target: string
  old: (string | null)[]
  new: (string | null)[]
  timestamp: number
}