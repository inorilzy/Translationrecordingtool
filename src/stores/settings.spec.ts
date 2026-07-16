import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useSettingsStore } from './settings'
import * as settingsLib from '../lib/settings'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: vi.fn(),
}))

vi.mock('../lib/settings', async () => {
  const actual = await vi.importActual<typeof import('../lib/settings')>('../lib/settings')
  return {
    ...actual,
    applyTheme: vi.fn(),
  }
})

const invokeMock = vi.mocked(invoke)
const revealItemInDirMock = vi.mocked(revealItemInDir)

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
      expect(store.ocrEngine).toBe('native_onnx')
      expect(store.ocrModelProfile).toBe('small')
      expect(store.ocrPreloadOnStartup).toBe(true)
      expect(store.globalShortcut).toBe('Ctrl+Q')
      expect(store.screenshotShortcut).toBe('Ctrl+Shift+Q')
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
        ocrEngine: 'native_onnx',
        ocrModelProfile: 'tiny',
        ocrPreloadOnStartup: false,
        screenshotShortcut: 'Ctrl+Shift+S',
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
      expect(store.ocrEngine).toBe('native_onnx')
      expect(store.ocrModelProfile).toBe('tiny')
      expect(store.ocrPreloadOnStartup).toBe(false)
      expect(store.screenshotShortcut).toBe('Ctrl+Shift+S')
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
      expect(store.ocrEngine).toBe('native_onnx')
      expect(store.ocrModelProfile).toBe('small')
      expect(store.ocrPreloadOnStartup).toBe(true)
      expect(store.theme).toBe('light')
      expect(store.globalShortcut).toBe('Ctrl+Q')
      expect(store.screenshotShortcut).toBe('Ctrl+Shift+Q')
      expect(store.enableTray).toBe(true)
      expect(store.error).toContain('加载设置失败')
    })

    it('applies theme via DOM when loading settings', async () => {
      invokeMock.mockResolvedValue({ theme: 'dark' })

      const store = useSettingsStore()
      await store.loadSettings()

      expect(settingsLib.applyTheme).toHaveBeenCalledWith('dark')
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
        ocrEngine: 'native_onnx',
        ocrModelProfile: 'small',
        ocrPreloadOnStartup: false,
      })

      expect(invokeMock).toHaveBeenCalledWith('update_api_config', {
        apiKey: 'new-key',
        apiSecret: 'new-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'new-ms-key',
        microsoftTranslatorRegion: 'eastasia',
        ocrEndpoint: 'http://127.0.0.1:8866/ocr',
        ocrEngine: 'native_onnx',
        ocrModelProfile: 'small',
        ocrPreloadOnStartup: false,
      })
      expect(store.apiKey).toBe('new-key')
      expect(store.apiSecret).toBe('new-secret')
      expect(store.translationProvider).toBe('microsoft')
      expect(store.microsoftTranslatorKey).toBe('new-ms-key')
      expect(store.microsoftTranslatorRegion).toBe('eastasia')
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr')
      expect(store.ocrEngine).toBe('native_onnx')
      expect(store.ocrModelProfile).toBe('small')
      expect(store.ocrPreloadOnStartup).toBe(false)
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
        ocrEngine: 'native_onnx',
        ocrModelProfile: 'tiny',
        ocrPreloadOnStartup: false,
      })).rejects.toThrow('backend error')

      expect(store.apiKey).toBe('') // unchanged
      expect(store.apiSecret).toBe('') // unchanged
      expect(store.translationProvider).toBe('youdao') // unchanged
      expect(store.microsoftTranslatorKey).toBe('') // unchanged
      expect(store.ocrEndpoint).toBe('http://127.0.0.1:8866/ocr') // unchanged
      expect(store.ocrModelProfile).toBe('small') // unchanged
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
      await store.updateTheme('dark')

      expect(invokeMock).toHaveBeenCalledWith('update_theme', { theme: 'dark' })
      expect(store.theme).toBe('dark')
      expect(settingsLib.applyTheme).toHaveBeenCalledWith('dark')
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
    it('checks OCR service using backend-managed settings', async () => {
      invokeMock.mockResolvedValue('PaddleOCR 服务正常')

      const store = useSettingsStore()
      store.ocrEndpoint = 'http://must-not-be-sent/ocr'

      const result = await store.checkOcrService()

      expect(invokeMock).toHaveBeenCalledWith('check_ocr_service')
      expect(result).toBe('PaddleOCR 服务正常')
    })

    it('sets error state when OCR service check fails', async () => {
      invokeMock.mockRejectedValue(new Error('connection refused'))

      const store = useSettingsStore()

      await expect(store.checkOcrService()).rejects.toThrow('connection refused')
      expect(store.error).toContain('OCR 服务检查失败')
    })
  })

  describe('updateScreenshotShortcut', () => {
    it('sends old and new screenshot shortcut to backend', async () => {
      invokeMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      store.screenshotShortcut = 'Ctrl+Shift+Q'
      await store.updateScreenshotShortcut('Ctrl+Shift+S')

      expect(invokeMock).toHaveBeenCalledWith('update_screenshot_shortcut', {
        oldShortcut: 'Ctrl+Shift+Q',
        newShortcut: 'Ctrl+Shift+S',
      })
      expect(store.screenshotShortcut).toBe('Ctrl+Shift+S')
    })

    it('rolls back local state on failure', async () => {
      invokeMock.mockRejectedValue(new Error('invalid screenshot shortcut'))

      const store = useSettingsStore()
      store.screenshotShortcut = 'Ctrl+Shift+Q'
      await expect(store.updateScreenshotShortcut('Ctrl+Alt+S')).rejects.toThrow('invalid screenshot shortcut')

      expect(store.screenshotShortcut).toBe('Ctrl+Shift+Q')
      expect(store.error).toContain('更新截图快捷键失败')
    })
  })

  describe('getOcrServiceStatus', () => {
    it('loads OCR service status from backend', async () => {
      const status = {
        running: true,
        endpoint: 'http://127.0.0.1:8866/ocr',
        message: 'PaddleOCR 服务正常',
        lastError: null,
        engine: 'native_onnx',
        modelProfile: 'small',
        modelDir: null,
        sidecarPath: null,
        logPath: 'C:/logs/paddle-ocr-service.log',
        preloadOnStartup: true,
        rapidocrVersion: '1.4.4',
        paddleocrVersion: '3.7.0',
        ppocrVersion: 'PP-OCRv6',
        onnxruntimeVersion: '1.27.0',
        lang: 'ch',
        device: 'cpu',
      }
      invokeMock.mockResolvedValue(status)

      const store = useSettingsStore()
      const result = await store.getOcrServiceStatus()

      expect(invokeMock).toHaveBeenCalledWith('get_ocr_service_status')
      expect(result).toEqual(status)
    })

    it('sets error state when OCR service status fails', async () => {
      invokeMock.mockRejectedValue(new Error('backend unavailable'))

      const store = useSettingsStore()

      await expect(store.getOcrServiceStatus()).rejects.toThrow('backend unavailable')
      expect(store.error).toContain('读取 OCR 服务状态失败')
    })
  })

  describe('OCR service controls', () => {
    it('warms up OCR service using backend-managed settings', async () => {
      invokeMock.mockResolvedValue('OCR 预热完成')

      const store = useSettingsStore()
      const result = await store.warmupOcrService()

      expect(invokeMock).toHaveBeenCalledWith('warmup_ocr_service')
      expect(result).toBe('OCR 预热完成')
    })

    it('restarts OCR service using backend-managed settings', async () => {
      invokeMock.mockResolvedValue('OCR 预热完成')

      const store = useSettingsStore()
      const result = await store.restartOcrService()

      expect(invokeMock).toHaveBeenCalledWith('restart_ocr_service')
      expect(result).toBe('OCR 预热完成')
    })

    it('reveals OCR log path from backend', async () => {
      invokeMock.mockResolvedValue('C:/logs/paddle-ocr-service.log')
      revealItemInDirMock.mockResolvedValue(undefined)

      const store = useSettingsStore()
      const path = await store.revealOcrLog()

      expect(invokeMock).toHaveBeenCalledWith('get_ocr_log_path')
      expect(revealItemInDirMock).toHaveBeenCalledWith('C:/logs/paddle-ocr-service.log')
      expect(path).toBe('C:/logs/paddle-ocr-service.log')
    })
  })
})
