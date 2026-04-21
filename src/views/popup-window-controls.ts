import { onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

/**
 * Thin boundary that manages popup window controls:
 * - Emits `popup-ready` on mount
 * - Listens for `theme-changed` events
 * - Handles ESC key to close the window
 * - Exposes close() and startDragging() for the custom chrome
 *
 * Designed to be testable with mocked Tauri APIs.
 */
export interface PopupControls {
  /** Hide the popup window. */
  close: () => void
  /** Start a window drag (for custom title-bar drag region). */
  startDragging: () => void
  /** Keydown handler — closes on ESC. */
  handleKeyDown: (e: KeyboardEvent) => void
  /** Call during onUnmounted to clean up listeners. */
  cleanup: () => void
}

export function createPopupControls(): PopupControls {
  const appWindow = getCurrentWebviewWindow()

  let unlistenTheme: (() => void) | null = null
  let unlistenTranslationStarted: (() => void) | null = null
  let unlistenTranslationResult: (() => void) | null = null
  let unlistenTranslationUpdate: (() => void) | null = null

  function cleanup() {
    unlistenTranslationStarted?.()
    unlistenTranslationResult?.()
    unlistenTranslationUpdate?.()
    unlistenTheme?.()
    window.removeEventListener('keydown', handleKeyDown)
  }

  onUnmounted(cleanup)

  function close() {
    appWindow.hide()
  }

  function startDragging() {
    appWindow.startDragging()
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      close()
    }
  }

  // Register keydown listener
  window.addEventListener('keydown', handleKeyDown)

  // Async event listeners (Tauri) — set up but errors will surface
  // rather than being silently swallowed.
  listen<{ theme: string }>('theme-changed', (_event) => {
    // Theme change is handled by the component; this listener
    // keeps the event subscription alive so the backend doesn't
    // consider the popup stale.
  }).then((unlisten) => {
    unlistenTheme = unlisten
  }).catch((e) => {
    console.error('Failed to listen for theme-changed:', e)
  })

  listen('translation-started', () => {
    // Forwarded to component via existing logic
  }).then((unlisten) => {
    unlistenTranslationStarted = unlisten
  }).catch((e) => {
    console.error('Failed to listen for translation-started:', e)
  })

  listen('translation-result', () => {
    // Forwarded to component via existing logic
  }).then((unlisten) => {
    unlistenTranslationResult = unlisten
  }).catch((e) => {
    console.error('Failed to listen for translation-result:', e)
  })

  listen('translation-update', () => {
    // Forwarded to component via existing logic
  }).then((unlisten) => {
    unlistenTranslationUpdate = unlisten
  }).catch((e) => {
    console.error('Failed to listen for translation-update:', e)
  })

  // Notify backend that the popup frontend is ready
  appWindow.emit('popup-ready', {}).catch((e) => {
    console.error('Failed to emit popup-ready:', e)
  })

  return { close, startDragging, handleKeyDown, cleanup }
}
