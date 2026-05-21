import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { useToast } from '@/composables/useToast'

export interface AlertRule {
  id: string
  target: string          // 监控目标（'*' 表示所有目标）
  enabled: boolean
  // 触发条件
  condition: {
    type: 'latency' | 'loss' | 'timeout_streak'
    threshold: number     // latency: ms, loss: %, timeout_streak: 连续超时次数
    duration?: number     // 持续多少次采样后触发（防抖）
  }
  // 通知方式
  notify: {
    toast: boolean
    sound: boolean
    system: boolean       // 系统通知（Tauri notification）
  }
  // 状态
  lastTriggered: number | null
  triggerCount: number
}

export interface AlertEvent {
  id: string
  ruleId: string
  target: string
  message: string
  timestamp: number
  value: number           // 触发时的实际值
}

export const useAlertStore = defineStore('alerts', () => {
  // 从 localStorage 加载规则
  const savedRules = localStorage.getItem('alert-rules')
  const rules = ref<AlertRule[]>(savedRules ? JSON.parse(savedRules) : getDefaultRules())

  // 告警事件历史（最近 100 条）
  const events = ref<AlertEvent[]>([])

  // 用于跟踪连续超时次数
  const timeoutStreaks = ref<Map<string, number>>(new Map())

  // 用于跟踪条件持续满足的次数（防抖）
  const conditionCounts = ref<Map<string, number>>(new Map())

  // 自动保存规则
  watch(rules, (val) => {
    localStorage.setItem('alert-rules', JSON.stringify(val))
  }, { deep: true })

  /**
   * 检查 Ping 结果是否触发告警
   */
  function checkPingResult(target: string, latencyMs: number | null, isTimeout: boolean) {
    const toast = useToast()

    // 更新超时连续计数
    if (isTimeout) {
      const current = timeoutStreaks.value.get(target) || 0
      timeoutStreaks.value.set(target, current + 1)
    } else {
      timeoutStreaks.value.set(target, 0)
    }

    // 检查每条规则
    for (const rule of rules.value) {
      if (!rule.enabled) continue
      if (rule.target !== '*' && rule.target !== target) continue

      let triggered = false
      let actualValue = 0
      const conditionKey = `${rule.id}_${target}`

      switch (rule.condition.type) {
        case 'latency':
          if (latencyMs !== null && latencyMs > rule.condition.threshold) {
            triggered = true
            actualValue = latencyMs
          }
          break

        case 'loss':
          // 丢包率需要在外部统计后调用 checkLossRate
          break

        case 'timeout_streak':
          const streak = timeoutStreaks.value.get(target) || 0
          if (streak >= rule.condition.threshold) {
            triggered = true
            actualValue = streak
          }
          break
      }

      if (triggered) {
        // 防抖：需要连续满足 duration 次
        const duration = rule.condition.duration || 1
        const count = (conditionCounts.value.get(conditionKey) || 0) + 1
        conditionCounts.value.set(conditionKey, count)

        if (count >= duration) {
          // 触发告警（每 30 秒最多触发一次同一规则）
          const now = Date.now()
          if (!rule.lastTriggered || now - rule.lastTriggered > 30000) {
            fireAlert(rule, target, actualValue, toast)
          }
          conditionCounts.value.set(conditionKey, 0)
        }
      } else {
        // 条件不满足，重置计数
        conditionCounts.value.set(conditionKey, 0)
      }
    }
  }

  /**
   * 检查丢包率
   */
  function checkLossRate(target: string, lossRate: number) {
    const toast = useToast()

    for (const rule of rules.value) {
      if (!rule.enabled) continue
      if (rule.condition.type !== 'loss') continue
      if (rule.target !== '*' && rule.target !== target) continue

      if (lossRate > rule.condition.threshold) {
        const now = Date.now()
        if (!rule.lastTriggered || now - rule.lastTriggered > 30000) {
          fireAlert(rule, target, lossRate, toast)
        }
      }
    }
  }

  /**
   * 触发告警
   */
  function fireAlert(rule: AlertRule, target: string, value: number, toast: ReturnType<typeof useToast>) {
    rule.lastTriggered = Date.now()
    rule.triggerCount++

    let message = ''
    switch (rule.condition.type) {
      case 'latency':
        message = `⚠️ ${target} 延迟过高: ${value.toFixed(1)}ms (阈值: ${rule.condition.threshold}ms)`
        break
      case 'loss':
        message = `⚠️ ${target} 丢包率过高: ${value.toFixed(1)}% (阈值: ${rule.condition.threshold}%)`
        break
      case 'timeout_streak':
        message = `⚠️ ${target} 连续超时 ${value} 次 (阈值: ${rule.condition.threshold} 次)`
        break
    }

    // 记录事件
    const event: AlertEvent = {
      id: crypto.randomUUID(),
      ruleId: rule.id,
      target,
      message,
      timestamp: Date.now(),
      value
    }
    events.value.unshift(event)
    if (events.value.length > 100) {
      events.value.pop()
    }

    // 通知
    if (rule.notify.toast) {
      toast.warning(message, 5000)
    }

    if (rule.notify.system) {
      // Tauri 系统通知
      try {
        new Notification('PingPlotter Next 告警', { body: message })
      } catch (e) {
        // 系统通知不可用时降级为 toast
        if (!rule.notify.toast) {
          toast.warning(message, 5000)
        }
      }
    }
  }

  /**
   * 添加规则
   */
  function addRule(rule: Omit<AlertRule, 'id' | 'lastTriggered' | 'triggerCount'>) {
    rules.value.push({
      ...rule,
      id: crypto.randomUUID(),
      lastTriggered: null,
      triggerCount: 0
    })
  }

  /**
   * 删除规则
   */
  function removeRule(id: string) {
    const index = rules.value.findIndex(r => r.id === id)
    if (index !== -1) {
      rules.value.splice(index, 1)
    }
  }

  /**
   * 切换规则启用状态
   */
  function toggleRule(id: string) {
    const rule = rules.value.find(r => r.id === id)
    if (rule) {
      rule.enabled = !rule.enabled
    }
  }

  /**
   * 清除告警事件
   */
  function clearEvents() {
    events.value = []
  }

  return {
    rules,
    events,
    checkPingResult,
    checkLossRate,
    addRule,
    removeRule,
    toggleRule,
    clearEvents
  }
})

/**
 * 默认告警规则
 */
function getDefaultRules(): AlertRule[] {
  return [
    {
      id: crypto.randomUUID(),
      target: '*',
      enabled: true,
      condition: {
        type: 'latency',
        threshold: 200,
        duration: 3
      },
      notify: { toast: true, sound: false, system: false },
      lastTriggered: null,
      triggerCount: 0
    },
    {
      id: crypto.randomUUID(),
      target: '*',
      enabled: true,
      condition: {
        type: 'timeout_streak',
        threshold: 5,
        duration: 1
      },
      notify: { toast: true, sound: false, system: true },
      lastTriggered: null,
      triggerCount: 0
    }
  ]
}
