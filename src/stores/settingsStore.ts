import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import type { AppSettings } from '@/types'
import { DEFAULT_SETTINGS } from '@/types'

export const useSettingsStore = defineStore('settings', () => {
  // Load settings from localStorage
  const savedSettings = localStorage.getItem('app-settings')
  const initialSettings: AppSettings = savedSettings
    ? JSON.parse(savedSettings)
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

  // Apply theme on init
  applyTheme(settings.value.theme)

  return {
    settings,
    updateSettings,
    resetSettings,
    setTheme
  }
})