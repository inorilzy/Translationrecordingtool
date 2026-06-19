// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useTranslationStore } from './translation'
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

function createMediaStreamMock({
  width = 1280,
  height = 720,
}: {
  width?: number
  height?: number
} = {}) {
  const track = {
    stop: vi.fn(),
    getSettings: vi.fn(() => ({ width, height })),
  }

  const stream = {
    getTracks: vi.fn(() => [track]),
    getVideoTracks: vi.fn(() => [track]),
  } as unknown as MediaStream

  return { stream, track }
}

function createVideoMock({
  videoWidth = 1280,
  videoHeight = 720,
  readyState = 0,
}: {
  videoWidth?: number
  videoHeight?: number
  readyState?: number
} = {}) {
  const listeners = new Map<string, Set<() => void>>()

  const video = {
    srcObject: null,
    muted: false,
    playsInline: false,
    readyState,
    videoWidth,
    videoHeight,
    play: vi.fn().mockResolvedValue(undefined),
    addEventListener: vi.fn((event: string, listener: () => void) => {
      if (!listeners.has(event)) {
        listeners.set(event, new Set())
      }
      listeners.get(event)?.add(listener)
    }),
    removeEventListener: vi.fn((event: string, listener: () => void) => {
      listeners.get(event)?.delete(listener)
    }),
    dispatch(event: string) {
      listeners.get(event)?.forEach((listener) => listener())
    },
  } as unknown as HTMLVideoElement & { dispatch: (event: string) => void }

  return video
}

function createCanvasMock({
  context = {
    drawImage: vi.fn(),
  } as unknown as CanvasRenderingContext2D,
  dataUrl = 'data:image/png;base64,fake-image',
} = {}) {
  return {
    width: 0,
    height: 0,
    getContext: vi.fn(() => context),
    toDataURL: vi.fn(() => dataUrl),
  } as unknown as HTMLCanvasElement
}

