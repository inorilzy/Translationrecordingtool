import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  applyTheme,
  defaultSettings,
  getSettingsSnapshot,
  normalizeTheme,
  type AppSettings,
} from '../lib/settings'

export interface OcrServiceStatus {
  running: boolean
  message: string
  lastError?: string | null
  engine: string
  modelProfile: string
  modelDir?: string | null
  preloadOnStartup: boolean
  ppocrVersion: string
  onnxruntimeVersion: string
  lang: string
  device: string
}

type ApiConfigPatch = Pick<
  AppSettings,
  | 'apiKey'
  | 'apiSecret'
  | 'translationProvider'
  | 'microsoftTranslatorKey'
  | 'microsoftTranslatorRegion'
  | 'googleApiKey'
  | 'ocrEndpoint'
  | 'ocrEngine'
  | 'ocrModelProfile'
  | 'ocrPreloadOnStartup'
>

function cloneSettings(settings: AppSettings): AppSettings {
  return { ...settings }
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<AppSettings>(cloneSettings(defaultSettings))
  const error = ref('')

  function field<K extends keyof AppSettings>(key: K) {
    return computed({
      get: () => settings.value[key],
      set: (value: AppSettings[K]) => {
        settings.value = {
          ...settings.value,
          [key]: value,
        }
      },
    })
  }

  const apiKey = field('apiKey')
  const apiSecret = field('apiSecret')
  const translationProvider = field('translationProvider')
  const microsoftTranslatorKey = field('microsoftTranslatorKey')
  const microsoftTranslatorRegion = field('microsoftTranslatorRegion')
  const googleApiKey = field('googleApiKey')
  const ocrEndpoint = field('ocrEndpoint')
  const ocrEngine = field('ocrEngine')
  const ocrModelProfile = field('ocrModelProfile')
  const ocrPreloadOnStartup = field('ocrPreloadOnStartup')
  const globalShortcut = field('globalShortcut')
  const screenshotShortcut = field('screenshotShortcut')
  const enableTray = field('enableTray')
  const theme = field('theme')

  function applySettings(next: AppSettings) {
    settings.value = cloneSettings(next)
    applyTheme(next.theme)
  }

  function createDraft(): AppSettings {
    return cloneSettings(settings.value)
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

  async function updateApiConfig(patch: ApiConfigPatch) {
    try {
      await invoke('update_api_config', {
        apiKey: patch.apiKey,
        apiSecret: patch.apiSecret,
        translationProvider: patch.translationProvider,
        microsoftTranslatorKey: patch.microsoftTranslatorKey,
        microsoftTranslatorRegion: patch.microsoftTranslatorRegion,
        googleApiKey: patch.googleApiKey,
        ocrEndpoint: patch.ocrEndpoint,
        ocrEngine: patch.ocrEngine,
        ocrModelProfile: patch.ocrModelProfile,
        ocrPreloadOnStartup: patch.ocrPreloadOnStartup,
      })
      settings.value = {
        ...settings.value,
        ...patch,
      }
    } catch (e) {
      error.value = `更新配置失败: ${e}`
      throw e
    }
  }

  async function updateGlobalShortcut(newShortcut: string) {
    const oldShortcut = settings.value.globalShortcut
    try {
      await invoke('update_global_shortcut', {
        oldShortcut,
        newShortcut,
      })
      settings.value = {
        ...settings.value,
        globalShortcut: newShortcut,
      }
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
      settings.value = {
        ...settings.value,
        enableTray: enabled,
      }
    } catch (e) {
      error.value = `更新托盘行为失败: ${e}`
      throw e
    }
  }

  async function updateTheme(nextTheme: string) {
    const themeName = normalizeTheme(nextTheme)
    try {
      await invoke('update_theme', {
        theme: themeName,
      })
      settings.value = {
        ...settings.value,
        theme: themeName,
      }
      applyTheme(themeName)
    } catch (e) {
      error.value = `更新主题失败: ${e}`
      throw e
    }
  }

  async function checkOcrService() {
    try {
      return await invoke<string>('check_ocr_service')
    } catch (e) {
      error.value = `OCR 服务检查失败: ${e}`
      throw e
    }
  }

  async function updateScreenshotShortcut(newShortcut: string) {
    const oldShortcut = settings.value.screenshotShortcut
    try {
      await invoke('update_screenshot_shortcut', {
        oldShortcut,
        newShortcut,
      })
      settings.value = {
        ...settings.value,
        screenshotShortcut: newShortcut,
      }
    } catch (e) {
      error.value = `更新截图快捷键失败: ${e}`
      throw e
    }
  }

  async function getOcrServiceStatus() {
    try {
      return await invoke<OcrServiceStatus>('get_ocr_service_status')
    } catch (e) {
      error.value = `读取 OCR 服务状态失败: ${e}`
      throw e
    }
  }

  async function warmupOcrService() {
    try {
      return await invoke<string>('warmup_ocr_service')
    } catch (e) {
      error.value = `预热 OCR 服务失败: ${e}`
      throw e
    }
  }

  async function restartOcrService() {
    try {
      return await invoke<string>('restart_ocr_service')
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
    settings,
    apiKey,
    apiSecret,
    translationProvider,
    microsoftTranslatorKey,
    microsoftTranslatorRegion,
    googleApiKey,
    ocrEndpoint,
    ocrEngine,
    ocrModelProfile,
    ocrPreloadOnStartup,
    globalShortcut,
    screenshotShortcut,
    enableTray,
    theme,
    error,
    createDraft,
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
