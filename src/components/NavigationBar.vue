<script setup lang="ts">
import { useRouter, useRoute } from 'vue-router'

const router = useRouter()
const route = useRoute()

function isActive(path: string) {
  return route.path === path
}

function navigate(path: string) {
  router.push(path)
}
</script>

<template>
  <nav class="navigation-bar">
    <button
      @click="navigate('/history')"
      :class="['nav-item', { active: isActive('/history') }]"
    >
      历史
    </button>
    <button
      @click="navigate('/favorites')"
      :class="['nav-item', { active: isActive('/favorites') }]"
    >
      收藏
    </button>
    <button
      @click="navigate('/logs')"
      :class="['nav-item', { active: isActive('/logs') }]"
    >
      日志
    </button>
    <button
      @click="navigate('/settings')"
      :class="['nav-item', { active: isActive('/settings') }]"
    >
      设置
    </button>
  </nav>
</template>

<style scoped>
.navigation-bar {
  display: flex;
  gap: var(--spacing-md);
  padding: var(--spacing-md) 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: var(--spacing-lg);
}

.nav-item {
  padding: var(--spacing-sm) var(--spacing-md);
  background: none;
  border: none;
  color: var(--color-text-secondary);
  font-size: var(--font-size-md);
  cursor: pointer;
  transition: all 0.2s;
  border-bottom: 2px solid transparent;
  font-family: inherit;
}

.nav-item:hover {
  color: var(--color-primary);
}

.nav-item.active {
  color: var(--color-primary);
  border-bottom-color: var(--color-primary);
  font-weight: 500;
}
</style>
