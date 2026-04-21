import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  mergeTranslationIntoHistory,
  updateHistoryFavoriteState,
  type TranslationRecord as Translation,
} from './translation-records'
import {
  applyTheme,
  defaultSettings,
  getSettingsSnapshot,
  type AppSettings,
} from '../lib/settings'

export type { Translation }

export const useTranslationStore = defineStore('translation', () => {
  const apiKey = ref(defaultSettings.apiKey)
  const apiSecret = ref(defaultSettings.apiSecret)
  const globalShortcut = ref(defaultSettings.globalShortcut)
  const enableTray = ref(defaultSettings.enableTray)
  const theme = ref(defaultSettings.theme)
  const currentTranslation = ref<Translation | null>(null)
  const history = ref<Translation[]>([])
  const loading = ref(false)
  const error = ref('')

  function applySettings(settings: AppSettings) {
    apiKey.value = settings.apiKey
    apiSecret.value = settings.apiSecret
    globalShortcut.value = settings.globalShortcut
    enableTray.value = settings.enableTray
    theme.value = settings.theme
    applyTheme(settings.theme)
  }

  async function loadSettings() {
    try {
      const persistedSettings = await getSettingsSnapshot()
      applySettings(persistedSettings)
    } catch (e) {
      console.error('加载设置失败，使用默认配置:', e)
      applySettings(defaultSettings)
    }
  }

  async function initDatabase() {
    try {
      await loadSettings()
      await loadHistory()
    } catch (e) {
      error.value = `初始化失败: ${e}`
    }
  }

  async function translateFromClipboard() {
    loading.value = true
    error.value = ''

    try {
      const result = await invoke<Translation>('translate_from_clipboard', {
        appKey: apiKey.value,
        appSecret: apiSecret.value,
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = `翻译失败: ${e}`
    } finally {
      loading.value = false
    }
  }

  async function translateText(text: string) {
    loading.value = true
    error.value = ''

    try {
      const result = await invoke<Translation>('translate_text', {
        text,
        appKey: apiKey.value,
        appSecret: apiSecret.value,
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = `翻译失败: ${e}`
    } finally {
      loading.value = false
    }
  }

  async function loadHistory() {
    try {
      history.value = await invoke<Translation[]>('load_history')
    } catch (e) {
      error.value = `加载历史记录失败: ${e}`
    }
  }

  async function loadFavorites() {
    try {
      return await invoke<Translation[]>('load_favorites')
    } catch (e) {
      error.value = `加载收藏列表失败: ${e}`
      return []
    }
  }

  async function getTranslationById(id: number) {
    try {
      return await invoke<Translation>('get_translation_by_id', { id })
    } catch (e) {
      error.value = `查询翻译记录失败: ${e}`
      return null
    }
  }

  async function toggleFavorite(id: number, isFavorite: boolean) {
    try {
      await invoke('toggle_favorite', {
        id,
        isFavorite,
      })

      history.value = updateHistoryFavoriteState(history.value, id, isFavorite ? 1 : 0)
      if (currentTranslation.value?.id === id) {
        currentTranslation.value = {
          ...currentTranslation.value,
          is_favorite: isFavorite ? 1 : 0,
        }
      }
    } catch (e) {
      error.value = `更新收藏状态失败: ${e}`
    }
  }

  async function updateGlobalShortcut(newShortcut: string) {
    try {
      await invoke('update_global_shortcut', {
        oldShortcut: globalShortcut.value,
        newShortcut
      })
      globalShortcut.value = newShortcut
    } catch (e) {
      error.value = `更新快捷键失败: ${e}`
      throw e
    }
  }

  async function updateApiConfig(key: string, secret: string) {
    try {
      await invoke('update_api_config', {
        apiKey: key,
        apiSecret: secret
      })
      apiKey.value = key
      apiSecret.value = secret
    } catch (e) {
      error.value = `更新配置失败: ${e}`
      throw e
    }
  }

  async function updateTrayBehavior(enabled: boolean) {
    try {
      await invoke('update_tray_behavior', {
        enabled,
      })
      enableTray.value = enabled
    } catch (e) {
      error.value = `更新托盘行为失败: ${e}`
      throw e
    }
  }

  async function updateTheme(nextTheme: string) {
    try {
      await invoke('update_theme', {
        theme: nextTheme,
      })
      theme.value = nextTheme
      applyTheme(nextTheme)
    } catch (e) {
      error.value = `更新主题失败: ${e}`
      throw e
    }
  }

  return {
    apiKey,
    apiSecret,
    globalShortcut,
    enableTray,
    theme,
    currentTranslation,
    history,
    loading,
    error,
    initDatabase,
    translateFromClipboard,
    translateText,
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
    loadSettings,
    updateGlobalShortcut,
    updateApiConfig,
    updateTrayBehavior,
    updateTheme,
  }
})
