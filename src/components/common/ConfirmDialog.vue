<script setup lang="ts">
defineProps<{
  visible: boolean
  title?: string
  message: string
  confirmText?: string
  cancelText?: string
}>()

const emit = defineEmits<{
  confirm: []
  cancel: []
}>()
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-fade">
      <div v-if="visible" class="dialog-overlay" @click.self="emit('cancel')">
        <div class="dialog-box">
          <div class="dialog-header">
            <span class="dialog-icon">⚠️</span>
            <span class="dialog-title">{{ title || '确认操作' }}</span>
          </div>
          <div class="dialog-body">
            {{ message }}
          </div>
          <div class="dialog-footer">
            <button class="btn btn-cancel" @click="emit('cancel')">
              {{ cancelText || '取消' }}
            </button>
            <button class="btn btn-confirm" @click="emit('confirm')">
              {{ confirmText || '确认' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style lang="scss" scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  backdrop-filter: blur(2px);
}

.dialog-box {
  background: var(--card-bg, #1e1e1e);
  border: 1px solid var(--border-color, #333);
  border-radius: 12px;
  padding: 24px;
  min-width: 320px;
  max-width: 420px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;

  .dialog-icon {
    font-size: 20px;
  }

  .dialog-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary, #fff);
  }
}

.dialog-body {
  font-size: 14px;
  color: var(--text-secondary, #aaa);
  line-height: 1.6;
  margin-bottom: 24px;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.btn {
  padding: 8px 20px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-cancel {
  background: var(--border-color, #333);
  color: var(--text-primary, #fff);

  &:hover {
    background: var(--text-muted, #666);
  }
}

.btn-confirm {
  background: var(--error-color, #ef4444);
  color: white;

  &:hover {
    filter: brightness(1.15);
  }
}

// 过渡动画
.dialog-fade-enter-active,
.dialog-fade-leave-active {
  transition: opacity 0.2s ease;

  .dialog-box {
    transition: transform 0.2s ease;
  }
}

.dialog-fade-enter-from,
.dialog-fade-leave-to {
  opacity: 0;

  .dialog-box {
    transform: scale(0.95);
  }
}
</style>
