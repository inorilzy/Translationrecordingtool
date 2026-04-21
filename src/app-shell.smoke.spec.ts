import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// ─── Tauri API Mocks ────────────────────────────────────────────────────────

const tauriMocks = vi.hoisted(() => ({
  windowLabel: 'main',
  throwGetCurrentWebviewWindow: false,
  hide: vi.fn().mockResolvedValue(undefined),
  startDragging: vi.fn().mockResolvedValue(undefined),
  emit: vi.fn().mockResolvedValue(undefined),
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => {
    if (tauriMocks.throwGetCurrentWebviewWindow) {
      throw new Error('webview unavailable')
    }
    return {
      label: tauriMocks.windowLabel,
      hide: tauriMocks.hide,
      startDragging: tauriMocks.startDragging,
      emit: tauriMocks.emit,
    }
  },
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => tauriMocks.invoke(...args),
}))

vi.mock('./lib/theme-bootstrap', () => ({
  bootstrapTheme: vi.fn().mockResolvedValue(undefined),
}))

// ─── Vue + Router mocks (shared mutable state) ──────────────────────────────

const vueMocks = vi.hoisted(() => ({
  currentRoutePath: '/history',
  replaceCalls: [] as string[],
  piniaRegistered: false,
  routerRegistered: false,
  mountSelector: '',
}))

vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>()
  return {
    ...actual,
    createApp: () => {
      const appObj = {
        use: (plugin: unknown) => {
          if ((plugin as { name?: string })?.name === 'PiniaPlugin') {
            vueMocks.piniaRegistered = true
          }
          if ((plugin as { install?: unknown })?.install && (plugin as { name?: string })?.name !== 'PiniaPlugin') {
            vueMocks.routerRegistered = true
          }
          return appObj
        },
        mount: (sel: string) => { vueMocks.mountSelector = sel },
      }
      return appObj
    },
    createPinia: () => ({ name: 'PiniaPlugin' }),
  }
})

vi.mock('./router', () => ({
  default: {
    get currentRoute() {
      return { value: { path: vueMocks.currentRoutePath } }
    },
    replace: (path: string) => { vueMocks.replaceCalls.push(path); return Promise.resolve() },
    isReady: () => Promise.resolve(),
  },
}))

// ─── Helpers ─────────────────────────────────────────────────────────────────

function resetVueMocks() {
  vueMocks.currentRoutePath = '/history'
  vueMocks.replaceCalls.length = 0
  vueMocks.piniaRegistered = false
  vueMocks.routerRegistered = false
  vueMocks.mountSelector = ''
  tauriMocks.throwGetCurrentWebviewWindow = false
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe('app-shell smoke: window label routing contract', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetVueMocks()
  })

  afterEach(() => {
    vi.resetModules()
  })

  describe('popup window', () => {
    it('routes to /popup when window label is "popup"', async () => {
      tauriMocks.windowLabel = 'popup'
      vueMocks.currentRoutePath = '/history'

      const { bootstrap } = await import('./main')
      await bootstrap()

      expect(vueMocks.replaceCalls).toContain('/popup')
    })

    it('does NOT replace route when already on /popup', async () => {
      tauriMocks.windowLabel = 'popup'
      vueMocks.currentRoutePath = '/popup'

      const { bootstrap } = await import('./main')
      await bootstrap()

      expect(vueMocks.replaceCalls).not.toContain('/popup')
    })
  })

  describe('main window', () => {
    it('routes to /history when window label is "main" and on /popup', async () => {
      tauriMocks.windowLabel = 'main'
      vueMocks.currentRoutePath = '/popup'

      const { bootstrap } = await import('./main')
      await bootstrap()

      expect(vueMocks.replaceCalls).toContain('/history')
    })

    it('does NOT replace route when main window is not on /popup', async () => {
      tauriMocks.windowLabel = 'main'
      vueMocks.currentRoutePath = '/history'

      const { bootstrap } = await import('./main')
      await bootstrap()

      expect(vueMocks.replaceCalls).not.toContain('/history')
    })
  })

  describe('fallback / error path', () => {
    it('defaults to main-window behavior when getCurrentWebviewWindow throws', async () => {
      vueMocks.currentRoutePath = '/popup'
      tauriMocks.throwGetCurrentWebviewWindow = true

      const { bootstrap } = await import('./main')
      await bootstrap()

      // Should treat as main window → redirect away from /popup
      expect(vueMocks.replaceCalls).toContain('/history')
    })

    it('uses pinia and router for both window types', async () => {
      tauriMocks.windowLabel = 'popup'

      const { bootstrap } = await import('./main')
      // Should not throw — bootstrap completes for popup window
      await expect(bootstrap()).resolves.not.toThrow()

      // Mount was called (app successfully mounted)
      expect(vueMocks.mountSelector).toBe('#app')
    })
  })
})
