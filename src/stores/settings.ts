import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  applyTheme,
  defaultSettings,
  getSettingsSnapshot,
  type AppSettings,
} from '../lib/settings'

export const useSettingsStore = defineStore('settings', () => {
  const apiKey = ref(defaultSettings.apiKey)
  const apiSecret = ref(defaultSettings.apiSecret)
  const translationProvider = ref(defaultSettings.translationProvider)
  const microsoftTranslatorKey = ref(defaultSettings.microsoftTranslatorKey)
  const microsoftTranslatorRegion = ref(defaultSettings.microsoftTranslatorRegion)
  const ocrEndpoint = ref(defaultSettings.ocrEndpoint)
  const globalShortcut = ref(defaultSettings.globalShortcut)
  const enableTray = ref(defaultSettings.enableTray)
  const theme = ref(defaultSettings.theme)
  const error = ref('')

  function applySettings(settings: AppSettings) {
    apiKey.value = settings.apiKey
    apiSecret.value = settings.apiSecret
    translationProvider.value = settings.translationProvider
    microsoftTranslatorKey.value = settings.microsoftTranslatorKey
    microsoftTranslatorRegion.value = settings.microsoftTranslatorRegion
    ocrEndpoint.value = settings.ocrEndpoint
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
      error.value = `加载设置失败: ${e}`
      applySettings(defaultSettings)
    }
  }

  async function updateApiConfig(settings: Pick<AppSettings,
    'apiKey'
    | 'apiSecret'
    | 'translationProvider'
    | 'microsoftTranslatorKey'
    | 'microsoftTranslatorRegion'
    | 'ocrEndpoint'
  >) {
    try {
      await invoke('update_api_config', {
        apiKey: settings.apiKey,
        apiSecret: settings.apiSecret,
        translationProvider: settings.translationProvider,
        microsoftTranslatorKey: settings.microsoftTranslatorKey,
        microsoftTranslatorRegion: settings.microsoftTranslatorRegion,
        ocrEndpoint: settings.ocrEndpoint,
      })
      apiKey.value = settings.apiKey
      apiSecret.value = settings.apiSecret
      translationProvider.value = settings.translationProvider
      microsoftTranslatorKey.value = settings.microsoftTranslatorKey
      microsoftTranslatorRegion.value = settings.microsoftTranslatorRegion
      ocrEndpoint.value = settings.ocrEndpoint
    } catch (e) {
      error.value = `更新配置失败: ${e}`
      throw e
    }
  }

  async function updateGlobalShortcut(newShortcut: string) {
    try {
      await invoke('update_global_shortcut', {
        oldShortcut: globalShortcut.value,
        newShortcut,
      })
      globalShortcut.value = newShortcut
    } catch (e) {
      error.value = `更新快捷键失败: ${e}`
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

  async function checkOcrService(endpoint = ocrEndpoint.value) {
    try {
      return await invoke<string>('check_ocr_service', {
        ocrEndpoint: endpoint,
      })
    } catch (e) {
      error.value = `OCR 服务检查失败: ${e}`
      throw e
    }
  }

  return {
    apiKey,
    apiSecret,
    translationProvider,
    microsoftTranslatorKey,
    microsoftTranslatorRegion,
    ocrEndpoint,
    globalShortcut,
    enableTray,
    theme,
    error,
    loadSettings,
    updateApiConfig,
    updateGlobalShortcut,
    updateTrayBehavior,
    updateTheme,
    checkOcrService,
  }
})