function mockCaptureDom(video: HTMLVideoElement, canvas: HTMLCanvasElement) {
  return vi.spyOn(document, 'createElement').mockImplementation(((tagName: string) => {
    if (tagName === 'video') {
      return video
    }
    if (tagName === 'canvas') {
      return canvas
    }

    throw new Error(`Unexpected element created in test: ${tagName}`)
  }) as typeof document.createElement)
}

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
    it('translates text and persists result on success', async () => {
      const translated = createTranslationRecord({ sourceText: 'hello' })
      const persisted = { ...translated, id: 1, access_count: 1 }

      invokeMock
        .mockResolvedValueOnce(translated)
        .mockResolvedValueOnce(persisted)

      const settings = useSettingsStore()
      settings.apiKey = 'fake-key'
      settings.apiSecret = 'fake-secret'
      settings.translationProvider = 'youdao'
      settings.microsoftTranslatorKey = 'ms-key'
      settings.microsoftTranslatorRegion = 'eastasia'

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'translate_text', {
        text: 'hello',
        appKey: 'fake-key',
        appSecret: 'fake-secret',
        translationProvider: 'youdao',
        microsoftTranslatorKey: 'ms-key',
        microsoftTranslatorRegion: 'eastasia',
      })
      expect(invokeMock).toHaveBeenNthCalledWith(2, 'save_translation', {
        translation: translated,
        incrementAccessCount: true,
      })
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.history).toEqual([persisted])
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
    })

    it('sets error state when translate_text rejects', async () => {
      invokeMock.mockRejectedValueOnce(new Error('network error'))

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(store.error).toContain('翻译失败')
      expect(store.loading).toBe(false)
      expect(store.currentTranslation).toBeNull()
    })

    it('sets error state when save_translation rejects', async () => {
      const translated = createTranslationRecord({ sourceText: 'hello' })
      invokeMock
        .mockResolvedValueOnce(translated)
        .mockRejectedValueOnce(new Error('save failed'))

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(store.error).toContain('翻译失败')
      expect(store.loading).toBe(false)
    })

    it('reads credentials from settings store at call time', async () => {
      const translated = createTranslationRecord({ sourceText: 'test' })
      invokeMock.mockResolvedValue(translated)

      const settings = useSettingsStore()
      settings.apiKey = 'dynamic-key'
      settings.apiSecret = 'dynamic-secret'
      settings.translationProvider = 'microsoft'
      settings.microsoftTranslatorKey = 'dynamic-ms-key'
      settings.microsoftTranslatorRegion = 'global'

      const store = useTranslationStore()
      await store.translateText('test')

      expect(invokeMock).toHaveBeenCalledWith('translate_text', {
        text: 'test',
        appKey: 'dynamic-key',
        appSecret: 'dynamic-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'dynamic-ms-key',
        microsoftTranslatorRegion: 'global',
      })
    })

    it('still translates when settings credentials are empty', async () => {
      const translated = createTranslationRecord({ sourceText: 'test' })
      invokeMock
        .mockResolvedValueOnce(translated)
        .mockResolvedValueOnce({ ...translated, id: 1, access_count: 1 })

      const store = useTranslationStore()
      await store.translateText('test')

      expect(invokeMock).toHaveBeenCalledWith('translate_text', {
        text: 'test',
        appKey: '',
        appSecret: '',
        translationProvider: 'youdao',
        microsoftTranslatorKey: '',
        microsoftTranslatorRegion: '',
      })
      expect(store.currentTranslation).not.toBeNull()
    })
  })

  describe('translateFromClipboard', () => {
    it('translates from clipboard and persists result on success', async () => {
      const translated = createTranslationRecord({ sourceText: 'clipboard text' })
      const persisted = { ...translated, id: 2, access_count: 1 }

      invokeMock
        .mockResolvedValueOnce(translated)
        .mockResolvedValueOnce(persisted)

      const settings = useSettingsStore()
      settings.apiKey = 'clip-key'
      settings.apiSecret = 'clip-secret'
      settings.translationProvider = 'microsoft'
      settings.microsoftTranslatorKey = 'clip-ms-key'
      settings.microsoftTranslatorRegion = 'eastasia'

      const store = useTranslationStore()
      await store.translateFromClipboard()

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'translate_from_clipboard', {
        appKey: 'clip-key',
        appSecret: 'clip-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'clip-ms-key',
        microsoftTranslatorRegion: 'eastasia',
      })
      expect(invokeMock).toHaveBeenNthCalledWith(2, 'save_translation', {
        translation: translated,
        incrementAccessCount: true,
      })
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

  describe('translateScreenshot', () => {
    it('captures the screen, translates the image, and persists the result', async () => {
      const { stream, track } = createMediaStreamMock({ width: 1440, height: 900 })
      const video = createVideoMock({ videoWidth: 1440, videoHeight: 900 })
      const canvas = createCanvasMock()
      const translated = createTranslationRecord({ sourceText: 'screen text' })
      const persisted = { ...translated, id: 3, access_count: 1 }

      const createElementSpy = mockCaptureDom(video, canvas)
      Object.defineProperty(navigator, 'mediaDevices', {
        configurable: true,
        value: {
          getDisplayMedia: vi.fn().mockResolvedValue(stream),
        },
      })

      invokeMock
        .mockResolvedValueOnce(translated)
        .mockResolvedValueOnce(persisted)

      const settings = useSettingsStore()
      settings.apiKey = 'screen-key'
      settings.apiSecret = 'screen-secret'
      settings.translationProvider = 'microsoft'
      settings.microsoftTranslatorKey = 'screen-ms-key'
      settings.microsoftTranslatorRegion = 'global'
      settings.ocrEndpoint = 'http://127.0.0.1:8000/ocr'

      const store = useTranslationStore()
      const promise = store.translateScreenshot()

      await Promise.resolve()
      await Promise.resolve()
      video.dispatch('loadedmetadata')
      await promise

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'translate_image', {
        imageBase64: 'data:image/png;base64,fake-image',
        ocrEndpoint: 'http://127.0.0.1:8000/ocr',
        appKey: 'screen-key',
        appSecret: 'screen-secret',
        translationProvider: 'microsoft',
        microsoftTranslatorKey: 'screen-ms-key',
        microsoftTranslatorRegion: 'global',
      })
      expect(invokeMock).toHaveBeenNthCalledWith(2, 'save_translation', {
        translation: translated,
        incrementAccessCount: true,
      })
      expect(store.currentTranslation).toEqual(persisted)
      expect(store.history).toEqual([persisted])
      expect(store.loading).toBe(false)
      expect(store.error).toBe('')
      expect(canvas.getContext).toHaveBeenCalledWith('2d')
      expect(track.stop).toHaveBeenCalledTimes(1)
      createElementSpy.mockRestore()
    })

    it('shows a readable error when the user cancels screen selection', async () => {
      Object.defineProperty(navigator, 'mediaDevices', {
        configurable: true,
        value: {
          getDisplayMedia: vi.fn().mockRejectedValue(
            new DOMException('Permission denied', 'NotAllowedError'),
          ),
        },
      })

      const store = useTranslationStore()
      await store.translateScreenshot()

      expect(store.error).toBe('截图 OCR 翻译失败: 已取消屏幕选择')
      expect(store.loading).toBe(false)
      expect(invokeMock).not.toHaveBeenCalled()
    })

    it('releases loading and reports a timeout when video metadata never becomes available', async () => {
      vi.useFakeTimers()

      try {
        const { stream, track } = createMediaStreamMock()
        const video = createVideoMock({ videoWidth: 0, videoHeight: 0 })
        const canvas = createCanvasMock()

        const createElementSpy = mockCaptureDom(video, canvas)
        Object.defineProperty(navigator, 'mediaDevices', {
          configurable: true,
          value: {
            getDisplayMedia: vi.fn().mockResolvedValue(stream),
          },
        })

        const store = useTranslationStore()
        const promise = store.translateScreenshot()

        await Promise.resolve()
        await Promise.resolve()
        await vi.advanceTimersByTimeAsync(2000)
        await promise

        expect(store.error).toBe('截图 OCR 翻译失败: 未能及时获取屏幕画面，请重试')
        expect(store.loading).toBe(false)
        expect(track.stop).toHaveBeenCalledTimes(1)
        expect(invokeMock).not.toHaveBeenCalled()
        createElementSpy.mockRestore()
      } finally {
        vi.useRealTimers()
      }
    })

    it('fails fast when the captured frame has no usable size', async () => {
      const { stream, track } = createMediaStreamMock({ width: 0, height: 0 })
      const video = createVideoMock({ videoWidth: 0, videoHeight: 0 })
      const canvas = createCanvasMock()

      const createElementSpy = mockCaptureDom(video, canvas)
      Object.defineProperty(navigator, 'mediaDevices', {
        configurable: true,
        value: {
          getDisplayMedia: vi.fn().mockResolvedValue(stream),
        },
      })

      const store = useTranslationStore()
      const promise = store.translateScreenshot()

      await Promise.resolve()
      await Promise.resolve()
      video.dispatch('loadedmetadata')
      await promise

      expect(store.error).toBe('截图 OCR 翻译失败: 无法获取有效的屏幕画面尺寸')
      expect(store.loading).toBe(false)
      expect(track.stop).toHaveBeenCalledTimes(1)
      expect(invokeMock).not.toHaveBeenCalled()
      createElementSpy.mockRestore()
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
        'microsoftTranslatorRegion', 'ocrEndpoint', 'globalShortcut', 'enableTray', 'theme',
        'loadSettings', 'updateGlobalShortcut', 'updateApiConfig',
        'updateTrayBehavior', 'updateTheme', 'checkOcrService',
      ]

      for (const key of settingsKeys) {
        expect(key in store).toBe(false)
      }
    })
  })
})
