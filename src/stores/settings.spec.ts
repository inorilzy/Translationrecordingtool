import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useSettingsStore } from './settings'
import * as settingsLib from '../lib/settings'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('../lib/settings', async () => {
  const actual = await vi.importActual<typeof import('../lib/settings')>('../lib/settings')
  return {
    ...actual,
    applyTheme: vi.fn(),
  }
})

const invokeMock = vi.mocked(invoke)

describe('useSettingsStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  describe('default state', () => {
    it('initializes with default settings values', () => {
      const store = useSettingsStore()
      expect(store.apiKey).toBe('')
      expect(store.apiSecret).toBe('')
      expect(store.translationProvider).toBe('youdao')
      expect(store.microsoftTranslatorKey).toBe('')
      expect(store.microsoftTranslatorRegion).toBe('')
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr')
      expect(store.globalShortcut).toBe('Ctrl+Q')
      expect(store.enableTray).toBe(true)
      expect(store.theme).toBe('light')
      expect(store.error).toBe('')
    })
  })

  describe('loadSettings', () => {
    it('applies persisted settings from Rust backend', async () => {
      invokeMock.mockResolvedValue({
        apiKey: 'test-key',
        apiSecret: 'test-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'ms-key',
        microsoftTranslatorRegion: 'eastasia',
        ocrEndpoint: 'http://127.0.0.1:8866/ocr',
        theme: 'dark',
      })

      const store = useSettingsStore()
      await store.loadSettings()

      expect(store.apiKey).toBe('test-key')
      expect(store.apiSecret).toBe('test-secret')
      expect(store.translationProvider).toBe('microsoft')
      expect(store.microsoftTranslatorKey).toBe('ms-key')
      expect(store.microsoftTranslatorRegion).toBe('eastasia')
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr')
      expect(store.theme).toBe('dark')
      expect(store.globalShortcut).toBe('Ctrl+Q') // default for missing fields
      expect(store.enableTray).toBe(true)
    })

    it('falls back to defaults when get_settings rejects', async () => {
      invokeMock.mockRejectedValue(new Error('RPC error'))

      const store = useSettingsStore()
      await store.loadSettings()

      expect(store.apiKey).toBe('')
      expect(store.apiSecret).toBe('')
      expect(store.translationProvider).toBe('youdao')
      expect(store.microsoftTranslatorKey).toBe('')
      expect(store.microsoftTranslatorRegion).toBe('')
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr')
      expect(store.theme).toBe('light')
      expect(store.globalShortcut).toBe('Ctrl+Q')
      expect(store.enableTray).toBe(true)
      expect(store.error).toContain('加载设置失败')
    })

    it('applies theme via DOM when loading settings', async () => {
      invokeMock.mockResolvedValue({ theme: 'one-dark' })

      const store = useSettingsStore()
      await store.loadSettings()

      expect(settingsLib.applyTheme).toHaveBeenCalledWith('one-dark')
    })
  })

  describe('updateApiConfig', () => {
    it('saves new API config and updates local state on success', async () => {
      invokeMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      await store.updateApiConfig({
        apiKey: 'new-key',
        apiSecret: 'new-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'new-ms-key',
        microsoftTranslatorRegion: 'eastasia',
        ocrEndpoint: 'http://127.0.0.1:8866/ocr',
      })

      expect(invokeMock).toHaveBeenCalledWith('update_api_config', {
        apiKey: 'new-key',
        apiSecret: 'new-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'new-ms-key',
        microsoftTranslatorRegion: 'eastasia',
        ocrEndpoint: 'http://127.0.0.1:8866/ocr',
      })
      expect(store.apiKey).toBe('new-key')
      expect(store.apiSecret).toBe('new-secret')
      expect(store.translationProvider).toBe('microsoft')
      expect(store.microsoftTranslatorKey).toBe('new-ms-key')
      expect(store.microsoftTranslatorRegion).toBe('eastasia')
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr')
    })

    it('rethrows error and preserves local state on failure', async () => {
      invokeMock.mockRejectedValue(new Error('backend error'))

      const store = useSettingsStore()
      await expect(store.updateApiConfig({
        apiKey: 'bad-key',
        apiSecret: 'bad-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'bad-ms-key',
        microsoftTranslatorRegion: 'westus',
        ocrEndpoint: 'http://bad.local/ocr',
      })).rejects.toThrow('backend error')

      expect(store.apiKey).toBe('') // unchanged
      expect(store.apiSecret).toBe('') // unchanged
      expect(store.translationProvider).toBe('youdao') // unchanged
      expect(store.microsoftTranslatorKey).toBe('') // unchanged
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr') // unchanged
      expect(store.error).toContain('更新配置失败')
    })
  })

  describe('updateGlobalShortcut', () => {
    it('sends old and new shortcut to backend', async () => {
      invokeMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      store.globalShortcut = 'Ctrl+Q'
      await store.updateGlobalShortcut('Ctrl+Shift+T')

      expect(invokeMock).toHaveBeenCalledWith('update_global_shortcut', {
        oldShortcut: 'Ctrl+Q',
        newShortcut: 'Ctrl+Shift+T',
      })
      expect(store.globalShortcut).toBe('Ctrl+Shift+T')
    })

    it('rolls back local state on failure', async () => {
      invokeMock.mockRejectedValue(new Error('invalid shortcut'))

      const store = useSettingsStore()
      store.globalShortcut = 'Ctrl+Q'
      await expect(store.updateGlobalShortcut('Ctrl+Z')).rejects.toThrow('invalid shortcut')

      expect(store.globalShortcut).toBe('Ctrl+Q') // unchanged
      expect(store.error).toContain('更新快捷键失败')
    })
  })

  describe('updateTrayBehavior', () => {
    it('saves tray behavior and updates local state', async () => {
      invokeMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      await store.updateTrayBehavior(false)

      expect(invokeMock).toHaveBeenCalledWith('update_tray_behavior', { enabled: false })
      expect(store.enableTray).toBe(false)
    })

    it('reverts local state on failure', async () => {
      invokeMock.mockRejectedValue(new Error('tray error'))

      const store = useSettingsStore()
      store.enableTray = true
      await expect(store.updateTrayBehavior(false)).rejects.toThrow('tray error')

      expect(store.enableTray).toBe(true) // unchanged
      expect(store.error).toContain('更新托盘行为失败')
    })
  })

  describe('updateTheme', () => {
    it('saves theme, updates local state, and applies to DOM', async () => {
      invokeMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      await store.updateTheme('github-dark')

      expect(invokeMock).toHaveBeenCalledWith('update_theme', { theme: 'github-dark' })
      expect(store.theme).toBe('github-dark')
      expect(settingsLib.applyTheme).toHaveBeenCalledWith('github-dark')
    })

    it('reverts local state on failure', async () => {
      invokeMock.mockRejectedValue(new Error('theme error'))

      const store = useSettingsStore()
      store.theme = 'light'
      await expect(store.updateTheme('dark')).rejects.toThrow('theme error')

      expect(store.theme).toBe('light') // unchanged
      expect(store.error).toContain('更新主题失败')
    })
  })

  describe('checkOcrService', () => {
    it('checks OCR service with the configured endpoint', async () => {
      invokeMock.mockResolvedValue('Paddle OCR 服务正常')

      const store = useSettingsStore()
      store.ocrEndpoint = 'http://127.0.0.1:8866/ocr'

      const result = await store.checkOcrService()

      expect(invokeMock).toHaveBeenCalledWith('check_ocr_service', {
        ocrEndpoint: 'http://127.0.0.1:8866/ocr',
      })
      expect(result).toBe('Paddle OCR 服务正常')
    })

    it('sets error state when OCR service check fails', async () => {
      invokeMock.mockRejectedValue(new Error('connection refused'))

      const store = useSettingsStore()

      await expect(store.checkOcrService()).rejects.toThrow('connection refused')
      expect(store.error).toContain('OCR 服务检查失败')
    })
  })
})
