import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { HistoryRecord } from '@/types'

export interface HistorySession {
  id: number
  target: string
  test_type: 'ping' | 'traceroute' | 'bandwidth'
  start_time: number
  end_time: number
  data: any
}

export const useHistoryStore = defineStore('history', () => {
  // State
  const records = ref<HistorySession[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // Actions
  async function loadHistory(
    target?: string,
    testType?: 'ping' | 'traceroute' | 'bandwidth',
    limit?: number
  ) {
    isLoading.value = true
    error.value = null

    try {
      const result = await invoke<HistoryRecord[]>('get_history', {
        target,
        testType,
        limit
      })

      // Parse JSON data for each record
      records.value = result.map(record => {
        try {
          return {
            ...record,
            data: record.data ? JSON.parse(record.data) : {}
          }
        } catch (e) {
          console.warn('Failed to parse data for record:', record.id, e)
          return {
            ...record,
            data: {}
          }
        }
      })
    } catch (e: any) {
      error.value = e.message || 'Failed to load history'
      console.error('Failed to load history:', e)
    } finally {
      isLoading.value = false
    }
  }

  function clearRecords() {
    records.value = []
  }

  async function clearAllHistory() {
    try {
      await invoke('clear_history')
      records.value = []
    } catch (e: any) {
      error.value = e.message || '清除历史记录失败'
      console.error('Failed to clear history:', e)
    }
  }

  function addRecord(record: HistorySession) {
    records.value.unshift(record)
  }

  return {
    records,
    isLoading,
    error,
    loadHistory,
    clearRecords,
    clearAllHistory,
    addRecord
  }
})
