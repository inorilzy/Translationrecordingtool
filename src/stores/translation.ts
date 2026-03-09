import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import Database from '@tauri-apps/plugin-sql'
import {
  mergeTranslationIntoHistory,
  normalizeTranslationRow,
  openTranslationsDatabase,
  type TranslationRow,
  updateHistoryFavoriteState,
  upsertTranslation,
  type TranslationRecord as Translation,
} from '../lib/database'

export type { Translation }

export const useTranslationStore = defineStore('translation', () => {
  const apiKey = ref(localStorage.getItem('youdao_app_key') || '')
  const apiSecret = ref(localStorage.getItem('youdao_app_secret') || '')
  const globalShortcut = ref(localStorage.getItem('global_shortcut') || 'Ctrl+Q')
  const currentTranslation = ref<Translation | null>(null)
  const history = ref<Translation[]>([])
  const loading = ref(false)
  const error = ref('')

  let db: Database | null = null

  async function initDatabase() {
    try {
      db = await openTranslationsDatabase()
      await loadHistory()

      // 同步配置到 Rust
      if (apiKey.value && apiSecret.value) {
        await invoke('update_api_config', {
          apiKey: apiKey.value,
          apiSecret: apiSecret.value
        })
      }

      // 同步全局快捷键到 Rust
      if (globalShortcut.value !== 'Ctrl+Q') {
        try {
          await invoke('update_global_shortcut', {
            oldShortcut: 'Ctrl+Q',
            newShortcut: globalShortcut.value
          })
        } catch (e) {
          console.error('同步全局快捷键失败:', e)
        }
      }

      try {
        await invoke('update_tray_behavior', {
          enabled: localStorage.getItem('enable_tray') !== 'false'
        })
      } catch (e) {
        console.error('同步托盘配置失败:', e)
      }
    } catch (e) {
      error.value = `数据库初始化失败: ${e}`
    }
  }

  async function translateFromClipboard() {
    loading.value = true
    error.value = ''

    try {
      const result = await invoke<Translation>('translate_from_clipboard', {
        appKey: apiKey.value,
        appSecret: apiSecret.value,
      })

      if (db) {
        const persisted = await upsertTranslation(db, result, { incrementAccessCount: true })
        currentTranslation.value = persisted
        history.value = mergeTranslationIntoHistory(history.value, persisted)
      } else {
        currentTranslation.value = result
      }
    } catch (e) {
      error.value = `翻译失败: ${e}`
    } finally {
      loading.value = false
    }
  }

  async function loadHistory() {
    if (!db) return

    try {
      const results = await db.select<TranslationRow[]>(
        'SELECT * FROM translations ORDER BY created_at DESC LIMIT 100'
      )
      history.value = results.map(normalizeTranslationRow)
    } catch (e) {
      error.value = `加载历史记录失败: ${e}`
    }
  }

  async function loadFavorites() {
    if (!db) return []

    try {
      const results = await db.select<TranslationRow[]>(
        'SELECT * FROM translations WHERE is_favorite = 1 ORDER BY created_at DESC'
      )
      return results.map(normalizeTranslationRow)
    } catch (e) {
      error.value = `加载收藏列表失败: ${e}`
      return []
    }
  }

  async function getTranslationById(id: number) {
    if (!db) return null

    try {
      const results = await db.select<TranslationRow[]>(
        'SELECT * FROM translations WHERE id = $1',
        [id]
      )
      return results.length > 0 ? normalizeTranslationRow(results[0]) : null
    } catch (e) {
      error.value = `查询翻译记录失败: ${e}`
      return null
    }
  }

  async function toggleFavorite(id: number, isFavorite: boolean) {
    if (!db) return

    try {
      await db.execute(
        'UPDATE translations SET is_favorite = $1 WHERE id = $2',
        [isFavorite ? 1 : 0, id]
      )

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

  async function updateGlobalShortcut(newShortcut: string) {
    try {
      await invoke('update_global_shortcut', {
        oldShortcut: globalShortcut.value,
        newShortcut
      })
      globalShortcut.value = newShortcut
      localStorage.setItem('global_shortcut', newShortcut)
    } catch (e) {
      error.value = `更新快捷键失败: ${e}`
      throw e
    }
  }

  function saveApiConfig() {
    localStorage.setItem('youdao_app_key', apiKey.value)
    localStorage.setItem('youdao_app_secret', apiSecret.value)
  }

  async function updateApiConfig(key: string, secret: string) {
    try {
      await invoke('update_api_config', {
        apiKey: key,
        apiSecret: secret
      })
      apiKey.value = key
      apiSecret.value = secret
      saveApiConfig()
    } catch (e) {
      error.value = `更新配置失败: ${e}`
      throw e
    }
  }

  return {
    apiKey,
    apiSecret,
    globalShortcut,
    currentTranslation,
    history,
    loading,
    error,
    initDatabase,
    translateFromClipboard,
    loadHistory,
    loadFavorites,
    getTranslationById,
    toggleFavorite,
    saveApiConfig,
    updateGlobalShortcut,
    updateApiConfig,
  }
})
