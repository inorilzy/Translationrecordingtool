import { invoke } from '@tauri-apps/api/core'
import {
  applyTheme,
  defaultSettings,
  normalizeSettings,
  type AppSettings,
} from './settings'

/**
 * Pure helper: decide the initial theme from a Rust settings snapshot.
 * Returns the theme string only — no DOM side effects.
 */
export function resolveInitialTheme(
  rustSettings: Partial<AppSettings> | null | undefined,
): string {
  const settings = normalizeSettings(rustSettings)
  const nextTheme =
    settings.theme && settings.theme !== defaultSettings.theme
      ? settings.theme
      : defaultSettings.theme
  return nextTheme
}

/**
 * Full async bootstrap: fetch settings from Rust, resolve theme, apply it.
 * On failure, applies the default theme.
 */
export async function bootstrapTheme() {
  try {
    const settings = await invoke<Partial<AppSettings>>('get_settings')
    applyTheme(resolveInitialTheme(settings))
  } catch (e) {
    console.error('读取持久化设置失败，使用默认主题:', e)
    applyTheme(defaultSettings.theme)
  }
}
