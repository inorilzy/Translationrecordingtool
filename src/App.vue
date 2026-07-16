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
import { computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'
import { useTranslationStore, type OcrSourceTextPayload } from './stores/translation'
import { useSettingsStore } from './stores/settings'
import AppShell from './components/AppShell.vue'

const translationStore = useTranslationStore()
const settingsStore = useSettingsStore()
const router = useRouter()
const windowLabel = resolveCurrentWindowLabel(getCurrentWebviewWindow)
const usesAppShell = computed(() => windowLabel !== 'popup' && windowLabel !== 'screenshot-selection')
const naiveTheme = computed(() => (settingsStore.theme === 'dark' ? darkTheme : null))
const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const isDark = settingsStore.theme === 'dark'
  return {
    common: {
      primaryColor: isDark ? '#2DD4BF' : '#0F766E',
      primaryColorHover: isDark ? '#5EEAD4' : '#0D9488',
      primaryColorPressed: isDark ? '#14B8A6' : '#115E59',
      primaryColorSuppl: isDark ? '#2DD4BF' : '#0F766E',
      borderRadius: '10px',
      fontFamily: 'var(--font-family-ui)',
    },
    Select: {
      peers: {
        InternalSelection: {
          borderRadius: '10px',
        },
      },
    },
  }
})
let unlistenOcrSourceText: (() => void) | null = null
let unlistenNavigateToTranslate: (() => void) | null = null

onMounted(async () => {
  await runStartupLoad(windowLabel, settingsStore, translationStore)
  if (windowLabel === 'main') {
    unlistenOcrSourceText = await listen<OcrSourceTextPayload>('ocr-source-text', (event) => {
      translationStore.acceptOcrSourceText(event.payload)
    })
    unlistenNavigateToTranslate = await listen('navigate-to-translate', async () => {
      if (router.currentRoute.value.path !== '/translate') {
        await router.push('/translate')
      }
    })
  }
})

onUnmounted(() => {
  unlistenOcrSourceText?.()
  unlistenNavigateToTranslate?.()
})
</script>

<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="themeOverrides">
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
