// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createTestRouter, createTranslationRecord } from '../test-utils'
import { useTranslationStore } from '../stores/translation'
import DetailPage from './DetailPage.vue'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ label: 'main', onFocusChanged: async () => () => {} }),
}))
const invokeMock = vi.mocked(invoke)

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
})

describe('DetailPage favorite persistence', () => {
  it.each([0, 1])('keeps favorite=%s on failure, displays error, and retries successfully', async (isFavorite) => {
    const record = createTranslationRecord({ is_favorite: isFavorite })
    invokeMock.mockResolvedValueOnce({ ...record })
    const router = createTestRouter([{ path: '/detail/:id', component: DetailPage }])
    await router.push('/detail/1')
    await router.isReady()
    const wrapper = mount(DetailPage, { global: { plugins: [router] } })
    await flushPromises()
    const store = useTranslationStore()
    store.history = [{ ...record }]
    store.currentTranslation = { ...record }
    const originalLabel = wrapper.get('.favorite-btn').text()

    invokeMock.mockRejectedValueOnce(new Error('database unavailable'))
    await wrapper.get('.favorite-btn').trigger('click')
    await flushPromises()
    expect(wrapper.get('.favorite-btn').text()).toBe(originalLabel)
    expect(wrapper.get('[role="alert"]').text()).toContain('database unavailable')
    expect(store.currentTranslation?.is_favorite).toBe(isFavorite)
    expect(store.history[0].is_favorite).toBe(isFavorite)

    invokeMock.mockResolvedValueOnce(undefined)
    await wrapper.get('.favorite-btn').trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenLastCalledWith('toggle_favorite', { id: 1, isFavorite: !isFavorite })
    expect(wrapper.get('.favorite-btn').text()).not.toBe(originalLabel)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(store.history[0].is_favorite).toBe(isFavorite ? 0 : 1)
    wrapper.unmount()
  })
})
