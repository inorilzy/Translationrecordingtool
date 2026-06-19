import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  mergeTranslationIntoHistory,
  updateHistoryFavoriteState,
  type TranslationRecord as Translation,
} from './translation-records'
import { useSettingsStore } from './settings'

export type { Translation }

const SCREENSHOT_METADATA_TIMEOUT_MS = 2000

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  return String(error)
}

function formatStoreError(prefix: string, error: unknown) {
  return `${prefix}: ${getErrorMessage(error)}`
}

function isScreenSelectionCancelled(error: unknown) {
  return error instanceof DOMException
    && (error.name === 'NotAllowedError' || error.name === 'AbortError')
}

async function waitForVideoMetadata(video: HTMLVideoElement): Promise<void> {
  const haveMetadata = typeof HTMLMediaElement === 'undefined'
    ? 1
    : HTMLMediaElement.HAVE_METADATA

  if (video.readyState >= haveMetadata) {
    return
  }

  await new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      window.clearTimeout(timeoutId)
      video.removeEventListener('loadedmetadata', handleReady)
      video.removeEventListener('loadeddata', handleReady)
      video.removeEventListener('error', handleError)
    }

    const handleReady = () => {
      cleanup()
      resolve()
    }

    const handleError = () => {
      cleanup()
      reject(new Error('无法读取屏幕画面，请重试'))
    }

    const timeoutId = window.setTimeout(() => {
      cleanup()
      reject(new Error('未能及时获取屏幕画面，请重试'))
    }, SCREENSHOT_METADATA_TIMEOUT_MS)

    video.addEventListener('loadedmetadata', handleReady)
    video.addEventListener('loadeddata', handleReady)
    video.addEventListener('error', handleError)
  })
}

function getCaptureDimensions(video: HTMLVideoElement, stream: MediaStream) {
  const trackSettings = stream.getVideoTracks()[0]?.getSettings()
  const width = video.videoWidth || trackSettings?.width || 0
  const height = video.videoHeight || trackSettings?.height || 0

  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    throw new Error('无法获取有效的屏幕画面尺寸')
  }

  return { width, height }
}

async function captureScreenFrame(): Promise<string> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error('当前环境不支持屏幕截图')
  }

  let stream: MediaStream | null = null

  try {
    stream = await navigator.mediaDevices.getDisplayMedia({
      video: true,
      audio: false,
    })

    const video = document.createElement('video')
    video.srcObject = stream
    video.muted = true
    video.playsInline = true
    await video.play()

    await waitForVideoMetadata(video)
    const { width, height } = getCaptureDimensions(video, stream)

    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) {
      throw new Error('无法创建截图画布')
    }

    try {
      context.drawImage(video, 0, 0, canvas.width, canvas.height)
    } catch {
      throw new Error('无法捕获屏幕画面，请重试')
    }

    return canvas.toDataURL('image/png')
  } catch (error) {
    if (isScreenSelectionCancelled(error)) {
      throw new Error('已取消屏幕选择')
    }

    throw error
  } finally {
    stream?.getTracks().forEach((track) => track.stop())
  }
}

export const useTranslationStore = defineStore('translation', () => {
  const currentTranslation = ref<Translation | null>(null)
  const history = ref<Translation[]>([])
  const loading = ref(false)
  const error = ref('')

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
    const settings = useSettingsStore()
    loading.value = true
    error.value = ''

    try {
      const result = await invoke<Translation>('translate_from_clipboard', {
        appKey: settings.apiKey,
        appSecret: settings.apiSecret,
        translationProvider: settings.translationProvider,
        microsoftTranslatorKey: settings.microsoftTranslatorKey,
        microsoftTranslatorRegion: settings.microsoftTranslatorRegion,
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = formatStoreError('翻译失败', e)
    } finally {
      loading.value = false
    }
  }

  async function translateText(text: string) {
    const settings = useSettingsStore()
    loading.value = true
    error.value = ''

    try {
      const result = await invoke<Translation>('translate_text', {
        text,
        appKey: settings.apiKey,
        appSecret: settings.apiSecret,
        translationProvider: settings.translationProvider,
        microsoftTranslatorKey: settings.microsoftTranslatorKey,
        microsoftTranslatorRegion: settings.microsoftTranslatorRegion,
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = formatStoreError('翻译失败', e)
    } finally {
      loading.value = false
    }
  }

  async function translateScreenshot() {
    const settings = useSettingsStore()
    loading.value = true
    error.value = ''

    try {
      const imageBase64 = await captureScreenFrame()
      const result = await invoke<Translation>('translate_image', {
        imageBase64,
        ocrEndpoint: settings.ocrEndpoint,
        appKey: settings.apiKey,
        appSecret: settings.apiSecret,
        translationProvider: settings.translationProvider,
        microsoftTranslatorKey: settings.microsoftTranslatorKey,
        microsoftTranslatorRegion: settings.microsoftTranslatorRegion,
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = formatStoreError('截图 OCR 翻译失败', e)
    } finally {
      loading.value = false
    }
  }

  return {
    currentTranslation,
    history,
    loading,
    error,
    translateFromClipboard,
    translateText,
    translateScreenshot,
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
  }
})
