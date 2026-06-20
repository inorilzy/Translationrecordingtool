import { describe, it, expect } from 'vitest'
import {
  defaultSettings,
  normalizeSettings,
  isDefaultSettings,
} from './settings'

describe('normalizeSettings', () => {
  it('returns full defaults when input is undefined', () => {
    const result = normalizeSettings(undefined)
    expect(result).toEqual(defaultSettings)
  })

  it('returns full defaults when input is null', () => {
    const result = normalizeSettings(null)
    expect(result).toEqual(defaultSettings)
  })

  it('returns full defaults when input is empty object', () => {
    const result = normalizeSettings({})
    expect(result).toEqual(defaultSettings)
  })

  it('merges partial settings with defaults', () => {
    const result = normalizeSettings({ theme: 'dark' })
    expect(result.theme).toBe('dark')
    expect(result.apiKey).toBe(defaultSettings.apiKey)
    expect(result.translationProvider).toBe(defaultSettings.translationProvider)
    expect(result.microsoftTranslatorKey).toBe(defaultSettings.microsoftTranslatorKey)
    expect(result.microsoftTranslatorRegion).toBe(defaultSettings.microsoftTranslatorRegion)
    expect(result.ocrEndpoint).toBe(defaultSettings.ocrEndpoint)
    expect(result.enableTray).toBe(defaultSettings.enableTray)
  })

  it('preserves valid non-default values', () => {
    const partial = {
      apiKey: 'test-key',
      apiSecret: 'test-secret',
      translationProvider: 'microsoft',
      microsoftTranslatorKey: 'ms-key',
      microsoftTranslatorRegion: 'eastasia',
      ocrEndpoint: 'http://127.0.0.1:8866/ocr',
      ocrEngine: 'paddleocr',
      ocrModelProfile: 'accurate',
      ocrPreloadOnStartup: false,
      globalShortcut: 'Ctrl+Shift+Q',
      screenshotShortcut: 'Ctrl+Shift+S',
      enableTray: false,
      theme: 'dark',
    }
    const result = normalizeSettings(partial)
    expect(result).toEqual(partial)
  })

  it('handles empty string api credentials as valid', () => {
    const result = normalizeSettings({ apiKey: '', apiSecret: '', theme: 'light' })
    expect(result.apiKey).toBe('')
    expect(result.apiSecret).toBe('')
    expect(result.translationProvider).toBe(defaultSettings.translationProvider)
    expect(result.microsoftTranslatorKey).toBe(defaultSettings.microsoftTranslatorKey)
    expect(result.microsoftTranslatorRegion).toBe(defaultSettings.microsoftTranslatorRegion)
    expect(result.ocrEndpoint).toBe(defaultSettings.ocrEndpoint)
    expect(result.ocrEngine).toBe(defaultSettings.ocrEngine)
    expect(result.ocrModelProfile).toBe(defaultSettings.ocrModelProfile)
    expect(result.ocrPreloadOnStartup).toBe(defaultSettings.ocrPreloadOnStartup)
    expect(result.theme).toBe('light')
    expect(result.globalShortcut).toBe(defaultSettings.globalShortcut)
    expect(result.screenshotShortcut).toBe(defaultSettings.screenshotShortcut)
    expect(result.enableTray).toBe(defaultSettings.enableTray)
  })
})

describe('isDefaultSettings', () => {
  it('returns true for defaultSettings', () => {
    expect(isDefaultSettings(defaultSettings)).toBe(true)
  })

  it('returns false when any field differs', () => {
    expect(isDefaultSettings({ ...defaultSettings, theme: 'dark' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, apiKey: 'x' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, translationProvider: 'microsoft' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, microsoftTranslatorKey: 'x' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, ocrEndpoint: 'http://127.0.0.1:8867/ocr' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, ocrModelProfile: 'lite' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, ocrPreloadOnStartup: false })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, screenshotShortcut: 'Ctrl+Shift+S' })).toBe(false)
    expect(isDefaultSettings({ ...defaultSettings, enableTray: false })).toBe(false)
  })
})
