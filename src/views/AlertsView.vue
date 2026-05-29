<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAlertStore } from '@/stores'
import type { AlertConditionType } from '@/stores/alertStore'

const { t } = useI18n()
const alertStore = useAlertStore()

const showAddForm = ref(false)

const initialForm = () => ({
  name: '',
  target: '*',
  enabled: true,
  conditionType: 'latency' as AlertConditionType,
  threshold: 200,
  duration: 3,
  notifyToast: true,
  notifySound: false,
  notifySystem: false,
  webhook: ''
})

const form = reactive(initialForm())

function resetForm() {
  Object.assign(form, initialForm())
}

function submitForm() {
  if (!form.name.trim()) {
    form.name = '未命名规则'
  }
  alertStore.addRule({
    name: form.name,
    target: form.target,
    enabled: form.enabled,
    condition: {
      type: form.conditionType,
      threshold: Number(form.threshold) || 0,
      duration: Number(form.duration) || 1
    },
    notify: {
      toast: form.notifyToast,
      sound: form.notifySound,
      system: form.notifySystem,
      webhook: form.webhook.trim()
    }
  })
  resetForm()
  showAddForm.value = false
}

function formatTime(ts: number): string {
  const d = new Date(ts)
  return `${d.toLocaleDateString()} ${d.toLocaleTimeString()}`
}

function describeCondition(rule: { condition: { type: string; threshold: number } }): string {
  switch (rule.condition.type) {
    case 'latency': return `${t('alerts.latencyGt')} ${rule.condition.threshold}ms`
    case 'loss': return `${t('alerts.lossGt')} ${rule.condition.threshold}%`
    case 'timeout_streak': return `${t('alerts.timeoutStreak')} ≥ ${rule.condition.threshold}`
    case 'path_change': return t('alerts.pathChange')
    default: return rule.condition.type
  }
}
</script>

