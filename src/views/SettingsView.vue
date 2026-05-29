<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores'
import ConfirmDialog from '@/components/common/ConfirmDialog.vue'

const { t, locale } = useI18n()
const settingsStore = useSettingsStore()

const showResetConfirm = ref(false)

function setTheme(theme: 'light' | 'dark' | 'system') {
  settingsStore.setTheme(theme)
}

function setLocale(lang: string) {
  locale.value = lang
  localStorage.setItem('locale', lang)
}

function handleReset() {
  showResetConfirm.value = true
}

function confirmReset() {
  showResetConfirm.value = false
  settingsStore.resetSettings()
  locale.value = 'zh'
  localStorage.setItem('locale', 'zh')
}
</script>

<template>
  <div class="settings-view">
    <ConfirmDialog
      :visible="showResetConfirm"
      :title="t('settings.resetConfirmTitle')"
      :message="t('settings.resetConfirmMessage')"
      @confirm="confirmReset"
      @cancel="showResetConfirm = false"
    />

    <div class="view-header">
      <h2>{{ t('settings.title') }}</h2>
      <p class="subtitle">{{ t('settings.subtitle') }}</p>
    </div>

    <div class="settings-content">
      <!-- 外观设置 -->
      <div class="settings-section">
        <h3 class="section-title">{{ t('settings.appearance') }}</h3>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.theme') }}</label>
          <div class="setting-options">
            <button
              :class="['option-btn', { active: settingsStore.settings.theme === 'dark' }]"
              @click="setTheme('dark')"
            >🌙 {{ t('settings.themeDark') }}</button>
            <button
              :class="['option-btn', { active: settingsStore.settings.theme === 'light' }]"
              @click="setTheme('light')"
            >☀️ {{ t('settings.themeLight') }}</button>
            <button
              :class="['option-btn', { active: settingsStore.settings.theme === 'system' }]"
              @click="setTheme('system')"
            >💻 {{ t('settings.themeSystem') }}</button>
          </div>
        </div>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.language') }}</label>
          <div class="setting-options">
            <button
              :class="['option-btn', { active: locale === 'zh' }]"
              @click="setLocale('zh')"
            >{{ t('settings.langZh') }}</button>
            <button
              :class="['option-btn', { active: locale === 'en' }]"
              @click="setLocale('en')"
            >{{ t('settings.langEn') }}</button>
          </div>
        </div>
      </div>

      <!-- Ping 默认设置 -->
      <div class="settings-section">
        <h3 class="section-title">{{ t('settings.pingDefaults') }}</h3>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.defaultInterval') }}</label>
          <input
            type="number"
            class="setting-input"
            v-model.number="settingsStore.settings.defaultPingInterval"
            min="100" max="10000" step="100"
          />
        </div>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.defaultTimeout') }}</label>
          <input
            type="number"
            class="setting-input"
            v-model.number="settingsStore.settings.defaultPingTimeout"
            min="100" max="10000" step="100"
          />
        </div>
      </div>

      <!-- Traceroute 默认设置 -->
      <div class="settings-section">
        <h3 class="section-title">{{ t('settings.traceDefaults') }}</h3>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.defaultMaxHops') }}</label>
          <input
            type="number"
            class="setting-input"
            v-model.number="settingsStore.settings.defaultTracerouteMaxHops"
            min="1" max="64"
          />
        </div>
      </div>

      <!-- 数据管理 -->
      <div class="settings-section">
        <h3 class="section-title">{{ t('settings.dataManagement') }}</h3>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.maxHistoryDays') }}</label>
          <input
            type="number"
            class="setting-input"
            v-model.number="settingsStore.settings.maxHistoryDays"
            min="1" max="365"
          />
        </div>
      </div>

      <!-- 系统集成 -->
      <div class="settings-section">
        <h3 class="section-title">{{ t('settings.systemIntegration') }}</h3>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.minimizeToTray') }}</label>
          <label class="switch">
            <input
              type="checkbox"
              v-model="settingsStore.settings.minimizeToTray"
            />
            <span class="slider"></span>
          </label>
        </div>
        <div class="setting-item">
          <label class="setting-label">{{ t('settings.autostart') }}</label>
          <label class="switch">
            <input
              type="checkbox"
              v-model="settingsStore.settings.autostart"
            />
            <span class="slider"></span>
          </label>
        </div>
      </div>

      <!-- 恢复默认 -->
      <div class="settings-section">
        <button class="reset-btn" @click="handleReset">
          {{ t('settings.resetSettings') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.settings-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 12px;
  height: 100%;
  overflow-y: auto;
}

.view-header {
  h2 {
    font-size: 20px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }
  .subtitle {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
}

.settings-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.settings-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.setting-label {
  font-size: 13px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.setting-options {
  display: flex;
  gap: 8px;
}

.option-btn {
  padding: 6px 14px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--input-bg);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    border-color: var(--accent-color);
    color: var(--text-primary);
  }

  &.active {
    background: var(--accent-color);
    border-color: var(--accent-color);
    color: white;
  }
}

.setting-input {
  width: 120px;
  padding: 6px 10px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--input-bg);
  color: var(--text-primary);
  font-size: 13px;
  text-align: right;

  &:focus {
    outline: none;
    border-color: var(--accent-color);
  }
}

.reset-btn {
  padding: 10px 20px;
  background: transparent;
  color: var(--error-color, #ef4444);
  border: 1px solid var(--error-color, #ef4444);
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  align-self: flex-start;

  &:hover {
    background: var(--error-color, #ef4444);
    color: white;
  }
}

.switch {
  position: relative;
  display: inline-block;
  width: 42px;
  height: 22px;

  input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: var(--border-color);
    transition: 0.2s;
    border-radius: 22px;

    &::before {
      position: absolute;
      content: '';
      height: 16px;
      width: 16px;
      left: 3px;
      bottom: 3px;
      background: white;
      transition: 0.2s;
      border-radius: 50%;
    }
  }

  input:checked + .slider {
    background: var(--accent-color);
  }

  input:checked + .slider::before {
    transform: translateX(20px);
  }
}
</style>
