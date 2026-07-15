import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  mergeTranslationIntoHistory,
  updateHistoryFavoriteState,
  type TranslationRecord as Translation,
} from './translation-records'

export type { Translation }

export interface ScreenshotSelection {
  requestId: number
  imageBase64: string
}

export type OcrSourceTextPayload = string | {
  requestId: number | null
  text: string
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  return String(error)
}

function formatStoreError(prefix: string, error: unknown) {
  return `${prefix}: ${getErrorMessage(error)}`
}

export const useTranslationStore = defineStore('translation', () => {
  const currentTranslation = ref<Translation | null>(null)
  const history = ref<Translation[]>([])
  const loading = ref(false)
  const error = ref('')
  const manualInputText = ref('')
  let screenshotOperationId = 0
  let activeScreenshotRequestId: number | null = null

  function setManualInputText(text: string) {
    manualInputText.value = text
  }

  function acceptOcrSourceText(payload: OcrSourceTextPayload) {
    if (typeof payload === 'string') {
      manualInputText.value = payload
      return
    }

    if (payload.requestId === null || payload.requestId === activeScreenshotRequestId) {
      manualInputText.value = payload.text
    }
  }

  async function loadHistory() {
    try {
      history.value = await invoke<Translation[]>('load_history')
    } catch (e) {
      error.value = formatStoreError('加载历史记录失败', e)
    }
  }

  async function loadFavorites() {
    try {
      return await invoke<Translation[]>('load_favorites')
    } catch (e) {
      error.value = formatStoreError('加载收藏列表失败', e)
      return []
    }
  }

  async function getTranslationById(id: number) {
    try {
      return await invoke<Translation>('get_translation_by_id', { id })
    } catch (e) {
      error.value = formatStoreError('查询翻译记录失败', e)
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
      error.value = formatStoreError('更新收藏状态失败', e)
    }
  }

  async function translateFromClipboard() {
    loading.value = true
    error.value = ''

    try {
      const persisted = await invoke<Translation>('translate_from_clipboard')
      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
      return persisted
    } catch (e) {
      error.value = formatStoreError('翻译失败', e)
      return null
    } finally {
      loading.value = false
    }
  }

  async function translateText(text: string) {
    loading.value = true
    error.value = ''

    try {
      const persisted = await invoke<Translation>('translate_text', { text })
      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
      return persisted
    } catch (e) {
      error.value = formatStoreError('翻译失败', e)
      return null
    } finally {
      loading.value = false
    }
  }

  async function translateScreenshot() {
    const operationId = ++screenshotOperationId
    activeScreenshotRequestId = null
    loading.value = true
    error.value = ''

    try {
      const selection = await invoke<ScreenshotSelection>('select_screenshot_area')
      if (operationId !== screenshotOperationId) {
        return null
      }

      activeScreenshotRequestId = selection.requestId
      const persisted = await invoke<Translation>('translate_image', {
        requestId: selection.requestId,
        imageBase64: selection.imageBase64,
      })
      if (
        operationId !== screenshotOperationId
        || activeScreenshotRequestId !== selection.requestId
      ) {
        return null
      }

      currentTranslation.value = persisted
      manualInputText.value = persisted.source_text
      history.value = mergeTranslationIntoHistory(history.value, persisted)
      return persisted
    } catch (e) {
      if (operationId === screenshotOperationId) {
        error.value = formatStoreError('截图 OCR 翻译失败', e)
      }
      return null
    } finally {
      if (operationId === screenshotOperationId) {
        activeScreenshotRequestId = null
        loading.value = false
      }
    }
  }

  return {
    currentTranslation,
    history,
    loading,
    error,
    manualInputText,
    setManualInputText,
    acceptOcrSourceText,
    translateFromClipboard,
    translateText,
    translateScreenshot,
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
  }
})
