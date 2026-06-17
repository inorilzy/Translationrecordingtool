import { createRouter, createMemoryHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import type { TranslationRecord } from './stores/translation-records'

export function createTestRouter(routes: RouteRecordRaw[]) {
  return createRouter({
    history: createMemoryHistory(),
    routes,
  })
}

export function createTranslationRecord(overrides: Partial<TranslationRecord> = {}): TranslationRecord {
  return {
    id: 1,
    source_text: 'hello',
    translated_text: '你好',
    phonetic: '/həˈloʊ/',
    us_phonetic: null,
    uk_phonetic: null,
    audio_url: null,
    explains: ['int. 你好'],
    examples: ['hello world'],
    synonyms: ['hi'],
    source_lang: 'en',
    target_lang: 'zh',
    word_type: 'int.',
    created_at: 1710000000,
    access_count: 1,
    is_favorite: 0,
    ...overrides,
  }
}
