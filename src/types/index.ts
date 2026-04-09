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

// Traceroute probe method
export type ProbeMethod = 'icmp' | 'udp' | 'tcp'

// Probe method descriptions
export const PROBE_METHOD_INFO: Record<ProbeMethod, { name: string; description: string; pros: string[]; cons: string[] }> = {
  icmp: {
    name: 'ICMP',
    description: '使用ICMP Echo Request探测，兼容性最好',
    pros: ['无需管理员权限', '兼容性最好', '速度快'],
    cons: ['中间路由器可能不响应', '穿透率较低']
  },
  udp: {
    name: 'UDP',
    description: '使用UDP数据包探测，能获取更多中间跳信息',
    pros: ['穿透率较高', '信息量丰富', '类似专业工具'],
    cons: ['需要管理员权限', '部分防火墙可能拦截']
  },
  tcp: {
    name: 'TCP',
    description: '使用TCP SYN包探测，穿透性最强',
    pros: ['穿透性最强', '可绕过部分防火墙'],
    cons: ['需要管理员权限', '速度较慢', '需指定端口']
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
}

// Default settings
export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'dark',
  maxHistoryDays: 30,
  defaultPingInterval: 1000,
  defaultPingTimeout: 3000,
  defaultTracerouteMaxHops: 30
}