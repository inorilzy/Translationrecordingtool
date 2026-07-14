import { onUnmounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

/**
 * Owns popup window controls only: ready signaling, Escape, close, drag, and cleanup.
 * Theme and translation domain events belong to PopupWindow.vue.
 */
export interface PopupControls {
  close: () => void
  signalReady: () => Promise<void>
  startDragging: () => void
  handleKeyDown: (event: KeyboardEvent) => void
  cleanup: () => void
}

export function createPopupControls(): PopupControls {
  const appWindow = getCurrentWebviewWindow()

  function close() {
    appWindow.hide()
  }

  function startDragging() {
    appWindow.startDragging()
  }

  async function signalReady() {
    try {
      await appWindow.emit('popup-ready', {})
    } catch (error) {
      console.error('Failed to emit popup-ready:', error)
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      close()
    }
  }

  function cleanup() {
    window.removeEventListener('keydown', handleKeyDown)
  }

  window.addEventListener('keydown', handleKeyDown)
  onUnmounted(cleanup)


  return { close, signalReady, startDragging, handleKeyDown, cleanup }
}
