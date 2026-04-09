<script setup lang="ts">
import { ref, computed } from 'vue'
import { useSettingsStore } from '@/stores'
import type { ProbeMethod } from '@/types'
import { PROBE_METHOD_INFO } from '@/types'

const emit = defineEmits<{
  (e: 'start', config: { target: string, maxHops: number, timeoutMs: number, probeMethod: ProbeMethod }): void
  (e: 'stop', target: string): void
  (e: 'clear', target: string): void
}>()

const props = defineProps<{
  target: string
  isRunning: boolean
}>()

const settingsStore = useSettingsStore()

const targetInput = ref(props.target)
const maxHops = ref(settingsStore.settings.defaultTracerouteMaxHops)
const timeoutMs = ref(2000)
const probeMethod = ref<ProbeMethod>('icmp')
const validationError = ref('')

const selectedMethodInfo = computed(() => PROBE_METHOD_INFO[probeMethod.value])

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
  return targetInput.value && validateTarget(targetInput.value)
})

function handleStart() {
  if (!isValid.value) return
  emit('start', {
    target: targetInput.value.trim(),
    maxHops: maxHops.value,
    timeoutMs: timeoutMs.value,
    probeMethod: probeMethod.value
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
  <div class="trace-config">
    <div class="config-row">
      <div class="config-field">
        <label class="config-label">目标地址</label>
        <div class="input-wrapper">
          <input
            v-model="targetInput"
            type="text"
            class="config-input"
            :class="{ 'input-error': validationError }"
            placeholder="IP 或域名"
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
      <div class="config-field small">
        <label class="config-label">最大跳数</label>
        <input
          v-model.number="maxHops"
          type="number"
          class="config-input"
          min="1"
          max="64"
          :disabled="isRunning"
        />
      </div>
      <div class="config-field small">
        <label class="config-label">超时(ms)</label>
        <input
          v-model.number="timeoutMs"
          type="number"
          class="config-input"
          min="500"
          max="10000"
          step="500"
          :disabled="isRunning"
        />
      </div>
    </div>

    <div class="probe-method-section">
      <div class="probe-left">
        <label class="config-label">探测方式</label>
        <div class="probe-methods">
          <label
            v-for="(info, method) in PROBE_METHOD_INFO"
            :key="method"
            class="probe-option"
            :class="{ active: probeMethod === method, disabled: isRunning }"
          >
            <input
              type="radio"
              :value="method"
              v-model="probeMethod"
              :disabled="isRunning"
            />
            <span class="probe-name">{{ info.name }}</span>
          </label>
        </div>
      </div>
      <div class="method-info" v-if="selectedMethodInfo">
        <p class="method-desc">{{ selectedMethodInfo.description }}</p>
        <div class="pros-cons">
          <div class="pros">
            <span class="label">优点：</span>
            <span v-for="pro in selectedMethodInfo.pros" :key="pro" class="tag success">{{ pro }}</span>
          </div>
          <div class="cons">
            <span class="label">缺点：</span>
            <span v-for="con in selectedMethodInfo.cons" :key="con" class="tag warning">{{ con }}</span>
          </div>
        </div>
      </div>
    </div>

    <div class="config-actions">
      <button
        v-if="!isRunning"
        class="action-btn start"
        @click="handleStart"
        :disabled="!isValid"
      >
        开始追踪
      </button>
      <button
        v-else
        class="action-btn stop running"
        @click="handleStop"
      >
        <span class="btn-spinner"></span>
        追踪中... 点击停止
      </button>
      <button
        class="action-btn clear"
        @click="handleClear"
        :disabled="isRunning"
      >
        清除数据
      </button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.trace-config {
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
  min-width: 140px;

  &.small {
    flex: 0 0 100px;
    min-width: 80px;
  }
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

.probe-method-section {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.probe-left {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.probe-methods {
  display: flex;
  gap: 8px;
}

.probe-option {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  background: var(--input-bg);

  input {
    display: none;
  }

  .probe-name {
    font-size: 12px;
    color: var(--text-secondary);
  }

  &:hover:not(.disabled) {
    border-color: var(--accent-color);
  }

  &.active {
    border-color: var(--accent-color);
    background: rgba(76, 175, 80, 0.1);

    .probe-name {
      color: var(--accent-color);
      font-weight: 500;
    }
  }

  &.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

.method-info {
  flex: 1;
  padding: 8px 10px;
  background: var(--input-bg);
  border-radius: 8px;
  border: 1px solid var(--border-color);

  .method-desc {
    margin: 0 0 6px 0;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .pros-cons {
    display: flex;
    gap: 12px;

    .pros, .cons {
      display: flex;
      align-items: center;
      gap: 4px;
      font-size: 10px;

      .label {
        color: var(--text-muted);
        white-space: nowrap;
      }
    }

    .tag {
      padding: 2px 4px;
      border-radius: 4px;
      font-size: 9px;

      &.success {
        background: rgba(76, 175, 80, 0.15);
        color: #81c784;
      }

      &.warning {
        background: rgba(255, 152, 0, 0.15);
        color: #ffb74d;
      }
    }
  }
}

.config-actions {
  display: flex;
  gap: 10px;
}

.action-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  gap: 6px;

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
      background: #d32f2f;
    }

    &.running {
      .btn-spinner {
        width: 14px;
        height: 14px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-top-color: white;
        border-radius: 50%;
        animation: spin 1s linear infinite;
      }
    }
  }

  &.clear {
    background: var(--border-color);
    color: var(--text-primary);

    &:hover:not(:disabled) {
      background: var(--text-muted);
    }

    &:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
  }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>