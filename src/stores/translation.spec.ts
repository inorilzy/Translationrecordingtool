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

      const store = useTranslationStore()
      await store.translateText('hello')

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'translate_text', {
        text: 'hello',
        appKey: 'fake-key',
        appSecret: 'fake-secret',
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

      const store = useTranslationStore()
      await store.translateText('test')

      expect(invokeMock).toHaveBeenCalledWith('translate_text', {
        text: 'test',
        appKey: 'dynamic-key',
        appSecret: 'dynamic-secret',
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

      const store = useTranslationStore()
      await store.translateFromClipboard()

      expect(invokeMock).toHaveBeenNthCalledWith(1, 'translate_from_clipboard', {
        appKey: 'clip-key',
        appSecret: 'clip-secret',
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
        'apiKey', 'apiSecret', 'globalShortcut', 'enableTray', 'theme',
        'loadSettings', 'updateGlobalShortcut', 'updateApiConfig',
        'updateTrayBehavior', 'updateTheme',
      ]

      for (const key of settingsKeys) {
        expect(key in store).toBe(false)
      }
    })
  })
})
