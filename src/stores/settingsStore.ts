import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { AppSettings } from '@/types'
import { DEFAULT_SETTINGS } from '@/types'

// 设置 schema 版本号。当前 v1，不存在历史迁移。
// 升级原则：新增/修改默认值时，不强制覆盖已存在的用户偏好；
// 仅在确有兼容性破坏（字段类型变更、字段重命名等）时才 bump 版本并加迁移分支。
const SETTINGS_SCHEMA_VERSION = 1
const STORAGE_VERSION_KEY = 'app-settings-version'

/**
 * 一次性迁移：仅用于不兼容的 schema 变更（字段类型/重命名/拆分等）。
 * 严禁在此处覆盖"用户可能主动设置过的偏好字段"，否则会破坏跨版本的用户体验。
 */
function runMigrations(settings: AppSettings, _fromVersion: number): AppSettings {
  // 目前无迁移分支
  return settings
}

export const useSettingsStore = defineStore('settings', () => {
  // Load settings from localStorage
  const savedSettings = localStorage.getItem('app-settings')
  const savedVersion = parseInt(
    localStorage.getItem(STORAGE_VERSION_KEY) ?? String(SETTINGS_SCHEMA_VERSION),
    10
  )

  // 合并策略：先放默认值，再用 localStorage 里的偏好覆盖。
  // 这保证了"新安装用户拿到默认值，已安装用户保留自己的配置"。
  let initialSettings: AppSettings = savedSettings
    ? { ...DEFAULT_SETTINGS, ...JSON.parse(savedSettings) }
    : { ...DEFAULT_SETTINGS }

  // 仅当 schema 版本落后才跑迁移（当前 v1 不会触发）
  if (savedVersion < SETTINGS_SCHEMA_VERSION) {
    initialSettings = runMigrations(initialSettings, savedVersion)
    localStorage.setItem(STORAGE_VERSION_KEY, String(SETTINGS_SCHEMA_VERSION))
    localStorage.setItem('app-settings', JSON.stringify(initialSettings))
  }

  // State
  const settings = ref<AppSettings>(initialSettings)

  // Watch for changes and save to localStorage
  watch(settings, (newSettings) => {
    localStorage.setItem('app-settings', JSON.stringify(newSettings))
  }, { deep: true })

  // Actions
  function updateSettings(newSettings: Partial<AppSettings>) {
    settings.value = { ...settings.value, ...newSettings }
  }

  function resetSettings() {
    settings.value = { ...DEFAULT_SETTINGS }
  }

  function setTheme(theme: 'light' | 'dark' | 'system') {
    settings.value.theme = theme
    applyTheme(theme)
  }

  function applyTheme(theme: 'light' | 'dark' | 'system') {
    const root = document.documentElement
    if (theme === 'system') {
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
      root.setAttribute('data-theme', prefersDark ? 'dark' : 'light')
    } else {
      root.setAttribute('data-theme', theme)
    }
  }

  // System integration: push minimize-to-tray to Rust whenever it changes.
  watch(
    () => settings.value.minimizeToTray,
    (val) => {
      invoke('set_minimize_to_tray', { enabled: val }).catch((e) => {
        console.error('set_minimize_to_tray failed', e)
      })
    },
    { immediate: true }
  )

  // System integration: sync autostart state with the OS on every change.
  watch(
    () => settings.value.autostart,
    (val) => {
      invoke('set_autostart', { enabled: val }).catch((e) => {
        console.error('set_autostart failed', e)
      })
    }
  )

  // On boot, query the actual autostart state from the OS to stay in sync.
  invoke<boolean>('is_autostart_enabled')
    .then((enabled) => {
      if (settings.value.autostart !== enabled) {
        settings.value.autostart = enabled
      }
    })
    .catch(() => {})

  // Apply theme on init
  applyTheme(settings.value.theme)

  return {
    settings,
    updateSettings,
    resetSettings,
    setTheme
  }
})