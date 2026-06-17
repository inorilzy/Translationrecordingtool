import { test, expect } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    type EventHandler = (payload: unknown) => void
    type InvokeArgs = Record<string, unknown> | undefined
    type TauriInternals = {
      metadata: {
        currentWindow: { label: string }
        currentWebview: { windowLabel: string; label: string }
      }
      invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>
      transformCallback: (fn: EventHandler) => number
      unregisterCallback: (id: number) => void
      convertFileSrc: (path: string) => string
    }

    const listeners = new Map<string, EventHandler[]>()
    const callbacks = new Map<number, EventHandler>()
    let callbackId = 0

    const settings = {
      apiKey: '',
      apiSecret: '',
      globalShortcut: 'Ctrl+Q',
      enableTray: true,
      theme: 'light',
    }

    const history = [
      {
        id: 1,
        source_text: 'hello',
        translated_text: '你好',
        phonetic: '/həˈloʊ/',
        us_phonetic: null,
        uk_phonetic: null,
        audio_url: null,
        explains: ['int. 你好'],
        examples: ['hello world'],
        synonyms: ['hi'],
        source_lang: 'en',
        target_lang: 'zh',
        word_type: 'int.',
        created_at: 1710000000,
        access_count: 1,
        is_favorite: 0,
      },
    ]

    const emit = (event: string, payload: unknown) => {
      const handlers = listeners.get(event) ?? []
      for (const handler of handlers) {
        handler(payload)
      }
    }

    ;(window as Window & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { windowLabel: 'main', label: 'main' },
      },
      invoke: async (cmd: string, args?: Record<string, unknown>) => {
        switch (cmd) {
          case 'get_settings':
            return settings
          case 'load_history':
            return history
          case 'translate_text':
            return {
              id: 2,
              source_text: String(args?.text ?? ''),
              translated_text: '测试译文',
              phonetic: '/test/',
              us_phonetic: null,
              uk_phonetic: null,
              audio_url: null,
              explains: ['n. 测试'],
              examples: ['test example'],
              synonyms: ['exam'],
              source_lang: 'en',
              target_lang: 'zh',
              word_type: 'n.',
              created_at: 1710000001,
              access_count: 0,
              is_favorite: 0,
            }
          case 'save_translation': {
            const persisted = {
              ...(args?.translation as Record<string, unknown>),
              access_count: 1,
            }
            history.unshift(persisted as never)
            return persisted
          }
          case 'plugin:event|listen': {
            const event = String(args?.event ?? '')
            const handlerId = Number(args?.handler)
            const arr = listeners.get(event) ?? []
            arr.push((payload) => {
              const callback = callbacks.get(handlerId)
              if (callback) {
                callback({ event, payload })
              }
            })
            listeners.set(event, arr)
            return handlerId
          }
          case 'plugin:event|unlisten':
            return null
          case 'plugin:event|emit':
            emit(String(args?.event ?? ''), args?.payload)
            return null
          default:
            return null
        }
      },
      transformCallback: (fn: (payload: unknown) => void) => {
        callbackId += 1
        callbacks.set(callbackId, fn)
        return callbackId
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id)
      },
      convertFileSrc: (path: string) => path,
    }
  })
})

test('main window golden path translates text and shows result', async ({ page }) => {
  await page.goto('/translate')

  await expect(page.getByRole('heading', { name: '手动翻译' })).toBeVisible()
  await page.getByRole('textbox').fill('hello world')
  await page.locator('.translate-btn').click()

  await expect(page.getByRole('heading', { name: '翻译结果' })).toBeVisible()
  await expect(page.getByText('测试译文')).toBeVisible()
})
