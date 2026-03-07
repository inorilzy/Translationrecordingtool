import { invoke } from '@tauri-apps/api/core'
import Database from '@tauri-apps/plugin-sql'

type TableInfoRow = {
  name: string
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

export async function openTranslationsDatabase() {
  const db = await Database.load('sqlite:translations.db')
  await ensureTranslationsSchema(db)
  return db
}
