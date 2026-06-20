<script lang="ts">
export async function runStartupLoad(
  windowLabel: string,
  settingsStore: { loadSettings: () => Promise<void> },
  translationStore: { loadHistory: () => Promise<void> },
) {
  if (windowLabel === 'main') {
    await settingsStore.loadSettings()
    await translationStore.loadHistory()
  }
}

export function resolveCurrentWindowLabel(
  getWindow: () => { label: string },
  fallback = 'browser',
) {
  try {
    return getWindow().label
  } catch {
    return fallback
  }
}
</script>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useTranslationStore } from './stores/translation'
import { useSettingsStore } from './stores/settings'
import AppShell from './components/AppShell.vue'

const translationStore = useTranslationStore()
const settingsStore = useSettingsStore()
const windowLabel = resolveCurrentWindowLabel(getCurrentWebviewWindow)
const usesAppShell = computed(() => windowLabel !== 'popup' && windowLabel !== 'screenshot-selection')

onMounted(async () => {
  await runStartupLoad(windowLabel, settingsStore, translationStore)
})
</script>

<template>
  <n-config-provider>
    <n-message-provider>
      <AppShell v-if="usesAppShell">
        <router-view />
      </AppShell>
      <router-view v-else />
    </n-message-provider>
  </n-config-provider>
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
