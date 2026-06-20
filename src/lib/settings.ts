import { invoke } from '@tauri-apps/api/core'

export interface AppSettings {
  apiKey: string
  apiSecret: string
  translationProvider: string
  microsoftTranslatorKey: string
  microsoftTranslatorRegion: string
  ocrEndpoint: string
  ocrEngine: string
  ocrModelProfile: string
  ocrPreloadOnStartup: boolean
  globalShortcut: string
  screenshotShortcut: string
  enableTray: boolean
  theme: string
}

export const defaultSettings: AppSettings = {
  apiKey: '',
  apiSecret: '',
  translationProvider: 'youdao',
  microsoftTranslatorKey: '',
  microsoftTranslatorRegion: '',
  ocrEndpoint: 'http://127.0.0.1:8866/ocr',
  ocrEngine: 'paddleocr',
  ocrModelProfile: 'standard',
  ocrPreloadOnStartup: true,
  globalShortcut: 'Ctrl+Q',
  screenshotShortcut: 'Ctrl+Shift+Q',
  enableTray: true,
  theme: 'light',
}

export function normalizeSettings(settings?: Partial<AppSettings> | null): AppSettings {
  return {
    ...defaultSettings,
    ...settings,
  }
}

export async function getSettingsSnapshot() {
  const settings = await invoke<Partial<AppSettings>>('get_settings')
  return normalizeSettings(settings)
}

export function isDefaultSettings(settings: AppSettings) {
  return (
    settings.apiKey === defaultSettings.apiKey
    && settings.apiSecret === defaultSettings.apiSecret
    && settings.translationProvider === defaultSettings.translationProvider
    && settings.microsoftTranslatorKey === defaultSettings.microsoftTranslatorKey
    && settings.microsoftTranslatorRegion === defaultSettings.microsoftTranslatorRegion
    && settings.ocrEndpoint === defaultSettings.ocrEndpoint
    && settings.ocrEngine === defaultSettings.ocrEngine
    && settings.ocrModelProfile === defaultSettings.ocrModelProfile
    && settings.ocrPreloadOnStartup === defaultSettings.ocrPreloadOnStartup
    && settings.globalShortcut === defaultSettings.globalShortcut
    && settings.screenshotShortcut === defaultSettings.screenshotShortcut
    && settings.enableTray === defaultSettings.enableTray
    && settings.theme === defaultSettings.theme
  )
}

export function applyTheme(theme: string) {
  document.documentElement.setAttribute('data-theme', theme)
}
