// Npcap 安装状态 store
//
// 启动时检测一次，结果缓存到此 store。
// UDP/TCP 探测方式按钮根据此状态显示提示（已装/未装）。
// 用户在 UI 上手动安装 Npcap 后，可以触发 refresh() 重新检测。

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface NpcapStatus {
  installed: boolean
  version: string | null
  install_path: string | null
  service_registered: boolean
  supported_platform: boolean
}

const DEFAULT_STATUS: NpcapStatus = {
  installed: false,
  version: null,
  install_path: null,
  service_registered: false,
  supported_platform: false,
}

export const useNpcapStore = defineStore('npcap', () => {
  // 检测结果
  const status = ref<NpcapStatus>({ ...DEFAULT_STATUS })
  // 是否已经做过初次检测（避免重复调用后端）
  const initialized = ref(false)
  // 检测进行中（用于防抖）
  const detecting = ref(false)
  // 用户是否已经看过引导对话框（持久化到 localStorage）
  const guideShown = ref(localStorage.getItem('npcap-guide-shown') === '1')

  /**
   * 触发一次检测。后端调用是同步且轻量的（仅文件 + 注册表），无需担心阻塞。
   * force=true 时即使已 initialized 也重新检测，用于"用户刚装完 Npcap"场景。
   */
  async function refresh(force = false): Promise<NpcapStatus> {
    if (initialized.value && !force) return status.value
    if (detecting.value) return status.value
    detecting.value = true
    try {
      const result = await invoke<NpcapStatus>('get_npcap_status')
      status.value = result
      initialized.value = true
      return result
    } catch (e) {
      console.error('[npcap] 检测失败:', e)
      return status.value
    } finally {
      detecting.value = false
    }
  }

  /**
   * 标记用户已经看过引导对话框，下次启动不再自动弹出
   */
  function markGuideShown() {
    guideShown.value = true
    localStorage.setItem('npcap-guide-shown', '1')
  }

  /**
   * 重置"已看过引导"标记，让对话框可以再次自动弹出
   * （目前没用到，但保留 API 以便未来"重置提示"功能）
   */
  function resetGuideShown() {
    guideShown.value = false
    localStorage.removeItem('npcap-guide-shown')
  }

  return {
    status,
    initialized,
    detecting,
    guideShown,
    refresh,
    markGuideShown,
    resetGuideShown,
  }
})
