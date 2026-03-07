import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import Database from '@tauri-apps/plugin-sql'
import { openTranslationsDatabase } from '../lib/database'

export interface Translation {
  id?: number
  source_text: string
  translated_text: string
  phonetic?: string
  us_phonetic?: string
  uk_phonetic?: string
  audio_url?: string
  explains?: string[]
  examples?: string[]
  synonyms?: string[]
  source_lang: string
  target_lang: string
  word_type?: string
  created_at: number
  access_count: number
  is_favorite: number
}

interface TranslationRow extends Omit<Translation, 'explains' | 'examples' | 'synonyms'> {
  explains?: string[] | string | null
  examples?: string[] | string | null
  synonyms?: string[] | string | null
}

function parseStringList(value?: string[] | string | null) {
  if (Array.isArray(value)) {
    return value
  }

  if (typeof value !== 'string' || !value.trim()) {
    return undefined
  }

  try {
    const parsed = JSON.parse(value)
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : undefined
  } catch {
    return [value]
  }
}

function normalizeTranslation(row: TranslationRow): Translation {
  return {
    ...row,
    explains: parseStringList(row.explains),
    examples: parseStringList(row.examples),
    synonyms: parseStringList(row.synonyms),
  }
}

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

      currentTranslation.value = result

      // 保存到数据库
      if (db) {
        const explainsJson = result.explains ? JSON.stringify(result.explains) : null
        const examplesJson = result.examples ? JSON.stringify(result.examples) : null
        const synonymsJson = result.synonyms ? JSON.stringify(result.synonyms) : null

        await db.execute(
          `INSERT INTO translations (source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 0)
           ON CONFLICT(source_text, source_lang, target_lang)
           DO UPDATE SET
             translated_text = $2,
             phonetic = $3,
             us_phonetic = $4,
             uk_phonetic = $5,
             audio_url = $6,
             explains = $7,
             examples = $8,
             synonyms = $9,
             word_type = $12,
             access_count = access_count + 1,
             created_at = $13`,
          [
            result.source_text,
            result.translated_text,
            result.phonetic,
            result.us_phonetic,
            result.uk_phonetic,
            result.audio_url,
            explainsJson,
            examplesJson,
            synonymsJson,
            result.source_lang,
            result.target_lang,
            result.word_type,
            result.created_at,
            result.access_count,
          ]
        )
        await loadHistory()
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
      history.value = results.map(normalizeTranslation)
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
      return results.map(normalizeTranslation)
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
      return results.length > 0 ? normalizeTranslation(results[0]) : null
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
    } catch (e) {
      error.value = `更新收藏状态失败: ${e}`
    }
  }

  function saveApiConfig() {
    localStorage.setItem('youdao_app_key', apiKey.value)
    localStorage.setItem('youdao_app_secret', apiSecret.value)
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
