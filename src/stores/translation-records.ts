export interface TranslationRecord {
  id?: number
  source_text: string
  translated_text: string
  phonetic?: string | null
  us_phonetic?: string | null
  uk_phonetic?: string | null
  audio_url?: string | null
  explains?: string[]
  examples?: string[]
  synonyms?: string[]
  source_lang: string
  target_lang: string
  word_type?: string | null
  created_at: number
  access_count: number
  is_favorite: number
}

export function mergeTranslationIntoHistory(
  history: TranslationRecord[],
  nextTranslation: TranslationRecord,
  limit = 100,
) {
  const merged = history.filter(item => {
    if (nextTranslation.id !== undefined && item.id !== undefined) {
      return item.id !== nextTranslation.id
    }

    return !(
      item.source_text === nextTranslation.source_text
      && item.source_lang === nextTranslation.source_lang
      && item.target_lang === nextTranslation.target_lang
    )
  })

  return [nextTranslation, ...merged].slice(0, limit)
}

export function updateHistoryFavoriteState(
  history: TranslationRecord[],
  id: number,
  isFavorite: number,
) {
  return history.map(item => {
    if (item.id !== id) {
      return item
    }

    return {
      ...item,
      is_favorite: isFavorite,
    }
  })
}
