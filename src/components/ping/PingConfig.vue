<script setup lang="ts">
import { ref, computed } from 'vue'
import type { TargetConfig } from '@/types'
import { useSettingsStore } from '@/stores'

const emit = defineEmits<{
  (e: 'start', config: TargetConfig): void
  (e: 'stop', target: string): void
  (e: 'clear', target: string): void
}>()

const props = defineProps<{
  target: string
  isRunning: boolean
}>()

const settingsStore = useSettingsStore()

const intervalMs = ref(settingsStore.settings.defaultPingInterval)
const timeoutMs = ref(settingsStore.settings.defaultPingTimeout)
const packetSize = ref(64)
const targetInput = ref(props.target)
const validationError = ref('')

// 校验函数
function validateTarget(value: string): boolean {
  if (!value.trim()) {
    validationError.value = ''
    return false
  }

  const trimmed = value.trim()

  // IPv4 校验 - 使用函数校验确保每段 0-255
  const isValidIPv4 = (ip: string): boolean => {
    const parts = ip.split('.')
    if (parts.length !== 4) return false
    return parts.every(part => {
      const num = parseInt(part, 10)
      return !isNaN(num) && num >= 0 && num <= 255 && part === String(num)
    })
  }

  // IPv6 校验 (简化版，支持完整和缩写格式)
  const ipv6Regex = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|::|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))$/

  if (isValidIPv4(trimmed)) {
    validationError.value = ''
    return true
  }

  if (ipv6Regex.test(trimmed)) {
    validationError.value = ''
    return true
  }

  // 检查是否看起来像 IP 但实际无效
  const looksLikeIP = /^[\d.]+$/.test(trimmed)
  if (looksLikeIP) {
    validationError.value = 'IP 地址格式无效，每段数值应在 0-255 之间'
    return false
  }

  // 检查是否是 IP 和域名的混合格式（如 129.36.3.mmmm）
  const parts = trimmed.split('.')
  const hasNumericParts = parts.some(p => /^\d+$/.test(p))
  const hasNonNumericParts = parts.some(p => !/^\d+$/.test(p))
  if (hasNumericParts && hasNonNumericParts) {
    validationError.value = '地址格式无效，请输入正确的 IP 地址或域名'
    return false
  }

  // 域名校验
  // 允许的保留主机名（单段，无需点）
  const reservedHostnames = ['localhost', 'localhost.localdomain', 'broadcasthost']

  // 单段域名检查（如 localhost）
  if (reservedHostnames.includes(trimmed.toLowerCase())) {
    validationError.value = ''
    return true
  }

  // 多段域名检查（必须包含至少一个点）
  // 每段规则：字母数字开头和结尾，中间可包含连字符，长度1-63
  const labelRegex = /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/

  // 必须有至少两段
  if (parts.length < 2) {
    validationError.value = '域名格式无效，请输入完整的域名（如 google.com）'
    return false
  }

  // 检查总长度
  if (trimmed.length > 253) {
    validationError.value = '域名长度超出限制'
    return false
  }

  // 检查每段
  for (const label of parts) {
    if (!label || label.length > 63 || !labelRegex.test(label)) {
      validationError.value = '域名格式无效，请输入有效的域名'
      return false
    }
  }

  validationError.value = ''
  return true
}

const isValid = computed(() => {
  if (!targetInput.value?.trim()) return false
  const trimmed = targetInput.value.trim()

  const isValidIPv4 = (ip: string): boolean => {
    const parts = ip.split('.')
    if (parts.length !== 4) return false
    return parts.every(part => {
      const num = parseInt(part, 10)
      return !isNaN(num) && num >= 0 && num <= 255 && part === String(num)
    })
  }

  const ipv6Regex = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|::|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))$/

  if (isValidIPv4(trimmed) || ipv6Regex.test(trimmed)) return true

  const looksLikeIP = /^[\d.]+$/.test(trimmed)
  if (looksLikeIP) return false

  const parts = trimmed.split('.')
  const hasNumericParts = parts.some(p => /^\d+$/.test(p))
  const hasNonNumericParts = parts.some(p => !/^\d+$/.test(p))
  if (hasNumericParts && hasNonNumericParts) return false

  const reservedHostnames = ['localhost', 'localhost.localdomain', 'broadcasthost']
  if (reservedHostnames.includes(trimmed.toLowerCase())) return true

  const labelRegex = /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/
  if (parts.length < 2 || trimmed.length > 253) return false

  return parts.every(label => label && label.length <= 63 && labelRegex.test(label))
})

