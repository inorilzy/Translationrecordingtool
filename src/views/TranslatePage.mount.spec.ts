// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, nextTick } from 'vue'
import { createTestRouter, createTranslationRecord } from '../test-utils'
import TranslatePage from './TranslatePage.vue'

const translationStore = vi.hoisted(() => ({
  translateText: vi.fn().mockResolvedValue(undefined),
  translateFromClipboard: vi.fn().mockResolvedValue(undefined),
  translateScreenshot: vi.fn().mockResolvedValue(undefined),
  loading: false,
  error: '',
  currentTranslation: null as ReturnType<typeof createTranslationRecord> | null,
}))

const settingsStore = vi.hoisted(() => ({
  globalShortcut: 'Ctrl+Q',
}))

vi.mock('../stores/translation', () => ({
  useTranslationStore: () => translationStore,
}))

vi.mock('../stores/settings', () => ({
  useSettingsStore: () => settingsStore,
}))

const TranslationCardStub = defineComponent({
  props: {
    translation: {
      type: Object,
      required: true,
    },
  },
  computed: {
    translatedText() {
      return (this.translation as { translated_text: string }).translated_text
    },
  },
  template: '<div data-testid="translation-card">{{ translatedText }}</div>',
})

async function mountPage() {
  const router = createTestRouter([
    { path: '/translate', component: TranslatePage },
    { path: '/history', component: { template: '<div>History</div>' } },
    { path: '/favorites', component: { template: '<div>Favorites</div>' } },
    { path: '/logs', component: { template: '<div>Logs</div>' } },
    { path: '/settings', component: { template: '<div>Settings</div>' } },
  ])

  await router.push('/translate')
  await router.isReady()

  return mount(TranslatePage, {
    global: {
      plugins: [router],
      stubs: {
        TranslationCard: TranslationCardStub,
      },
    },
  })
}

describe('TranslatePage mounted interactions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    translationStore.loading = false
    translationStore.error = ''
    translationStore.currentTranslation = null
    settingsStore.globalShortcut = 'Ctrl+Q'
  })

  it('renders shortcut text from settings store', async () => {
    settingsStore.globalShortcut = 'Ctrl+Shift+T'
    const wrapper = await mountPage()

    expect(wrapper.text()).toContain('剪贴板翻译 (Ctrl+Shift+T)')
  })

  it('submits trimmed text when clicking translate', async () => {
    const wrapper = await mountPage()

    await wrapper.get('textarea').setValue('  hello world  ')
    await wrapper.get('.translate-btn').trigger('click')

    expect(translationStore.translateText).toHaveBeenCalledWith('hello world')
  })

  it('does not submit blank text and keeps translate button disabled', async () => {
    const wrapper = await mountPage()
    const button = wrapper.get('.translate-btn')

    expect((button.element as HTMLButtonElement).disabled).toBe(true)

    await wrapper.get('textarea').setValue('   ')
    await nextTick()

    expect((button.element as HTMLButtonElement).disabled).toBe(true)
    expect(translationStore.translateText).not.toHaveBeenCalled()
  })

  it('triggers clipboard translation button', async () => {
    const wrapper = await mountPage()

    await wrapper.get('.clipboard-btn').trigger('click')

    expect(translationStore.translateFromClipboard).toHaveBeenCalledTimes(1)
  })

  it('triggers screenshot OCR translation button', async () => {
    const wrapper = await mountPage()

    await wrapper.get('.screenshot-btn').trigger('click')

    expect(translationStore.translateScreenshot).toHaveBeenCalledTimes(1)
  })

  it('disables all translate actions while the store is loading', async () => {
    translationStore.loading = true
    const wrapper = await mountPage()

    expect((wrapper.get('.translate-btn').element as HTMLButtonElement).disabled).toBe(true)
    expect((wrapper.get('.clipboard-btn').element as HTMLButtonElement).disabled).toBe(true)
    expect((wrapper.get('.screenshot-btn').element as HTMLButtonElement).disabled).toBe(true)
    expect(wrapper.text()).toContain('翻译中...')
    expect(wrapper.text()).toContain('处理中...')
  })

  it('renders error and translation result when store state is populated', async () => {
    translationStore.error = '翻译失败: network'
    translationStore.currentTranslation = createTranslationRecord({
      translated_text: '你好，世界',
    })

    const wrapper = await mountPage()

    expect(wrapper.text()).toContain('翻译失败: network')
    expect(wrapper.text()).toContain('翻译结果')
    expect(wrapper.get('[data-testid="translation-card"]').text()).toContain('你好，世界')
  })
})
