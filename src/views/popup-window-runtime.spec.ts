// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { TranslationRecord } from '../stores/translation-records'

interface MockEvent {
  payload: unknown
}

type MockEventHandler = (event: MockEvent) => unknown
type MockUnlisten = () => void

const windowMocks = vi.hoisted(() => ({
  hide: vi.fn().mockResolvedValue(undefined),
  startDragging: vi.fn().mockResolvedValue(undefined),
  emit: vi.fn().mockResolvedValue(undefined),
}))

const eventMocks = vi.hoisted(() => {
  const handlers: Record<string, MockEventHandler | undefined> = {}
  const unlisteners: Record<string, MockUnlisten | undefined> = {}
  const listen = vi.fn(async (eventName: string, handler: MockEventHandler) => {
    handlers[eventName] = handler
    const unlisten = vi.fn()
    unlisteners[eventName] = unlisten
    return unlisten
  })

  return { handlers, unlisteners, listen }
})

const coreMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}))

const settingsMocks = vi.hoisted(() => ({
  applyTheme: vi.fn(),
  getSettingsSnapshot: vi.fn().mockResolvedValue({ theme: 'light' }),
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    hide: windowMocks.hide,
    startDragging: windowMocks.startDragging,
    emit: windowMocks.emit,
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: eventMocks.listen,
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: coreMocks.invoke,
}))

vi.mock('../lib/settings', () => ({
  applyTheme: settingsMocks.applyTheme,
  defaultSettings: { theme: 'light' },
  getSettingsSnapshot: settingsMocks.getSettingsSnapshot,
}))

import PopupWindow from './PopupWindow.vue'

function translationRecord(overrides: Partial<TranslationRecord> = {}): TranslationRecord {
  return {
    id: 1,
    source_text: 'hello',
    translated_text: '你好',
    phonetic: null,
    us_phonetic: null,
    uk_phonetic: null,
    audio_url: null,
    explains: ['int. 你好'],
    examples: [],
    synonyms: [],
    source_lang: 'en',
    target_lang: 'zh',
    word_type: 'int.',
    created_at: 100,
    access_count: 1,
    is_favorite: 0,
    ...overrides,
  }
}

async function mountPopup() {
  const wrapper = mount(PopupWindow)
  await vi.waitFor(() => {
    expect(eventMocks.listen).toHaveBeenCalledTimes(5)
  })
  return wrapper
}

async function emitPopupEvent(eventName: string, payload: unknown) {
  const handler = eventMocks.handlers[eventName]
  expect(handler).toBeDefined()
  await handler?.({ payload })
  await flushPromises()
}

describe('PopupWindow runtime contract', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    for (const key of Object.keys(eventMocks.handlers)) {
      delete eventMocks.handlers[key]
    }
    for (const key of Object.keys(eventMocks.unlisteners)) {
      delete eventMocks.unlisteners[key]
    }
    document.documentElement.removeAttribute('data-theme')
    Object.defineProperty(window, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    })
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    })
  })

  it('owns each theme and translation listener exactly once', async () => {
    const wrapper = await mountPopup()

    expect(eventMocks.listen.mock.calls.map(([eventName]) => eventName)).toEqual([
      'theme-changed',
      'translation-started',
      'translation-result',
      'translation-update',
      'translation-failed',
    ])

    expect(windowMocks.emit).toHaveBeenCalledWith('popup-ready', {})
    const listenerOrders = eventMocks.listen.mock.invocationCallOrder
    const lastListenerOrder = listenerOrders[listenerOrders.length - 1]
    const readyOrder = windowMocks.emit.mock.invocationCallOrder[0]
    expect(lastListenerOrder).toBeLessThan(readyOrder)

    wrapper.unmount()
    for (const eventName of [
      'theme-changed',
      'translation-started',
      'translation-result',
      'translation-update',
      'translation-failed',
    ]) {
      expect(eventMocks.unlisteners[eventName]).toHaveBeenCalledTimes(1)
    }
  })

  it('renders loading, initial result, and enrichment as one popup state', async () => {
    const wrapper = await mountPopup()

    await emitPopupEvent('translation-started', { message: 'OCR 识别中...' })
    expect(wrapper.text()).toContain('OCR 识别中...')

    await emitPopupEvent('translation-result', translationRecord())
    expect(wrapper.find('.word').text()).toBe('hello')
    expect(wrapper.text()).toContain('int. 你好')

    await emitPopupEvent('translation-update', translationRecord({
      examples: ['Hello, world!'],
      synonyms: ['hi'],
    }))
    expect(wrapper.findAll('.word')).toHaveLength(1)
    expect(wrapper.text()).toContain('Hello, world!')
    expect(wrapper.text()).toContain('hi')
    expect(coreMocks.invoke).not.toHaveBeenCalledWith('save_translation', expect.anything())
  })

  it('renders workflow failures instead of closing silently', async () => {
    const wrapper = await mountPopup()

    await emitPopupEvent('translation-failed', { message: 'OCR 服务不可用' })

    expect(wrapper.find('.error').text()).toContain('OCR 服务不可用')
    expect(wrapper.find('.loading').exists()).toBe(false)
  })

  it('applies theme changes while the popup is open', async () => {
    await mountPopup()

    await emitPopupEvent('theme-changed', { theme: 'dark' })

    expect(settingsMocks.applyTheme).toHaveBeenLastCalledWith('dark')
  })

  it('keeps favorite and main-window actions on their existing commands', async () => {
    coreMocks.invoke.mockResolvedValue(undefined)
    const wrapper = await mountPopup()
    await emitPopupEvent('translation-result', translationRecord())

    await wrapper.find('.favorite-btn').trigger('click')
    expect(coreMocks.invoke).toHaveBeenCalledWith('toggle_favorite', {
      id: 1,
      isFavorite: true,
    })

    await wrapper.find('.main-entry-btn').trigger('click')
    expect(coreMocks.invoke).toHaveBeenCalledWith('open_main_translate_window')
    expect(windowMocks.hide).toHaveBeenCalledTimes(1)
  })
})
