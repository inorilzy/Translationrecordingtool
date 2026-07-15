import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

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

// Attach to globalThis BEFORE any module import (inside vi.hoisted for ordering)
vi.hoisted(() => {
  // @ts-ignore - we're deliberately setting a global
  globalThis.window = fakeWindow
})

// ─── Tauri API Mocks ────────────────────────────────────────────────────────

const mocks = vi.hoisted(() => ({
  hide: vi.fn().mockResolvedValue(undefined),
  startDragging: vi.fn().mockResolvedValue(undefined),
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    hide: mocks.hide,
    startDragging: mocks.startDragging,
    emit: mocks.emit,
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
}))

// ─── Vue lifecycle mock ─────────────────────────────────────────────────────

const cleanupRegistry: Array<() => void> = []

vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>()
  return {
    ...actual,
    onUnmounted: (fn: () => void) => {
      cleanupRegistry.push(fn)
    },
  }
})

// ─── Import after mocks ─────────────────────────────────────────────────────

import { createPopupControls } from './popup-window-controls'

// ─── Helpers ─────────────────────────────────────────────────────────────────

function fireKeydown(key: string) {
  // Node environment has no KeyboardEvent; simulate with a plain object
  const event = { type: 'keydown', key } as unknown as KeyboardEvent
  fakeWindow.dispatchEvent(event)
}

function runUnmountedCleanup() {
  for (const fn of cleanupRegistry) {
    fn()
  }
  cleanupRegistry.length = 0
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe('popup-window-controls: ready / close / ESC / drag contract', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    cleanupRegistry.length = 0
    fakeWindow._clear()
  })

  afterEach(() => {
    runUnmountedCleanup()
  })

  describe('popup-ready', () => {
    it('emits popup-ready only when signaled', async () => {
      const controls = createPopupControls()

      expect(mocks.emit).not.toHaveBeenCalled()
      await controls.signalReady()
      expect(mocks.emit).toHaveBeenCalledWith('popup-ready', {})
    })

    it('propagates popup-ready emission failures', async () => {
      mocks.emit.mockRejectedValueOnce(new Error('IPC unavailable'))
      const controls = createPopupControls()

      await expect(controls.signalReady()).rejects.toThrow('IPC unavailable')
    })
  })

  describe('close', () => {
    it('calls window.hide() when close() is invoked', () => {
      const controls = createPopupControls()

      controls.close()

      expect(mocks.hide).toHaveBeenCalledTimes(1)
    })
  })

  describe('ESC key', () => {
    it('closes the window when ESC is pressed', () => {
      createPopupControls()

      fireKeydown('Escape')

      expect(mocks.hide).toHaveBeenCalledTimes(1)
    })

    it('does NOT close the window for non-ESC keys', () => {
      createPopupControls()

      fireKeydown('Enter')
      fireKeydown('Tab')
      fireKeydown('a')

      expect(mocks.hide).not.toHaveBeenCalled()
    })
  })

  describe('drag', () => {
    it('calls startDragging() when startDragging() is invoked', () => {
      const controls = createPopupControls()

      controls.startDragging()

      expect(mocks.startDragging).toHaveBeenCalledTimes(1)
    })

    it('close() does NOT call startDragging()', () => {
      const controls = createPopupControls()

      controls.close()

      expect(mocks.startDragging).not.toHaveBeenCalled()
    })
  })

  describe('listener lifecycle', () => {
    it('registers a keydown listener on mount', () => {
      createPopupControls()

      expect(fakeWindow.addEventListener).toHaveBeenCalledWith(
        'keydown',
        expect.any(Function),
      )
    })

    it('removes the keydown listener on unmount', () => {
      createPopupControls()
      runUnmountedCleanup()

      expect(fakeWindow.removeEventListener).toHaveBeenCalledWith(
        'keydown',
        expect.any(Function),
      )
    })

    it('does not fire close after unmount even if ESC is pressed', () => {
      createPopupControls()
      runUnmountedCleanup()

      fireKeydown('Escape')

      expect(mocks.hide).not.toHaveBeenCalled()
    })
  })

  describe('Tauri event ownership', () => {
    it('does not subscribe to theme or translation events', async () => {
      createPopupControls()

      await Promise.resolve()
      expect(mocks.listen).not.toHaveBeenCalled()
    })
  })
})
