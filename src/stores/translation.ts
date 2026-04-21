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

export const useTranslationStore = defineStore('translation', () => {
  const currentTranslation = ref<Translation | null>(null)
  const history = ref<Translation[]>([])
  const loading = ref(false)
  const error = ref('')

  async function loadHistory() {
    try {
      history.value = await invoke<Translation[]>('load_history')
    } catch (e) {
      error.value = `加载历史记录失败: ${e}`
    }
  }

  async function loadFavorites() {
    try {
      return await invoke<Translation[]>('load_favorites')
    } catch (e) {
      error.value = `加载收藏列表失败: ${e}`
      return []
    }
  }

  async function getTranslationById(id: number) {
    try {
      return await invoke<Translation>('get_translation_by_id', { id })
    } catch (e) {
      error.value = `查询翻译记录失败: ${e}`
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
      error.value = `更新收藏状态失败: ${e}`
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
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = `翻译失败: ${e}`
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
      })

      const persisted = await invoke<Translation>('save_translation', {
        translation: result,
        incrementAccessCount: true,
      })

      currentTranslation.value = persisted
      history.value = mergeTranslationIntoHistory(history.value, persisted)
    } catch (e) {
      error.value = `翻译失败: ${e}`
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
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
  }
})
