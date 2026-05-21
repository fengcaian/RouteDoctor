<script setup lang="ts">
import { useToast } from '@/composables/useToast'

const { toasts, removeToast } = useToast()

function getIcon(type: string): string {
  switch (type) {
    case 'success': return '✓'
    case 'error': return '✕'
    case 'warning': return '⚠'
    case 'info': return 'ℹ'
    default: return 'ℹ'
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="toast-container">
      <TransitionGroup name="toast">
        <div
          v-for="toast in toasts"
          :key="toast.id"
          class="toast-item"
          :class="toast.type"
          @click="removeToast(toast.id)"
        >
          <span class="toast-icon">{{ getIcon(toast.type) }}</span>
          <span class="toast-message">{{ toast.message }}</span>
          <button class="toast-close" @click.stop="removeToast(toast.id)">✕</button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style lang="scss" scoped>
.toast-container {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 10000;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 360px;
  pointer-events: none;
}

.toast-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 10px;
  background: var(--card-bg, #1e1e1e);
  border: 1px solid var(--border-color, #333);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  cursor: pointer;
  pointer-events: all;
  transition: all 0.3s ease;

  &:hover {
    transform: translateX(-4px);
  }

  &.success {
    border-left: 4px solid #4CAF50;
    .toast-icon { color: #4CAF50; }
  }

  &.error {
    border-left: 4px solid #f44336;
    .toast-icon { color: #f44336; }
  }

  &.warning {
    border-left: 4px solid #FF9800;
    .toast-icon { color: #FF9800; }
  }

  &.info {
    border-left: 4px solid #2196F3;
    .toast-icon { color: #2196F3; }
  }
}

.toast-icon {
  font-size: 16px;
  font-weight: 700;
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toast-message {
  flex: 1;
  font-size: 13px;
  color: var(--text-primary, #fff);
  line-height: 1.4;
}

.toast-close {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--text-muted, #888);
  cursor: pointer;
  font-size: 12px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;

  &:hover {
    background: var(--hover-bg, #333);
    color: var(--text-primary, #fff);
  }
}

// 过渡动画
.toast-enter-active {
  transition: all 0.3s ease;
}

.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(100%);
}

.toast-move {
  transition: transform 0.3s ease;
}
</style>
