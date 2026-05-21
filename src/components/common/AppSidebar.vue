<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores'

const route = useRoute()
const router = useRouter()
const settingsStore = useSettingsStore()
const { t } = useI18n()

// 侧边栏折叠状态
const collapsed = ref(false)

const navItems = computed(() => [
  { path: '/ping', name: t('nav.ping'), icon: '📡' },
  { path: '/traceroute', name: t('nav.traceroute'), icon: '🔗' },
  { path: '/bandwidth', name: t('nav.bandwidth'), icon: '⚡' },
  { path: '/dns', name: t('nav.dns'), icon: '🔎' },
  { path: '/network-info', name: t('nav.networkInfo'), icon: '🖥️' },
  { path: '/history', name: t('nav.history'), icon: '📊' },
  { path: '/settings', name: t('nav.settings'), icon: '⚙️' },
])

const currentTheme = computed(() => settingsStore.settings.theme)

function toggleTheme() {
  const newTheme = currentTheme.value === 'dark' ? 'light' : 'dark'
  settingsStore.setTheme(newTheme)
}

function toggleCollapse() {
  collapsed.value = !collapsed.value
}

function isActive(path: string): boolean {
  return route.path === path
}

function goToHome() {
  router.push('/')
}
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <div class="sidebar-header" @click="goToHome" :title="t('nav.backHome')">
      <h1 class="app-title">{{ collapsed ? 'PP' : 'PingPlotter' }}</h1>
      <span v-if="!collapsed" class="app-version">Next</span>
    </div>

    <nav class="sidebar-nav">
      <router-link
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        :class="['nav-item', { active: isActive(item.path) }]"
        :title="collapsed ? item.name : undefined"
      >
        <span class="nav-icon">{{ item.icon }}</span>
        <span v-if="!collapsed" class="nav-label">{{ item.name }}</span>
      </router-link>
    </nav>

    <div class="sidebar-footer">
      <button class="theme-toggle" @click="toggleTheme" :title="collapsed ? t('nav.switchLang') : undefined">
        <span v-if="currentTheme === 'dark'">🌙</span>
        <span v-else>☀️</span>
      </button>
      <button class="collapse-toggle" @click="toggleCollapse" :title="collapsed ? '展开' : '折叠'">
        <span>{{ collapsed ? '»' : '«' }}</span>
      </button>
    </div>
  </aside>
</template>

<style lang="scss" scoped>
.sidebar {
  width: 220px;
  height: 100vh;
  background: var(--sidebar-bg);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  transition: width 0.25s ease;

  &.collapsed {
    width: 60px;

    .sidebar-header {
      padding: 20px 12px;
      justify-content: center;
    }

    .app-title {
      font-size: 14px;
    }

    .nav-item {
      padding: 12px 0;
      justify-content: center;
    }

    .nav-icon {
      width: auto;
    }

    .sidebar-footer {
      padding: 12px 8px;
      flex-direction: column;
      gap: 8px;
    }

    .theme-toggle,
    .collapse-toggle {
      width: 100%;
    }
  }
}

.sidebar-header {
  padding: 20px;
  display: flex;
  align-items: baseline;
  gap: 8px;
  cursor: pointer;
  transition: background 0.2s ease;

  &:hover {
    background: var(--hover-bg);
  }

  &:active {
    opacity: 0.8;
  }
}

.app-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  white-space: nowrap;
}

.app-version {
  font-size: 12px;
  color: var(--accent-color);
  font-weight: 600;
}

.sidebar-nav {
  flex: 1;
  padding: 10px 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 20px;
  color: var(--text-secondary);
  text-decoration: none;
  transition: all 0.2s ease;
  border-radius: 0;

  &:hover {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  &.active {
    background: var(--active-bg);
    color: var(--accent-color);
    border-left: 3px solid var(--accent-color);
  }
}

.nav-icon {
  font-size: 18px;
  width: 24px;
  text-align: center;
  flex-shrink: 0;
}

.nav-label {
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
}

.sidebar-footer {
  padding: 16px 20px;
  border-top: 1px solid var(--border-color);
  display: flex;
  gap: 8px;
}

.theme-toggle {
  flex: 1;
  padding: 10px;
  background: var(--button-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  color: var(--text-primary);
  cursor: pointer;
  font-size: 16px;
  transition: all 0.2s ease;

  &:hover {
    background: var(--hover-bg);
  }
}

.collapse-toggle {
  padding: 10px;
  background: var(--button-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
  transition: all 0.2s ease;

  &:hover {
    background: var(--hover-bg);
    color: var(--text-primary);
  }
}
</style>
