import { ref } from 'vue'

export interface Toast {
  id: string
  type: 'success' | 'error' | 'warning' | 'info'
  message: string
  duration: number
}

// 全局 Toast 状态（单例模式）
const toasts = ref<Toast[]>([])

export function useToast() {
  function addToast(type: Toast['type'], message: string, duration = 3000) {
    const id = crypto.randomUUID()
    const toast: Toast = { id, type, message, duration }
    toasts.value.push(toast)

    // 自动移除
    if (duration > 0) {
      setTimeout(() => {
        removeToast(id)
      }, duration)
    }

    // 最多同时显示 5 条
    if (toasts.value.length > 5) {
      toasts.value.shift()
    }

    return id
  }

  function removeToast(id: string) {
    const index = toasts.value.findIndex(t => t.id === id)
    if (index !== -1) {
      toasts.value.splice(index, 1)
    }
  }

  function success(message: string, duration?: number) {
    return addToast('success', message, duration)
  }

  function error(message: string, duration?: number) {
    return addToast('error', message, duration ?? 5000)
  }

  function warning(message: string, duration?: number) {
    return addToast('warning', message, duration)
  }

  function info(message: string, duration?: number) {
    return addToast('info', message, duration)
  }

  return {
    toasts,
    addToast,
    removeToast,
    success,
    error,
    warning,
    info
  }
}
