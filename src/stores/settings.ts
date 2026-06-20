import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  applyTheme,
  defaultSettings,
  getSettingsSnapshot,
  type AppSettings,
} from '../lib/settings'

export interface OcrServiceStatus {
  running: boolean
  endpoint: string
  message: string
  lastError?: string | null
  engine: string
  modelProfile: string
  modelDir?: string | null
  sidecarPath?: string | null
  logPath?: string | null
  preloadOnStartup: boolean
  rapidocrVersion: string
  paddleocrVersion: string
  ppocrVersion: string
  onnxruntimeVersion: string
  lang: string
  device: string
}

export const useSettingsStore = defineStore('settings', () => {
  const apiKey = ref(defaultSettings.apiKey)
  const apiSecret = ref(defaultSettings.apiSecret)
  const translationProvider = ref(defaultSettings.translationProvider)
  const microsoftTranslatorKey = ref(defaultSettings.microsoftTranslatorKey)
  const microsoftTranslatorRegion = ref(defaultSettings.microsoftTranslatorRegion)
  const ocrEndpoint = ref(defaultSettings.ocrEndpoint)
  const ocrEngine = ref(defaultSettings.ocrEngine)
  const ocrModelProfile = ref(defaultSettings.ocrModelProfile)
  const ocrPreloadOnStartup = ref(defaultSettings.ocrPreloadOnStartup)
  const globalShortcut = ref(defaultSettings.globalShortcut)
  const screenshotShortcut = ref(defaultSettings.screenshotShortcut)
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
    ocrEngine.value = settings.ocrEngine
    ocrModelProfile.value = settings.ocrModelProfile
    ocrPreloadOnStartup.value = settings.ocrPreloadOnStartup
    globalShortcut.value = settings.globalShortcut
    screenshotShortcut.value = settings.screenshotShortcut
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
    | 'ocrEngine'
    | 'ocrModelProfile'
    | 'ocrPreloadOnStartup'
  >) {
    try {
      await invoke('update_api_config', {
        apiKey: settings.apiKey,
        apiSecret: settings.apiSecret,
        translationProvider: settings.translationProvider,
        microsoftTranslatorKey: settings.microsoftTranslatorKey,
        microsoftTranslatorRegion: settings.microsoftTranslatorRegion,
        ocrEndpoint: settings.ocrEndpoint,
        ocrEngine: settings.ocrEngine,
        ocrModelProfile: settings.ocrModelProfile,
        ocrPreloadOnStartup: settings.ocrPreloadOnStartup,
      })
      apiKey.value = settings.apiKey
      apiSecret.value = settings.apiSecret
      translationProvider.value = settings.translationProvider
      microsoftTranslatorKey.value = settings.microsoftTranslatorKey
      microsoftTranslatorRegion.value = settings.microsoftTranslatorRegion
      ocrEndpoint.value = settings.ocrEndpoint
      ocrEngine.value = settings.ocrEngine
      ocrModelProfile.value = settings.ocrModelProfile
      ocrPreloadOnStartup.value = settings.ocrPreloadOnStartup
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

  async function updateScreenshotShortcut(newShortcut: string) {
    try {
      await invoke('update_screenshot_shortcut', {
        oldShortcut: screenshotShortcut.value,
        newShortcut,
      })
      screenshotShortcut.value = newShortcut
    } catch (e) {
      error.value = `更新截图快捷键失败: ${e}`
      throw e
    }
  }

  async function getOcrServiceStatus(endpoint = ocrEndpoint.value) {
    try {
      return await invoke<OcrServiceStatus>('get_ocr_service_status', {
        ocrEndpoint: endpoint,
      })
    } catch (e) {
      error.value = `读取 OCR 服务状态失败: ${e}`
      throw e
    }
  }

  async function warmupOcrService(endpoint = ocrEndpoint.value) {
    try {
      return await invoke<string>('warmup_ocr_service', {
        ocrEndpoint: endpoint,
      })
    } catch (e) {
      error.value = `预热 OCR 服务失败: ${e}`
      throw e
    }
  }

  async function restartOcrService(endpoint = ocrEndpoint.value) {
    try {
      return await invoke<string>('restart_ocr_service', {
        ocrEndpoint: endpoint,
      })
    } catch (e) {
      error.value = `重启 OCR 服务失败: ${e}`
      throw e
    }
  }

  async function revealOcrLog() {
    try {
      const path = await invoke<string>('get_ocr_log_path')
      await revealItemInDir(path)
      return path
    } catch (e) {
      error.value = `打开 OCR 日志失败: ${e}`
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
    ocrEngine,
    ocrModelProfile,
    ocrPreloadOnStartup,
    globalShortcut,
    screenshotShortcut,
    enableTray,
    theme,
    error,
    loadSettings,
    updateApiConfig,
    updateGlobalShortcut,
    updateScreenshotShortcut,
    updateTrayBehavior,
    updateTheme,
    checkOcrService,
    getOcrServiceStatus,
    warmupOcrService,
    restartOcrService,
    revealOcrLog,
  }
})
