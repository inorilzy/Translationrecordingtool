// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { RouterView } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { createTestRouter, createTranslationRecord } from '../test-utils'
import { useTranslationStore, type Translation } from '../stores/translation'
import HistoryPage from './HistoryPage.vue'
import FavoritesPage from './FavoritesPage.vue'
import DetailPage from './DetailPage.vue'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({ getCurrentWebviewWindow: vi.fn() }))
const invokeMock = vi.mocked(invoke)
let focus: (event: { payload: boolean }) => void
let cleanup: ReturnType<typeof vi.fn<() => void>>
let subscribe: ReturnType<typeof vi.fn>
let records: Translation[]

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

async function open(path: string) {
  const router = createTestRouter([
    { path: '/history', component: HistoryPage },
    { path: '/favorites', component: FavoritesPage },
    { path: '/detail/:id', component: DetailPage },
    { path: '/other', component: { template: '<div>other</div>' } },
  ])
  await router.push(path)
  await router.isReady()
  const wrapper = mount(RouterView, { global: { plugins: [router] } })
  await flushPromises()
  return { wrapper, router }
}

beforeEach(() => {
  vi.resetAllMocks()
  setActivePinia(createPinia())
  cleanup = vi.fn()
  subscribe = vi.fn(async (handler) => { focus = handler; return cleanup })
  vi.mocked(getCurrentWebviewWindow).mockReturnValue({ label: 'main', onFocusChanged: subscribe } as unknown as ReturnType<typeof getCurrentWebviewWindow>)
  records = [createTranslationRecord({ is_favorite: 1 })]
  invokeMock.mockImplementation(async (command, args) => {
    if (command === 'load_history') return records.map((item) => ({ ...item }))
    if (command === 'load_favorites') return records.filter((item) => item.is_favorite).map((item) => ({ ...item }))
    if (command === 'get_translation_by_id') return { ...records.find((item) => item.id === (args as { id: number }).id) }
    throw new Error(`unexpected command: ${command}`)
  })
})

describe('mounted main window revalidation', () => {
  it.each(['/history', '/favorites', '/detail/1'])('refreshes only the mounted %s view on main focus', async (path) => {
    const { wrapper } = await open(path)
    expect(wrapper.text()).toContain('你好')
    records[0] = { ...records[0], translated_text: 'enriched remotely', is_favorite: 0 }
    records.push(createTranslationRecord({ id: 2, source_text: 'new word', translated_text: 'new result', is_favorite: 1 }))
    const store = useTranslationStore()
    store.error = 'unrelated translation error'
    focus({ payload: false })
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledTimes(1)
    focus({ payload: true })
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledTimes(2)
    expect(subscribe).toHaveBeenCalledTimes(1)
    if (path === '/favorites') {
      expect(wrapper.text()).toContain('new result')
      expect(wrapper.text()).not.toContain('你好')
    } else {
      expect(wrapper.text()).toContain('enriched remotely')
    }
    if (path === '/history') expect(wrapper.text()).toContain('new result')
    if (path === '/detail/1') expect(wrapper.get('.favorite-btn').text()).toBe('☆ 收藏')
    expect(store.error).toBe('unrelated translation error')
    wrapper.unmount()
    expect(cleanup).toHaveBeenCalledOnce()
  })

  it('does not subscribe when the window is popup', async () => {
    vi.mocked(getCurrentWebviewWindow).mockReturnValue({ label: 'popup', onFocusChanged: subscribe } as unknown as ReturnType<typeof getCurrentWebviewWindow>)
    const { wrapper } = await open('/history')
    expect(subscribe).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it.each(['/history', '/favorites', '/detail/1'])('discards older refresh results for %s', async (path) => {
    const { wrapper } = await open(path)
    const old = deferred<Translation[] | Translation>()
    invokeMock.mockImplementationOnce(() => old.promise)
    focus({ payload: true })
    records[0].translated_text = 'newest content'
    focus({ payload: true })
    await flushPromises()
    old.resolve(path === '/detail/1' ? createTranslationRecord({ translated_text: 'stale content' }) : [createTranslationRecord({ translated_text: 'stale content' })])
    await flushPromises()
    expect(wrapper.text()).toContain('newest content')
    expect(wrapper.text()).not.toContain('stale content')
    wrapper.unmount()
  })

  it('keeps favorites on read failure and displays an error instead of an empty list', async () => {
    const { wrapper } = await open('/favorites')
    invokeMock.mockRejectedValueOnce(new Error('synthetic read failure'))
    focus({ payload: true })
    await flushPromises()
    expect(wrapper.findAll('.favorite-card')).toHaveLength(1)
    expect(wrapper.get('[role="alert"]').text()).toContain('synthetic read failure')
    expect(wrapper.text()).not.toContain('暂无收藏')
    wrapper.unmount()
  })

  it('does not claim an empty favorites list when initial loading fails', async () => {
    invokeMock.mockRejectedValueOnce(new Error('initial read failure'))
    const { wrapper } = await open('/favorites')
    expect(wrapper.get('[role="alert"]').text()).toContain('initial read failure')
    expect(wrapper.text()).not.toContain('暂无收藏')
    wrapper.unmount()
  })

  it('drops pending history updates and focus events after unmount', async () => {
    const { wrapper, router } = await open('/history')
    const old = deferred<Translation[]>()
    invokeMock.mockImplementationOnce(() => old.promise)
    focus({ payload: true })
    await router.push('/other')
    await flushPromises()
    old.resolve([createTranslationRecord({ translated_text: 'late content' })])
    await flushPromises()
    expect(useTranslationStore().history[0].translated_text).toBe('你好')
    const calls = invokeMock.mock.calls.length
    focus({ payload: true })
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledTimes(calls)
    expect(cleanup).toHaveBeenCalledOnce()
    wrapper.unmount()
  })

  it('disposes a listener that registers after the page was unmounted', async () => {
    let releaseRegistration!: (unlisten: () => void) => void
    const registration = new Promise<() => void>((resolve) => { releaseRegistration = resolve })
    subscribe.mockImplementationOnce(() => registration)
    const { wrapper } = await open('/history')
    wrapper.unmount()
    releaseRegistration(cleanup)
    await flushPromises()
    expect(cleanup).toHaveBeenCalledOnce()
  })

  it('does not overwrite a new detail route with an old ID response', async () => {
    records.push(createTranslationRecord({ id: 2, source_text: 'second', translated_text: 'second detail' }))
    const { wrapper, router } = await open('/detail/1')
    const old = deferred<Translation>()
    invokeMock.mockImplementationOnce(() => old.promise)
    focus({ payload: true })
    await router.push('/detail/2')
    await flushPromises()
    old.resolve(createTranslationRecord({ translated_text: 'old ID response' }))
    await flushPromises()
    expect(wrapper.text()).toContain('second detail')
    expect(wrapper.text()).not.toContain('old ID response')
    expect(invokeMock).toHaveBeenLastCalledWith('get_translation_by_id', { id: 2 })
    wrapper.unmount()
  })
})
