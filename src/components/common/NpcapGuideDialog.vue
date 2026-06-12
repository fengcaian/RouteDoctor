<script setup lang="ts">
// Npcap 安装引导对话框
//
// 在用户首次切换到 UDP/TCP 探测方式（且 Npcap 未安装）时弹出。
// 解释 Npcap 的作用与安装收益，提供"打开下载页"的快捷按钮。
// 用户做出选择后写入 localStorage，下次启动不再自动弹出（除非用户主动触发）。

import { onMounted, onUnmounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { open } from '@tauri-apps/plugin-shell'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  // 触发对话框的协议（'UDP' / 'TCP'），用于文案插值
  protocol: string
}>()

const emit = defineEmits<{
  // 用户做出任意选择都关闭对话框
  close: []
  // 已经引导用户去下载（让 store 记录"已展示过"）
  acknowledged: []
}>()

const NPCAP_DOWNLOAD_URL = 'https://npcap.com/#download'

const protocolUpper = computed(() => props.protocol.toUpperCase())

async function openDownload() {
  try {
    await open(NPCAP_DOWNLOAD_URL)
  } catch (e) {
    // tauri shell 打开失败时退化到剪贴板/提示
    console.error('[npcap] 打开下载页失败:', e)
    try {
      await navigator.clipboard.writeText(NPCAP_DOWNLOAD_URL)
    } catch {
      /* 剪贴板也失败就放弃，至少链接显示在对话框里 */
    }
  }
  emit('acknowledged')
  emit('close')
}

function remindLater() {
  emit('close')
}

function keepBasic() {
  emit('acknowledged')
  emit('close')
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) {
    remindLater()
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-fade">
      <div v-if="visible" class="dialog-overlay" @click.self="remindLater">
        <div class="dialog-box">
          <div class="dialog-header">
            <span class="dialog-icon">🚀</span>
            <span class="dialog-title">
              {{ t('traceroute.npcapTip.dialogTitle', { protocol: protocolUpper }) }}
            </span>
          </div>
          <div class="dialog-body">
            <p class="intro">{{ t('traceroute.npcapTip.dialogIntro') }}</p>
            <div class="benefit-block">
              <p class="benefit-title">{{ t('traceroute.npcapTip.dialogBenefit') }}</p>
              <ul class="benefit-list">
                <li>{{ t('traceroute.npcapTip.benefit1', { protocol: protocolUpper }) }}</li>
                <li>{{ t('traceroute.npcapTip.benefit2') }}</li>
                <li>{{ t('traceroute.npcapTip.benefit3', { protocol: protocolUpper }) }}</li>
              </ul>
            </div>
            <p class="trust">
              {{ t('traceroute.npcapTip.dialogTrust') }}
              <span class="size-hint">{{ t('traceroute.npcapTip.dialogSize') }}</span>
            </p>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-text" @click="keepBasic">
              {{ t('traceroute.npcapTip.keepBasic') }}
            </button>
            <button class="btn btn-secondary" @click="remindLater">
              {{ t('traceroute.npcapTip.remindLater') }}
            </button>
            <button class="btn btn-primary" @click="openDownload">
              {{ t('traceroute.npcapTip.openDownload') }}
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
  min-width: 420px;
  max-width: 520px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;

  .dialog-icon {
    font-size: 22px;
  }

  .dialog-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary, #fff);
    line-height: 1.4;
  }
}

.dialog-body {
  font-size: 14px;
  color: var(--text-secondary, #aaa);
  line-height: 1.65;
  margin-bottom: 24px;

  .intro {
    margin: 0 0 14px;
  }

  .benefit-block {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color, #2a2a2a);
    border-radius: 8px;
    padding: 12px 16px;
    margin: 14px 0;
  }

  .benefit-title {
    margin: 0 0 8px;
    color: var(--text-primary, #fff);
    font-weight: 500;
  }

  .benefit-list {
    margin: 0;
    padding-left: 20px;

    li {
      margin: 4px 0;
    }
  }

  .trust {
    margin: 14px 0 0;
    font-size: 13px;
  }

  .size-hint {
    color: var(--text-muted, #666);
    margin-left: 6px;
  }
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  flex-wrap: wrap;
}

.btn {
  padding: 8px 18px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-text {
  background: transparent;
  color: var(--text-muted, #666);

  &:hover {
    color: var(--text-secondary, #aaa);
  }
}

.btn-secondary {
  background: var(--border-color, #333);
  color: var(--text-primary, #fff);

  &:hover {
    background: var(--text-muted, #666);
  }
}

.btn-primary {
  background: var(--primary-color, #10b981);
  color: white;

  &:hover {
    filter: brightness(1.1);
  }
}

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
