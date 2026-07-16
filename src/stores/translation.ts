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

export interface OverlayTextBlock {
  sourceText: string
  translatedText: string
  x: number
  y: number
  width: number
  height: number
}

export interface ImageOverlayTranslation {
  imageBase64: string
  imageWidth: number
  imageHeight: number
  blocks: OverlayTextBlock[]
  record: Translation
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
  const imageOverlay = ref<ImageOverlayTranslation | null>(null)
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

  async function completeImageTranslation(
    operationId: number,
    requestId: number,
    imageBase64: string,
    failurePrefix: string,
  ) {
    activeScreenshotRequestId = requestId
    try {
      const overlay = await invoke<ImageOverlayTranslation>('translate_image_overlay', {
        requestId,
        imageBase64,
      })
      if (
        operationId !== screenshotOperationId
        || activeScreenshotRequestId !== requestId
      ) {
        return null
      }

      const persisted = overlay.record
      currentTranslation.value = persisted
      manualInputText.value = persisted.source_text
      history.value = mergeTranslationIntoHistory(history.value, persisted)
      imageOverlay.value = overlay
      return persisted
    } catch (e) {
      if (operationId === screenshotOperationId) {
        const message = getErrorMessage(e)
        if (message.includes('已取消截图选择') || message.includes('已有截图选择进行中')) {
          error.value = message
        } else {
          error.value = formatStoreError(failurePrefix, e)
        }
      }
      return null
    } finally {
      if (operationId === screenshotOperationId) {
        activeScreenshotRequestId = null
        loading.value = false
      }
    }
  }

  function clearImageOverlay() {
    imageOverlay.value = null
  }

  async function translateImage(imageBase64: string) {
    if (loading.value) {
      error.value = '已有翻译任务进行中，请稍候'
      return null
    }

    const operationId = ++screenshotOperationId
    activeScreenshotRequestId = null
    loading.value = true
    error.value = ''

    try {
      const requestId = await invoke<number>('begin_image_translation')
      if (operationId !== screenshotOperationId) {
        return null
      }

      return await completeImageTranslation(
        operationId,
        requestId,
        imageBase64,
        '图片 OCR 翻译失败',
      )
    } catch (e) {
      if (operationId === screenshotOperationId) {
        error.value = formatStoreError('图片 OCR 翻译失败', e)
        activeScreenshotRequestId = null
        loading.value = false
      }
      return null
    }
  }

  async function translateScreenshot() {
    if (loading.value) {
      error.value = '已有截图选择进行中，请先按 ESC 取消当前截图'
      return null
    }

    const operationId = ++screenshotOperationId
    activeScreenshotRequestId = null
    loading.value = true
    error.value = ''

    try {
      const selection = await invoke<ScreenshotSelection>('select_screenshot_area')
      if (operationId !== screenshotOperationId) {
        return null
      }

      return await completeImageTranslation(
        operationId,
        selection.requestId,
        selection.imageBase64,
        '截图 OCR 翻译失败',
      )
    } catch (e) {
      if (operationId === screenshotOperationId) {
        const message = getErrorMessage(e)
        if (message.includes('已取消截图选择') || message.includes('已有截图选择进行中')) {
          error.value = message
        } else {
          error.value = formatStoreError('截图 OCR 翻译失败', e)
        }
        activeScreenshotRequestId = null
        loading.value = false
      }
      return null
    }
  }

  return {
    currentTranslation,
    history,
    loading,
    error,
    manualInputText,
    imageOverlay,
    setManualInputText,
    clearImageOverlay,
    acceptOcrSourceText,
    translateFromClipboard,
    translateText,
    translateImage,
    translateScreenshot,
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
  }
})
