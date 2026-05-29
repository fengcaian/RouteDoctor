import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { AppSettings } from '@/types'
import { DEFAULT_SETTINGS } from '@/types'

export const useSettingsStore = defineStore('settings', () => {
  // Load settings from localStorage
  const savedSettings = localStorage.getItem('app-settings')
  const initialSettings: AppSettings = savedSettings
    ? { ...DEFAULT_SETTINGS, ...JSON.parse(savedSettings) }
    : { ...DEFAULT_SETTINGS }

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