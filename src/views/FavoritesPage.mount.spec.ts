// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createTestRouter, createTranslationRecord } from '../test-utils'
import FavoritesPage from './FavoritesPage.vue'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ label: 'main', onFocusChanged: async () => () => {} }),
}))
const invokeMock = vi.mocked(invoke)

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
})

describe('FavoritesPage favorite persistence', () => {
  it('preserves the list and displays failure without reloading, then retries successfully', async () => {
    invokeMock.mockResolvedValueOnce([createTranslationRecord({ is_favorite: 1 })])
    const router = createTestRouter([{ path: '/favorites', component: FavoritesPage }])
    await router.push('/favorites')
    await router.isReady()
    const wrapper = mount(FavoritesPage, { global: { plugins: [router] } })
    await flushPromises()
    expect(wrapper.findAll('.favorite-card')).toHaveLength(1)

    invokeMock.mockRejectedValueOnce(new Error('database unavailable'))
    await wrapper.get('.remove-btn').trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledTimes(2)
    expect(wrapper.findAll('.favorite-card')).toHaveLength(1)
    expect(wrapper.get('[role="alert"]').text()).toContain('database unavailable')

    invokeMock.mockResolvedValueOnce(undefined).mockResolvedValueOnce([])
    await wrapper.get('.remove-btn').trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'toggle_favorite', { id: 1, isFavorite: false })
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'load_favorites')
    expect(wrapper.findAll('.favorite-card')).toHaveLength(0)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    wrapper.unmount()
  })
})
