<script setup lang="ts">
import { computed, h } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { Component } from 'vue'
import {
  FileText,
  History,
  Languages,
  Settings,
  Star,
  Info,
} from '@lucide/vue'

interface NavItem {
  key: string
  label: string
  path: string
  icon: Component
}

const route = useRoute()
const router = useRouter()

const navItems: NavItem[] = [
  { key: 'translate', label: '翻译', path: '/translate', icon: Languages },
  { key: 'history', label: '历史', path: '/history', icon: History },
  { key: 'favorites', label: '收藏', path: '/favorites', icon: Star },
  { key: 'logs', label: '日志', path: '/logs', icon: FileText },
  { key: 'settings', label: '设置', path: '/settings', icon: Settings },
]

const menuOptions = computed(() => navItems.map((item) => ({
  key: item.key,
  label: item.label,
  icon: () => h(item.icon, { size: 21, strokeWidth: 1.9 }),
})))

const activeKey = computed(() => {
  const matchedItem = navItems.find((item) => route.path === item.path || route.path.startsWith(`${item.path}/`))
  return matchedItem?.key ?? 'translate'
})

function handleMenuUpdate(key: string) {
  const item = navItems.find((navItem) => navItem.key === key)
  if (item && item.path !== route.path) {
    router.push(item.path)
  }
}
</script>

<template>
  <n-layout has-sider class="app-shell">
    <n-layout-sider
      class="app-sidebar"
      :width="212"
      :native-scrollbar="false"
      bordered
    >
      <div class="brand-zone">
        <div class="brand-mark">
          <Languages :size="20" :stroke-width="2.1" />
        </div>
        <div class="brand-copy">
          <div class="brand-title">选词翻译</div>
          <div class="brand-kicker">DESKTOP</div>
        </div>
      </div>

      <n-menu
        :value="activeKey"
        :options="menuOptions"
        :indent="16"
        class="shell-menu"
        @update:value="handleMenuUpdate"
      />

      <div class="sidebar-footer">
        <button class="about-button" type="button">
          <Settings :size="20" :stroke-width="1.9" />
          <span>关于</span>
          <Info :size="17" :stroke-width="1.9" class="about-info" />
        </button>
      </div>
    </n-layout-sider>

    <n-layout-content class="app-main">
      <slot />
    </n-layout-content>
  </n-layout>
</template>

<style scoped>
.app-shell {
  --app-sidebar-width: 212px;
  min-height: 100vh;
  color: var(--color-text-primary);
  font-family: var(--font-family-ui);
  background:
    linear-gradient(180deg, var(--color-app-shell-bg) 0%, var(--color-app-shell-bg-end) 100%);
}

.app-sidebar {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  z-index: 20;
  height: 100vh;
  width: var(--app-sidebar-width) !important;
  background: var(--color-app-sidebar-bg);
  border-right: 1px solid var(--color-app-panel-border);
}

.brand-zone {
  height: 84px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 18px;
  border-bottom: 1px solid var(--color-app-panel-border);
}

.brand-mark {
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-on-primary);
  border-radius: 10px;
  background: var(--color-app-accent-strong);
  box-shadow: var(--shadow-app-brand);
  flex-shrink: 0;
}

.brand-copy {
  min-width: 0;
}

.brand-title {
  font-family: var(--font-family-display);
  font-size: 15px;
  font-weight: 650;
  letter-spacing: -0.02em;
  color: var(--color-app-text-strong);
  line-height: 1.2;
}

.brand-kicker {
  margin-top: 2px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.14em;
  color: var(--color-app-text-muted);
}

.shell-menu {
  padding: 14px 10px 0;
  background: transparent;
}

.sidebar-footer {
  position: absolute;
  left: 12px;
  right: 12px;
  bottom: 18px;
}

.about-button {
  width: 100%;
  height: 40px;
  display: grid;
  grid-template-columns: 20px 1fr 16px;
  align-items: center;
  gap: 9px;
  padding: 0 13px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  color: var(--color-app-text-soft);
  font: inherit;
  text-align: left;
  cursor: default;
}

.about-info {
  opacity: 0.8;
}

.app-main {
  width: calc(100vw - var(--app-sidebar-width));
  margin-left: var(--app-sidebar-width);
  min-height: 100vh;
  background: transparent;
}

:deep(.n-layout-sider-scroll-container) {
  position: relative;
  height: 100vh;
}

:deep(.n-menu-item) {
  margin: 4px 0;
  height: 40px;
}

:deep(.n-menu-item-content) {
  height: 40px;
  padding-left: 14px !important;
  border-radius: 10px;
  color: var(--color-app-icon-muted);
  font-size: 14px;
}

:deep(.n-menu-item-content::before) {
  left: 0;
  right: 0;
  border-radius: 10px;
}

:deep(.n-menu-item-content.n-menu-item-content--selected) {
  color: var(--color-app-accent-strong);
  font-weight: 650;
}

:deep(.n-menu-item-content.n-menu-item-content--selected::before) {
  background: var(--color-app-accent-tint);
}

:deep(.n-menu-item-content.n-menu-item-content--selected::after) {
  content: '';
  position: absolute;
  left: -10px;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 999px;
  background: var(--color-app-accent-strong);
}

:deep(.n-menu-item-content-header) {
  padding-left: 8px;
}
</style>
