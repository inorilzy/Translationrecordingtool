import { invoke } from '@tauri-apps/api/core'
import Database from '@tauri-apps/plugin-sql'

type TableInfoRow = {
  name: string
}

type StringListValue = string[] | string | null | undefined

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

export type TranslationRow = Omit<TranslationRecord, 'explains' | 'examples' | 'synonyms'> & {
  explains?: StringListValue
  examples?: StringListValue
  synonyms?: StringListValue
}

const REQUIRED_COLUMNS: Record<string, string> = {
  us_phonetic: 'TEXT',
  uk_phonetic: 'TEXT',
  audio_url: 'TEXT',
  explains: 'TEXT',
  examples: 'TEXT',
  synonyms: 'TEXT',
}

export async function ensureTranslationsSchema(db: Database) {
  const initSql = await invoke<string>('get_init_sql')
  await db.execute(initSql)

  const columns = await db.select<TableInfoRow[]>('PRAGMA table_info(translations)')
  const existingColumns = new Set(columns.map((column) => column.name))

  for (const [columnName, columnType] of Object.entries(REQUIRED_COLUMNS)) {
    if (!existingColumns.has(columnName)) {
      await db.execute(`ALTER TABLE translations ADD COLUMN ${columnName} ${columnType}`)
    }
  }
}

function serializeStringList(items?: string[]) {
  return items?.length ? JSON.stringify(items) : null
}

function parseStringList(value?: StringListValue) {
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

export function normalizeTranslationRow(row: TranslationRow): TranslationRecord {
  return {
    ...row,
    explains: parseStringList(row.explains),
    examples: parseStringList(row.examples),
    synonyms: parseStringList(row.synonyms),
  }
}

export async function getTranslationByLookupKey(
  db: Database,
  sourceText: string,
  sourceLang: string,
  targetLang: string
) {
  const results = await db.select<TranslationRow[]>(
    'SELECT * FROM translations WHERE source_text = $1 AND source_lang = $2 AND target_lang = $3',
    [sourceText, sourceLang, targetLang]
  )

  return results[0] ? normalizeTranslationRow(results[0]) : null
}

export async function upsertTranslation(
  db: Database,
  translation: TranslationRecord,
  options: { incrementAccessCount?: boolean } = {}
) {
  const explainsJson = serializeStringList(translation.explains)
  const examplesJson = serializeStringList(translation.examples)
  const synonymsJson = serializeStringList(translation.synonyms)
  const accessCountClause = options.incrementAccessCount === false ? '' : 'access_count = access_count + 1,'

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
       ${accessCountClause}
       created_at = $13`,
    [
      translation.source_text,
      translation.translated_text,
      translation.phonetic ?? null,
      translation.us_phonetic ?? null,
      translation.uk_phonetic ?? null,
      translation.audio_url ?? null,
      explainsJson,
      examplesJson,
      synonymsJson,
      translation.source_lang,
      translation.target_lang,
      translation.word_type ?? null,
      translation.created_at,
      translation.access_count,
    ]
  )

  const persisted = await getTranslationByLookupKey(
    db,
    translation.source_text,
    translation.source_lang,
    translation.target_lang
  )

  return persisted ?? translation
}

export function mergeTranslationIntoHistory(
  history: TranslationRecord[],
  nextTranslation: TranslationRecord,
  limit = 100
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
  isFavorite: number
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

export async function openTranslationsDatabase() {
  const db = await Database.load('sqlite:translations.db')
  await ensureTranslationsSchema(db)
  return db
}