<template>
  <div class="alerts-view">
    <div class="view-header">
      <h2>{{ t('alerts.title') }}</h2>
      <p class="subtitle">{{ t('alerts.subtitle') }}</p>
    </div>

    <div class="content">
      <!-- 规则管理 -->
      <div class="settings-section">
        <div class="section-header">
          <h3 class="section-title">{{ t('alerts.rules') }}</h3>
          <button class="btn-primary" @click="showAddForm = !showAddForm">
            {{ showAddForm ? t('alerts.cancel') : '+ ' + t('alerts.addRule') }}
          </button>
        </div>

        <!-- 新建规则表单 -->
        <div v-if="showAddForm" class="rule-form">
          <div class="form-row">
            <label>{{ t('alerts.ruleName') }}</label>
            <input v-model="form.name" type="text" :placeholder="t('alerts.ruleNamePlaceholder')" />
          </div>
          <div class="form-row">
            <label>{{ t('alerts.target') }}</label>
            <input v-model="form.target" type="text" placeholder="*" />
          </div>
          <div class="form-row">
            <label>{{ t('alerts.conditionType') }}</label>
            <div class="condition-buttons">
              <button
                v-for="ct in (['latency', 'loss', 'timeout_streak', 'path_change'] as AlertConditionType[])"
                :key="ct"
                :class="['type-btn', { active: form.conditionType === ct }]"
                @click="form.conditionType = ct"
              >{{ t('alerts.cond.' + ct) }}</button>
            </div>
          </div>
          <div class="form-row" v-if="form.conditionType !== 'path_change'">
            <label>{{ t('alerts.threshold') }}</label>
            <input v-model.number="form.threshold" type="number" min="0" />
          </div>
          <div class="form-row">
            <label>{{ t('alerts.duration') }}</label>
            <input v-model.number="form.duration" type="number" min="1" />
          </div>
          <div class="form-row notify-row">
            <label>{{ t('alerts.notifyWays') }}</label>
            <div class="notify-checks">
              <label><input type="checkbox" v-model="form.notifyToast" /> Toast</label>
              <label><input type="checkbox" v-model="form.notifySystem" /> {{ t('alerts.systemNotify') }}</label>
              <label><input type="checkbox" v-model="form.notifySound" /> {{ t('alerts.sound') }}</label>
            </div>
          </div>
          <div class="form-row">
            <label>{{ t('alerts.webhookUrl') }}</label>
            <input v-model="form.webhook" type="text" placeholder="https://example.com/webhook" />
          </div>
          <div class="form-actions">
            <button class="btn-primary" @click="submitForm">{{ t('alerts.save') }}</button>
            <button class="btn-secondary" @click="showAddForm = false; resetForm()">{{ t('alerts.cancel') }}</button>
          </div>
        </div>

        <!-- 规则列表 -->
        <table v-if="alertStore.rules.length > 0" class="rule-table">
          <thead>
            <tr>
              <th>{{ t('alerts.name') }}</th>
              <th>{{ t('alerts.target') }}</th>
              <th>{{ t('alerts.condition') }}</th>
              <th>{{ t('alerts.notify') }}</th>
              <th>{{ t('alerts.triggers') }}</th>
              <th>{{ t('alerts.enabled') }}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="rule in alertStore.rules" :key="rule.id">
              <td>{{ rule.name }}</td>
              <td>{{ rule.target }}</td>
              <td>{{ describeCondition(rule) }}</td>
              <td class="notify-cell">
                <span v-if="rule.notify.toast" title="Toast">📢</span>
                <span v-if="rule.notify.system" :title="t('alerts.systemNotify')">🔔</span>
                <span v-if="rule.notify.webhook" title="Webhook">🌐</span>
              </td>
              <td>{{ rule.triggerCount }}</td>
              <td>
                <label class="switch">
                  <input type="checkbox" :checked="rule.enabled" @change="alertStore.toggleRule(rule.id)" />
                  <span class="slider"></span>
                </label>
              </td>
              <td>
                <button class="btn-danger-small" @click="alertStore.removeRule(rule.id)">
                  {{ t('alerts.delete') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty">{{ t('alerts.noRules') }}</div>
      </div>

      <!-- 最近事件 -->
      <div class="settings-section">
        <div class="section-header">
          <h3 class="section-title">{{ t('alerts.events') }}</h3>
          <button class="btn-secondary" @click="alertStore.clearEvents()">
            {{ t('alerts.clearEvents') }}
          </button>
        </div>
        <table v-if="alertStore.events.length > 0" class="rule-table">
          <thead>
            <tr>
              <th>{{ t('alerts.time') }}</th>
              <th>{{ t('alerts.target') }}</th>
              <th>{{ t('alerts.message') }}</th>
              <th>{{ t('alerts.value') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="event in alertStore.events" :key="event.id">
              <td>{{ formatTime(event.timestamp) }}</td>
              <td>{{ event.target }}</td>
              <td>{{ event.message }}</td>
              <td>{{ event.value.toFixed(1) }}</td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty">{{ t('alerts.noEvents') }}</div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.alerts-view {
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

.content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.settings-section {
  background: var(--card-bg);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.rule-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  border: 1px dashed var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;

  label {
    flex: 0 0 110px;
    font-size: 13px;
    color: var(--text-secondary);
  }

  input[type="text"],
  input[type="number"] {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: var(--card-bg);
    color: var(--text-primary);
    font-size: 13px;

    &:focus {
      outline: none;
      border-color: var(--accent-color);
    }
  }
}

.notify-row {
  align-items: flex-start;
}

.notify-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;

  label {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-primary);
  }
}

.condition-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.type-btn {
  padding: 5px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--card-bg);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;

  &.active {
    background: var(--accent-color);
    border-color: var(--accent-color);
    color: white;
  }
}

.form-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.btn-primary,
.btn-secondary,
.btn-danger-small {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: opacity 0.15s;

  &:hover { opacity: 0.85; }
}

.btn-primary {
  background: var(--accent-color);
  color: white;
}

.btn-secondary {
  background: transparent;
  border-color: var(--border-color);
  color: var(--text-secondary);
}

.btn-danger-small {
  background: transparent;
  border-color: var(--error-color, #ef4444);
  color: var(--error-color, #ef4444);
  padding: 4px 10px;
}

.rule-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;

  th, td {
    padding: 8px 10px;
    text-align: left;
    border-bottom: 1px solid var(--border-color);
    color: var(--text-primary);
  }

  th {
    color: var(--text-muted);
    font-weight: 600;
    background: var(--table-header-bg);
  }
}

.notify-cell {
  display: flex;
  gap: 6px;
  font-size: 13px;
}

.switch {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 18px;

  input { opacity: 0; width: 0; height: 0; }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0; left: 0; right: 0; bottom: 0;
    background: var(--border-color);
    transition: 0.2s;
    border-radius: 18px;

    &::before {
      position: absolute;
      content: '';
      height: 12px;
      width: 12px;
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
    transform: translateX(18px);
  }
}

.empty {
  padding: 20px;
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}
</style>
