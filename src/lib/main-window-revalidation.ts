import { onMounted, onUnmounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

/** Revalidate only the mounted page; late queries and listener registrations lose ownership. */
export function useMainWindowRevalidation(load: (isCurrent: () => boolean) => Promise<void>) {
  let disposed = false
  let revision = 0
  let unlisten: (() => void) | undefined

  async function refresh() {
    const request = ++revision
    await load(() => !disposed && request === revision)
  }

  onMounted(async () => {
    void refresh()
    try {
      const window = getCurrentWebviewWindow()
      if (window.label !== 'main') return
      const cleanup = await window.onFocusChanged(({ payload }) => {
        if (payload && !disposed) void refresh()
      })
      if (disposed) cleanup()
      else unlisten = cleanup
    } catch (error) {
      // Browser preview has no native window; initial loading still works.
      console.warn('无法监听主窗口焦点变化:', error)
    }
  })

  onUnmounted(() => {
    disposed = true
    revision++
    unlisten?.()
  })

  return refresh
}
