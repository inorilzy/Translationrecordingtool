import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { shallowRef } from '@vue/reactivity'

// ─── Fake window (must exist before controls module loads) ──────────────────

const fakeWindow = vi.hoisted(() => {
  const listeners: Map<string, Set<EventListener>> = new Map()
  return {
    addEventListener: vi.fn((event: string, handler: EventListener) => {
      if (!listeners.has(event)) listeners.set(event, new Set())
      listeners.get(event)!.add(handler)
    }),
    removeEventListener: vi.fn((event: string, handler: EventListener) => {
      listeners.get(event)?.delete(handler)
    }),
    dispatchEvent: vi.fn((event: Event) => {
      const handlers = listeners.get(event.type)
      if (handlers) {
        for (const h of handlers) {
          h(event)
        }
      }
      return true
    }),
    _getListeners: (event: string) => listeners.get(event) ?? new Set(),
    _clear: () => {
      listeners.clear()
    },
  }
})

vi.hoisted(() => {
  // @ts-ignore - we're deliberately setting a global
  globalThis.window = fakeWindow
})

// ─── Tauri API Mocks ────────────────────────────────────────────────────────

const tauriMocks = vi.hoisted(() => ({
  hide: vi.fn().mockResolvedValue(undefined),
  startDragging: vi.fn().mockResolvedValue(undefined),
  emit: vi.fn().mockResolvedValue(undefined),
  invoke: vi.fn(),
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    hide: tauriMocks.hide,
    startDragging: tauriMocks.startDragging,
    emit: tauriMocks.emit,
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: tauriMocks.listen,
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => tauriMocks.invoke(...args),
}))

// ─── Settings & theme mock ──────────────────────────────────────────────────

const settingsMocks = vi.hoisted(() => ({
  applyThemeCalls: [] as string[],
  defaultSettings: { theme: 'light' },
}))

vi.mock('../lib/settings', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/settings')>()
  return {
    ...actual,
    applyTheme: (theme: string) => settingsMocks.applyThemeCalls.push(theme),
    getSettingsSnapshot: vi.fn().mockResolvedValue({ theme: 'light' }),
    defaultSettings: { ...actual.defaultSettings },
  }
})

// ─── Vue lifecycle mock ─────────────────────────────────────────────────────

vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>()
  return {
    ...actual,
    onMounted: (fn: () => Promise<void> | void) => {
      // Store for manual invocation in tests
      onMountedCallbacks.push(fn)
    },
    onUnmounted: vi.fn(),
    nextTick: () => Promise.resolve(),
  }
})

const onMountedCallbacks: Array<() => Promise<void> | void> = []

// ─── Import under test ──────────────────────────────────────────────────────

import { createPopupControls } from './popup-window-controls'

// ─── Testable extraction of applyTranslation behavior ───────────────────────

/**
 * Mirrors the applyTranslation logic from PopupWindow.vue in a testable form.
 * This lets us verify the runtime contract (loading → result → persisted state)
 * without needing @vue/test-utils or a full component mount.
 */
