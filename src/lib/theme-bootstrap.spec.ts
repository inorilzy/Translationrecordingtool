import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { resolveInitialTheme } from './theme-bootstrap'
import { defaultSettings } from './settings'

// vi.mock is hoisted — define the mock factory before any imports take effect
const applyThemeCalls: string[] = []

vi.mock('./settings', async () => {
  const mod = await vi.importActual<typeof import('./settings')>('./settings')
  return {
    ...mod,
    applyTheme: (theme: string) => {
      applyThemeCalls.push(theme)
    },
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const mockInvoke = vi.mocked(invoke)

describe('resolveInitialTheme (pure)', () => {
  it('uses theme from Rust snapshot when present and non-default', () => {
    expect(resolveInitialTheme({ theme: 'dark' })).toBe('dark')
  })

  it('falls back to default when Rust snapshot theme equals default', () => {
    expect(resolveInitialTheme({ theme: 'light' })).toBe('light')
  })

  it('falls back to default when theme field is missing', () => {
    expect(resolveInitialTheme({ apiKey: 'key' })).toBe(
      defaultSettings.theme,
    )
  })

  it('falls back to default when input is null', () => {
    expect(resolveInitialTheme(null)).toBe(defaultSettings.theme)
  })

  it('falls back to default when input is undefined', () => {
    expect(resolveInitialTheme(undefined)).toBe(defaultSettings.theme)
  })

  it('falls back to default when input is empty object', () => {
    expect(resolveInitialTheme({})).toBe(defaultSettings.theme)
  })
})

describe('bootstrapTheme (async)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    applyThemeCalls.length = 0
  })

  afterEach(() => {
    vi.resetModules()
  })

  it('applies theme from Rust snapshot on success', async () => {
    mockInvoke.mockResolvedValueOnce({ theme: 'dark' })
    // Re-import after resetModules to get fresh module with mocks applied
    const { bootstrapTheme: bt } = await import('./theme-bootstrap')
    await bt()
    expect(mockInvoke).toHaveBeenCalledWith('get_settings')
    expect(applyThemeCalls).toContain('dark')
  })

  it('applies default theme when Rust invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('connection refused'))
    const { bootstrapTheme: bt } = await import('./theme-bootstrap')
    await bt()
    expect(mockInvoke).toHaveBeenCalledWith('get_settings')
    expect(applyThemeCalls).toContain(defaultSettings.theme)
  })

  it('applies default theme when Rust invoke returns malformed data', async () => {
    mockInvoke.mockResolvedValueOnce(null)
    const { bootstrapTheme: bt } = await import('./theme-bootstrap')
    await bt()
    expect(applyThemeCalls).toContain(defaultSettings.theme)
  })
})
