// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useTranslationStore, type Translation } from './translation'
import { useSettingsStore } from './settings'
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

function createTranslationRecord({
  id = 1,
  sourceText = 'hello',
  translatedText = '你好',
  isFavorite = 0,
}: {
  id?: number
  sourceText?: string
  translatedText?: string
  isFavorite?: number
}) {
  return {
    id,
    source_text: sourceText,
    translated_text: translatedText,
    phonetic: null,
    us_phonetic: null,
    uk_phonetic: null,
    audio_url: null,
    explains: [],
    examples: [],
    synonyms: [],
    source_lang: 'en',
    target_lang: 'zh',
    word_type: null,
    created_at: Date.now(),
    access_count: 0,
    is_favorite: isFavorite,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

describe('useTranslationStore', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  describe('default state', () => {
    it('initializes with empty translation state', () => {
      const store = useTranslationStore()
      expect(store.currentTranslation).toBeNull()
      expect(store.history).toEqual([])
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
    })
  })

  describe('translateText', () => {
    it('delegates text-only input to the backend workflow and uses its persisted record', async () => {
      const persisted = createTranslationRecord({ sourceText: 'hello' })
      invokeMock.mockResolvedValueOnce(persisted)

      const settings = useSettingsStore()
      settings.apiKey = 'must-not-be-sent'
      settings.translationProvider = 'microsoft'

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(invokeMock).toHaveBeenCalledTimes(1)
      expect(invokeMock).toHaveBeenCalledWith('translate_text', {
        text: 'hello',
      })
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.history).toEqual([persisted])
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
    })

    it('updates manual input text explicitly', () => {
      const store = useTranslationStore()

      store.setManualInputText('corrected OCR text')

      expect(store.manualInputText).toBe('corrected OCR text')
    })

    it('sets error state when translate_text rejects', async () => {
      invokeMock.mockRejectedValueOnce(new Error('network error'))

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(store.error).toContain('翻译失败')
      expect(store.loading).toBe(false)
      expect(store.currentTranslation).toBeNull()
    })
  })

  describe('translateFromClipboard', () => {
    it('delegates clipboard acquisition and persistence to the backend workflow', async () => {
      const persisted = createTranslationRecord({ id: 2, sourceText: 'clipboard text' })
      invokeMock.mockResolvedValueOnce(persisted)

      const settings = useSettingsStore()
      settings.apiSecret = 'must-not-be-sent'

      const store = useTranslationStore()
      await store.translateFromClipboard()

      expect(invokeMock).toHaveBeenCalledTimes(1)
      expect(invokeMock).toHaveBeenCalledWith('translate_from_clipboard')
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.history).toEqual([persisted])
      expect(store.loading).toBe(false)
    })

    it('sets error state when translate_from_clipboard rejects', async () => {
      invokeMock.mockRejectedValueOnce(new Error('clipboard read failed'))

      const store = useTranslationStore()
      await store.translateFromClipboard()

      expect(store.error).toContain('翻译失败')
      expect(store.loading).toBe(false)
    })
  })

  describe('translateImage', () => {
    it('starts an image request and uses the backend-persisted record', async () => {
      const persisted = createTranslationRecord({ id: 9, sourceText: 'pasted image text' })
      invokeMock
        .mockResolvedValueOnce(21)
        .mockResolvedValueOnce({
          imageBase64: 'data:image/png;base64,pasted',
          imageWidth: 180,
          imageHeight: 90,
          blocks: [
            {
              sourceText: 'pasted image text',
              translatedText: persisted.translated_text,
              x: 8,
              y: 9,
              width: 70,
              height: 16,
            },
          ],
          record: persisted,
        })

      const store = useTranslationStore()
      await store.translateImage('data:image/png;base64,pasted')

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'begin_image_translation')
      expect(invokeMock).toHaveBeenNthCalledWith(2, 'translate_image_overlay', {
        requestId: 21,
        imageBase64: 'data:image/png;base64,pasted',
      })
      expect(store.imageOverlay?.imageWidth).toBe(180)
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.manualInputText).toBe('pasted image text')
      expect(store.history).toEqual([persisted])
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
    })

    it('blocks while another translation is already loading', async () => {
      const store = useTranslationStore()
      store.loading = true

      await expect(store.translateImage('data:image/png;base64,x')).resolves.toBeNull()

      expect(store.error).toBe('已有翻译任务进行中，请稍候')
      expect(invokeMock).not.toHaveBeenCalled()
    })

    it('surfaces OCR failures with a readable prefix', async () => {
      invokeMock
        .mockResolvedValueOnce(22)
        .mockRejectedValueOnce(new Error('OCR 未识别到文本'))

      const store = useTranslationStore()
      await store.translateImage('data:image/png;base64,empty')

      expect(store.error).toBe('图片 OCR 翻译失败: OCR 未识别到文本')
      expect(store.loading).toBe(false)
    })
  })

  describe('translateScreenshot', () => {
    it('sends the selected request and uses the backend-persisted record', async () => {
      const persisted = createTranslationRecord({ id: 3, sourceText: 'screen text' })
      invokeMock
        .mockResolvedValueOnce({
          requestId: 7,
          imageBase64: 'data:image/png;base64,fake-image',
        })
        .mockResolvedValueOnce({
          imageBase64: 'data:image/png;base64,fake-image',
          imageWidth: 200,
          imageHeight: 100,
          blocks: [
            {
              sourceText: 'screen text',
              translatedText: persisted.translated_text,
              x: 10,
              y: 12,
              width: 80,
              height: 18,
            },
          ],
          record: persisted,
        })

      const settings = useSettingsStore()
      settings.ocrEndpoint = 'http://must-not-be-sent/ocr'
      settings.apiKey = 'must-not-be-sent'

      const store = useTranslationStore()
      await store.translateScreenshot()

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'select_screenshot_area')
      expect(invokeMock).toHaveBeenNthCalledWith(2, 'translate_image_overlay', {
        requestId: 7,
        imageBase64: 'data:image/png;base64,fake-image',
      })
      expect(store.imageOverlay?.blocks[0]?.translatedText).toBe(persisted.translated_text)
      expect(invokeMock).toHaveBeenCalledTimes(2)
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.history).toEqual([persisted])
      expect(store.manualInputText).toBe('screen text')
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
    })
    it('keeps recognized text when provider translation fails', async () => {
      const providerResult = deferred<{
        imageBase64: string
        imageWidth: number
        imageHeight: number
        blocks: Array<{
          sourceText: string
          translatedText: string
          x: number
          y: number
          width: number
          height: number
        }>
        record: Translation
      }>()
      invokeMock
        .mockResolvedValueOnce({ requestId: 8, imageBase64: 'image-8' })
        .mockImplementationOnce(() => providerResult.promise)

      const store = useTranslationStore()
      const request = store.translateScreenshot()
      await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2))

      store.acceptOcrSourceText({ requestId: 8, text: 'recognized text' })
      providerResult.reject(new Error('provider unavailable'))
      await request

      expect(store.manualInputText).toBe('recognized text')
      expect(store.currentTranslation).toBeNull()
      expect(store.error).toBe('截图 OCR 翻译失败: provider unavailable')
      expect(store.loading).toBe(false)
    })

    it('blocks a second screenshot request while one is already active', async () => {
      const firstSelection = deferred<{ requestId: number; imageBase64: string }>()
      invokeMock.mockImplementationOnce(() => firstSelection.promise)

      const store = useTranslationStore()
      const firstRequest = store.translateScreenshot()
      await vi.waitFor(() => expect(store.loading).toBe(true))

      const secondRequest = store.translateScreenshot()
      await expect(secondRequest).resolves.toBeNull()
      expect(store.error).toBe('已有截图选择进行中，请先按 ESC 取消当前截图')
      expect(invokeMock).toHaveBeenCalledTimes(1)

      firstSelection.resolve({ requestId: 7, imageBase64: 'image-7' })
      invokeMock.mockResolvedValueOnce({
        imageBase64: 'image-7',
        imageWidth: 100,
        imageHeight: 80,
        blocks: [],
        record: createTranslationRecord({ id: 7, sourceText: 'ok' }),
      })
      await firstRequest
      expect(store.loading).toBe(false)
    })

    it('shows a readable error when the user cancels native screen selection', async () => {
      invokeMock.mockRejectedValueOnce(new Error('已取消截图选择'))

      const store = useTranslationStore()
      await store.translateScreenshot()

      expect(store.error).toBe('已取消截图选择')
      expect(store.loading).toBe(false)
      expect(invokeMock).toHaveBeenCalledTimes(1)
      expect(invokeMock).toHaveBeenCalledWith('select_screenshot_area')
    })

    it('does not make another request when OCR translation rejects', async () => {
      invokeMock
        .mockResolvedValueOnce({ requestId: 51, imageBase64: 'fake-image' })
        .mockRejectedValueOnce(new Error('OCR 未识别到文本'))

      const store = useTranslationStore()
      await store.translateScreenshot()

      expect(store.error).toBe('截图 OCR 翻译失败: OCR 未识别到文本')
      expect(store.loading).toBe(false)
      expect(invokeMock).toHaveBeenCalledTimes(2)
    })
  })

  describe('loadHistory', () => {
    it('loads history from backend', async () => {
      const records = [
        createTranslationRecord({ id: 1, sourceText: 'first' }),
        createTranslationRecord({ id: 2, sourceText: 'second' }),
      ]
      invokeMock.mockResolvedValueOnce(records)

      const store = useTranslationStore()
      await store.loadHistory()

      expect(store.history).toEqual(records)
      expect(store.error).toBe('')
    })

    it('sets error state when load_history rejects', async () => {
      invokeMock.mockRejectedValueOnce(new Error('db error'))

      const store = useTranslationStore()
      await store.loadHistory()

      expect(store.error).toContain('加载历史记录失败')
    })
  })

  describe('loadFavorites', () => {
    it('returns favorites list on success', async () => {
      const favorites = [
        createTranslationRecord({ id: 1, isFavorite: 1 }),
      ]
      invokeMock.mockResolvedValueOnce(favorites)

      const store = useTranslationStore()
      const result = await store.loadFavorites()

      expect(result).toEqual(favorites)
      expect(store.error).toBe('')
    })

    it('returns empty array and sets error on failure', async () => {
      invokeMock.mockRejectedValueOnce(new Error('load favorites failed'))

      const store = useTranslationStore()
      const result = await store.loadFavorites()

      expect(result).toEqual([])
      expect(store.error).toContain('加载收藏列表失败')
    })
  })

  describe('getTranslationById', () => {
    it('returns translation record by id', async () => {
      const record = createTranslationRecord({ id: 42 })
      invokeMock.mockResolvedValueOnce(record)

      const store = useTranslationStore()
      const result = await store.getTranslationById(42)

      expect(result).toEqual(record)
    })

    it('returns null and sets error on failure', async () => {
      invokeMock.mockRejectedValueOnce(new Error('not found'))

      const store = useTranslationStore()
      const result = await store.getTranslationById(999)

      expect(result).toBeNull()
      expect(store.error).toContain('查询翻译记录失败')
    })
  })

  describe('toggleFavorite', () => {
    it('toggles favorite and updates history and currentTranslation', async () => {
      invokeMock.mockResolvedValueOnce(undefined)

      const record = createTranslationRecord({ id: 1, isFavorite: 0 })
      const store = useTranslationStore()
      store.history = [record]
      store.currentTranslation = record

      await store.toggleFavorite(1, true)

      expect(invokeMock).toHaveBeenCalledWith('toggle_favorite', {
        id: 1,
        isFavorite: true,
      })
      expect(store.history[0].is_favorite).toBe(1)
      expect(store.currentTranslation?.is_favorite).toBe(1)
    })

    it('updates history even when currentTranslation is unrelated', async () => {
      invokeMock.mockResolvedValueOnce(undefined)

      const record = createTranslationRecord({ id: 1, isFavorite: 0 })
      const unrelated = createTranslationRecord({ id: 2, sourceText: 'unrelated' })

      const store = useTranslationStore()
      store.history = [record]
      store.currentTranslation = unrelated

      await store.toggleFavorite(1, true)

      expect(store.history[0].is_favorite).toBe(1)
      expect(store.currentTranslation?.id).toBe(2)
      expect(store.currentTranslation?.is_favorite).toBe(0)
    })

    it('sets error state when toggle_favorite rejects', async () => {
      invokeMock.mockRejectedValueOnce(new Error('toggle failed'))

      const store = useTranslationStore()
      store.history = [createTranslationRecord({ id: 1 })]

      await store.toggleFavorite(1, true)

      expect(store.error).toContain('更新收藏状态失败')
    })
  })

  describe('translation store has no settings exports', () => {
    it('does not export settings-related refs or functions', () => {
      const store = useTranslationStore()

      const settingsKeys = [
        'apiKey', 'apiSecret', 'translationProvider', 'microsoftTranslatorKey',
        'microsoftTranslatorRegion', 'ocrEndpoint', 'globalShortcut', 'screenshotShortcut',
        'enableTray', 'theme', 'loadSettings', 'updateGlobalShortcut',
        'updateScreenshotShortcut', 'updateApiConfig', 'updateTrayBehavior',
        'updateTheme', 'checkOcrService', 'getOcrServiceStatus',
      ]

      for (const key of settingsKeys) {
        expect(key in store).toBe(false)
      }
    })
  })
})