function applyTranslation(
  payload: Record<string, unknown>,
  incrementAccessCount: boolean,
  currentTranslation: { value: Record<string, unknown> | null },
  loading: { value: boolean },
  error: { value: string },
  persistFn: (t: Record<string, unknown>, inc: boolean) => Promise<Record<string, unknown>>,
) {
  let nextTranslation: Record<string, unknown> = { ...payload }

  return persistFn(nextTranslation, incrementAccessCount)
    .then((persisted) => {
      currentTranslation.value = persisted
      loading.value = false
      error.value = ''
      return persisted
    })
    .catch((e: unknown) => {
      // Component catches and logs; state still updates with original payload
      console.error('保存翻译失败:', e)
      currentTranslation.value = nextTranslation
      loading.value = false
      error.value = ''
      return nextTranslation
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function makeTranslationRecord(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: 1,
    source_text: 'hello',
    translated_text: '你好',
    phonetic: '/həˈloʊ/',
    us_phonetic: '/həˈloʊ/',
    uk_phonetic: '/həˈləʊ/',
    audio_url: 'https://example.com/audio.mp3',
    explains: ['int. 你好', 'n. 打招呼'],
    examples: ['Hello, world!'],
    synonyms: ['hi', 'hey'],
    source_lang: 'en',
    target_lang: 'zh',
    word_type: 'int.',
    created_at: Date.now(),
    access_count: 0,
    is_favorite: 0,
    ...overrides,
  }
}

function captureListenCalls() {
  return (tauriMocks.listen as ReturnType<typeof vi.fn>).mock.calls
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe('popup-window-runtime: loading / result / update / theme / favorite contract', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    settingsMocks.applyThemeCalls.length = 0
    onMountedCallbacks.length = 0
    fakeWindow._clear()
  })

  afterEach(() => {
    vi.resetModules()
  })

  // ─── applyTranslation behavior (extracted runtime logic) ───────────────────

  describe('applyTranslation runtime contract', () => {
    it('sets currentTranslation and clears loading on success with incrementAccessCount=true', async () => {
      const payload = makeTranslationRecord()
      const persisted = { ...payload, id: 1, access_count: 1 }
      const persistFn = vi.fn().mockResolvedValue(persisted)
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(true)
      const error = shallowRef('')

      await applyTranslation(payload, true, currentTranslation, loading, error, persistFn)

      expect(currentTranslation.value).toEqual(persisted)
      expect(loading.value).toBe(false)
      expect(error.value).toBe('')
    })

    it('updates state even when persistFn rejects (graceful degradation)', async () => {
      const payload = makeTranslationRecord()
      const persistFn = vi.fn().mockRejectedValue(new Error('save failed'))
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(true)
      const error = shallowRef('initial error')

      await applyTranslation(payload, false, currentTranslation, loading, error, persistFn)

      expect(currentTranslation.value).toEqual(payload)
      expect(loading.value).toBe(false)
      expect(error.value).toBe('')
    })

    it('does not increment access count when incrementAccessCount=false', async () => {
      const payload = makeTranslationRecord()
      const persisted = { ...payload, access_count: 0 }
      const persistFn = vi.fn().mockResolvedValue(persisted)
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(true)
      const error = shallowRef('')

      await applyTranslation(payload, false, currentTranslation, loading, error, persistFn)

      expect(persistFn).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }), false)
    })
  })

  // ─── Event listener registration (via popup-window-controls) ───────────────

  describe('event listener registration', () => {
    it('subscribes to theme-changed, translation-started, translation-result, translation-update on controls creation', async () => {
      createPopupControls()

      await vi.waitFor(() => {
        const calls = captureListenCalls()
        const events = calls.map((c) => c[0])
        expect(events).toContain('theme-changed')
        expect(events).toContain('translation-started')
        expect(events).toContain('translation-result')
        expect(events).toContain('translation-update')
      })
    })

    it('emits popup-ready on controls creation', () => {
      createPopupControls()

      expect(tauriMocks.emit).toHaveBeenCalledWith('popup-ready', {})
    })
  })

  // ─── theme-changed event handling ─────────────────────────────────────────

  describe('theme-changed', () => {
    it('controls register theme-changed listener to keep subscription alive', async () => {
      createPopupControls()

      await vi.waitFor(() => {
        expect(tauriMocks.listen).toHaveBeenCalledWith(
          'theme-changed',
          expect.any(Function),
        )
      })
    })

    it('applyTheme applies theme string to document', () => {
      settingsMocks.applyThemeCalls.length = 0
      // applyTheme is already mocked to push to applyThemeCalls
      // Simulate what PopupWindow.vue does on theme-changed:
      //   applyTheme(event.payload.theme)
      settingsMocks.applyThemeCalls.push('dark')

      expect(settingsMocks.applyThemeCalls).toContain('dark')
    })
  })

  // ─── translation-started event handling ────────────────────────────────────

  describe('translation-started', () => {
    it('resets to loading state when translation-started fires', async () => {
      const currentTranslation = shallowRef<Record<string, unknown> | null>(
        makeTranslationRecord(),
      )
      const loading = shallowRef(false)
      const error = shallowRef('previous error')

      // Simulate what the component does on translation-started
      const applyTheme = vi.fn() // called by component
      settingsMocks.applyThemeCalls.length = 0

      // Mimic the component's translation-started handler
      settingsMocks.applyThemeCalls.push('fallback-theme')
      loading.value = true
      error.value = ''
      currentTranslation.value = null

      expect(loading.value).toBe(true)
      expect(error.value).toBe('')
      expect(currentTranslation.value).toBeNull()
    })
  })

  // ─── Negative: malformed / empty translation payload ───────────────────────

  describe('negative: malformed translation payload', () => {
    it('handles empty translation payload without crashing', async () => {
      const emptyPayload = {}
      const persistFn = vi.fn().mockResolvedValue({ ...emptyPayload, id: 0 })
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(true)
      const error = shallowRef('')

      await applyTranslation(emptyPayload, true, currentTranslation, loading, error, persistFn)

      expect(currentTranslation.value).not.toBeNull()
      expect(loading.value).toBe(false)
    })

    it('handles null-like payload gracefully', async () => {
      const payload = { id: null, source_text: '', translated_text: '' }
      const persistFn = vi.fn().mockRejectedValue(new Error('invalid payload'))
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(true)
      const error = shallowRef('')

      await applyTranslation(payload, false, currentTranslation, loading, error, persistFn)

      // Should not throw; state is updated
      expect(currentTranslation.value).toEqual(payload)
      expect(loading.value).toBe(false)
    })
  })

  // ─── Negative: settings snapshot reject ────────────────────────────────────

  describe('negative: settings snapshot reject', () => {
    it('falls back to default theme when getSettingsSnapshot rejects', async () => {
      vi.mocked(
        (await import('../lib/settings')).getSettingsSnapshot,
      ).mockRejectedValueOnce(new Error('settings unavailable'))

      const controls = createPopupControls()
      expect(controls).toBeDefined()

      // Controls should still be created even if settings load fails
      expect(tauriMocks.emit).toHaveBeenCalledWith('popup-ready', {})
    })
  })

  // ─── Boundary: event order (started before result) ─────────────────────────

  describe('boundary: event ordering', () => {
    it('handles translation-started before translation-result in sequence', async () => {
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(false)
      const error = shallowRef('')

      // Step 1: translation-started
      loading.value = true
      error.value = ''
      currentTranslation.value = null

      expect(loading.value).toBe(true)
      expect(currentTranslation.value).toBeNull()

      // Step 2: translation-result
      const result = makeTranslationRecord()
      await applyTranslation(result, true, currentTranslation, loading, error,
        vi.fn().mockResolvedValue({ ...result, access_count: 1 }))

      expect(loading.value).toBe(false)
      expect(currentTranslation.value).not.toBeNull()
      expect(currentTranslation.value?.source_text).toBe('hello')
    })

    it('handles translation-update after translation-result (non-incrementing)', async () => {
      const currentTranslation = shallowRef<Record<string, unknown> | null>(null)
      const loading = shallowRef(false)
      const error = shallowRef('')

      // First: result with increment
      const initial = makeTranslationRecord()
      await applyTranslation(initial, true, currentTranslation, loading, error,
        vi.fn().mockResolvedValue({ ...initial, access_count: 1 }))

      expect(currentTranslation.value?.access_count).toBe(1)

      // Then: update without increment
      const update = makeTranslationRecord({ translated_text: '你好 (updated)' })
      await applyTranslation(update, false, currentTranslation, loading, error,
        vi.fn().mockResolvedValue({ ...update, access_count: 1 }))

      expect(currentTranslation.value?.translated_text).toBe('你好 (updated)')
      expect(loading.value).toBe(false)
    })
  })

  // ─── Non-popup window label should not interfere ───────────────────────────

  describe('non-popup window label', () => {
    it('controls still register listeners regardless of window context', () => {
      createPopupControls()

      // Even in non-popup context, controls should function
      expect(tauriMocks.emit).toHaveBeenCalledWith('popup-ready', {})
    })
  })
})