function handleStart() {
  if (!isValid.value) return
  emit('start', {
    target: targetInput.value.trim(),
    interval_ms: intervalMs.value,
    timeout_ms: timeoutMs.value,
    count: null,
    packet_size: packetSize.value
  })
}

function handleStop() {
  emit('stop', props.target)
}

function handleClear() {
  emit('clear', props.target)
}

function onInputChange(value: string) {
  if (value) {
    validateTarget(value)
  } else {
    validationError.value = ''
  }
}
</script>

<template>
  <div class="ping-config">
    <div class="config-row">
      <div class="config-field">
        <label class="config-label">Target</label>
        <div class="input-wrapper">
          <input
            v-model="targetInput"
            type="text"
            class="config-input"
            :class="{ 'input-error': validationError }"
            placeholder="IP or hostname"
            :disabled="isRunning"
            @input="onInputChange(targetInput)"
          />
          <button
            v-if="targetInput && !isRunning"
            class="clear-input-btn"
            @click="targetInput = ''; validationError = ''"
            type="button"
          >
            ✕
          </button>
        </div>
        <span v-if="validationError" class="error-text">{{ validationError }}</span>
      </div>
      <div class="config-field">
        <label class="config-label">Interval (ms)</label>
        <input
          v-model.number="intervalMs"
          type="number"
          class="config-input"
          min="100"
          max="10000"
          step="100"
          :disabled="isRunning"
        />
      </div>
      <div class="config-field">
        <label class="config-label">Timeout (ms)</label>
        <input
          v-model.number="timeoutMs"
          type="number"
          class="config-input"
          min="100"
          max="10000"
          step="100"
          :disabled="isRunning"
        />
      </div>
      <div class="config-field">
        <label class="config-label">Packet Size</label>
        <input
          v-model.number="packetSize"
          type="number"
          class="config-input"
          min="32"
          max="65500"
          step="1"
          :disabled="isRunning"
        />
      </div>
    </div>
    <div class="config-actions">
      <button
        v-if="!isRunning"
        class="action-btn start"
        @click="handleStart"
        :disabled="!isValid"
      >
        Start Ping
      </button>
      <button
        v-else
        class="action-btn stop"
        @click="handleStop"
      >
        Stop Ping
      </button>
      <button
        class="action-btn clear"
        @click="handleClear"
      >
        Clear Results
      </button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.ping-config {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px 16px;
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
}

.config-row {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 120px;
}

.config-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;

  .config-input {
    width: 100%;
    padding-right: 32px;
  }
}

.config-input {
  padding: 8px 10px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
  color: var(--text-primary);
  font-size: 13px;

  &:focus {
    outline: none;
    border-color: var(--accent-color);
    box-shadow: 0 0 0 2px var(--accent-color-alpha);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  &.input-error {
    border-color: var(--error-color);

    &:focus {
      border-color: var(--error-color);
      box-shadow: 0 0 0 2px rgba(244, 67, 54, 0.2);
    }
  }
}

.error-text {
  font-size: 11px;
  color: var(--error-color);
  margin-top: 2px;
}

.clear-input-btn {
  position: absolute;
  right: 8px;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  background: var(--border-color);
  color: var(--text-muted);
  border-radius: 50%;
  cursor: pointer;
  font-size: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;

  &:hover {
    background: var(--error-color);
    color: white;
  }
}

.config-actions {
  display: flex;
  gap: 8px;
}

.action-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;

  &.start {
    background: var(--accent-color);
    color: white;

    &:hover:not(:disabled) {
      background: var(--accent-color-hover);
    }

    &:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
  }

  &.stop {
    background: var(--error-color);
    color: white;

    &:hover {
      background: var(--error-color-hover);
    }
  }

  &.clear {
    background: var(--button-bg);
    color: var(--text-primary);
    border: 1px solid var(--border-color);

    &:hover {
      background: var(--hover-bg);
    }
  }
}
</style>