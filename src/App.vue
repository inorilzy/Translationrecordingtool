<script setup lang="ts">
import { onMounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useTranslationStore } from './stores/translation'

const store = useTranslationStore()
const windowLabel = getCurrentWebviewWindow().label

onMounted(() => {
  if (windowLabel === 'main') {
    store.initDatabase()
  }
})
</script>

<template>
  <router-view />
</template>

<style>
@import './styles/design-tokens.css';
@import './styles/global.css';

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html,
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
  transition: background-color 0.2s, color 0.2s;
}

#app {
  min-height: 100vh;
  background-color: var(--color-bg-primary);
}
</style>
