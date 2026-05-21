/**
 * 数据导出 composable
 * 支持 CSV 和 JSON 格式导出
 */

export function useExport() {
  /**
   * 导出为 CSV 文件
   */
  function exportCSV(filename: string, headers: string[], rows: (string | number | null)[][]) {
    const csvContent = [
      headers.join(','),
      ...rows.map(row =>
        row.map(cell => {
          if (cell === null || cell === undefined) return ''
          const str = String(cell)
          // 如果包含逗号、引号或换行，用引号包裹
          if (str.includes(',') || str.includes('"') || str.includes('\n')) {
            return `"${str.replace(/"/g, '""')}"`
          }
          return str
        }).join(',')
      )
    ].join('\n')

    downloadFile(`${filename}.csv`, csvContent, 'text/csv;charset=utf-8;')
  }

  /**
   * 导出为 JSON 文件
   */
  function exportJSON(filename: string, data: any) {
    const jsonContent = JSON.stringify(data, null, 2)
    downloadFile(`${filename}.json`, jsonContent, 'application/json;charset=utf-8;')
  }

  /**
   * 导出 Ping 结果为 CSV
   */
  function exportPingCSV(target: string, results: any[]) {
    const headers = ['序号', '目标', 'IP', '延迟(ms)', '是否超时', '时间戳']
    const rows = results.map(r => [
      r.seq,
      r.target,
      r.ip,
      r.is_timeout ? null : r.latency_ms,
      r.is_timeout ? '是' : '否',
      new Date(r.timestamp).toLocaleString('zh-CN')
    ])
    exportCSV(`ping_${target}_${formatDateForFile()}`, headers, rows)
  }

  /**
   * 导出 Traceroute 结果为 CSV
   */
  function exportTracerouteCSV(target: string, hops: any[]) {
    const headers = ['跳数', 'IP', '主机名', '平均延迟(ms)', '丢包率(%)', '延迟详情']
    const rows = hops.map(h => [
      h.hop_number,
      h.ip || '* * *',
      h.hostname || '--',
      h.avg_latency?.toFixed(1) || '--',
      h.packet_loss?.toFixed(1) || '0',
      h.latencies?.map((l: number | null) => l === null ? '*' : `${l.toFixed(0)}ms`).join(' / ') || '--'
    ])
    exportCSV(`traceroute_${target}_${formatDateForFile()}`, headers, rows)
  }

  /**
   * 导出带宽测试历史为 CSV
   */
  function exportBandwidthCSV(history: any[]) {
    const headers = ['时间', '下载速度(Mbps)', '上传速度(Mbps)', '延迟(ms)', '服务器']
    const rows = history.map(r => [
      new Date(r.timestamp).toLocaleString('zh-CN'),
      r.download_speed_mbps?.toFixed(2),
      r.upload_speed_mbps?.toFixed(2),
      r.latency_ms?.toFixed(1),
      r.server || '--'
    ])
    exportCSV(`bandwidth_${formatDateForFile()}`, headers, rows)
  }

  /**
   * 导出历史记录为 JSON
   */
  function exportHistoryJSON(records: any[]) {
    exportJSON(`history_${formatDateForFile()}`, {
      exportTime: new Date().toISOString(),
      totalRecords: records.length,
      records: records
    })
  }

  /**
   * 触发文件下载
   */
  function downloadFile(filename: string, content: string, mimeType: string) {
    const blob = new Blob(['\ufeff' + content], { type: mimeType })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    link.style.display = 'none'
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    URL.revokeObjectURL(url)
  }

  /**
   * 格式化日期用于文件名
   */
  function formatDateForFile(): string {
    const now = new Date()
    return `${now.getFullYear()}${(now.getMonth() + 1).toString().padStart(2, '0')}${now.getDate().toString().padStart(2, '0')}_${now.getHours().toString().padStart(2, '0')}${now.getMinutes().toString().padStart(2, '0')}`
  }

  return {
    exportCSV,
    exportJSON,
    exportPingCSV,
    exportTracerouteCSV,
    exportBandwidthCSV,
    exportHistoryJSON
  }
}
