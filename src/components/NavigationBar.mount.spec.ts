// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createTestRouter } from '../test-utils'
import NavigationBar from './NavigationBar.vue'

async function mountBar(startPath = '/translate') {
  const router = createTestRouter([
    { path: '/translate', component: { template: '<div>Translate</div>' } },
    { path: '/history', component: { template: '<div>History</div>' } },
    { path: '/favorites', component: { template: '<div>Favorites</div>' } },
    { path: '/logs', component: { template: '<div>Logs</div>' } },
    { path: '/settings', component: { template: '<div>Settings</div>' } },
  ])

  await router.push(startPath)
  await router.isReady()

  const wrapper = mount(NavigationBar, {
    global: {
      plugins: [router],
    },
  })

  return { wrapper, router }
}

describe('NavigationBar mounted interactions', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
  })

  it('marks the current route button as active', async () => {
    const { wrapper } = await mountBar('/history')
    const buttons = wrapper.findAll('button')

    expect(buttons[1].classes()).toContain('active')
    expect(buttons[0].classes()).not.toContain('active')
  })

  it('navigates to another route when button is clicked', async () => {
    const { wrapper, router } = await mountBar('/translate')
    const pushSpy = vi.spyOn(router, 'push')
    const buttons = wrapper.findAll('button')

    await buttons[3].trigger('click')
    await nextTick()

    expect(pushSpy).toHaveBeenCalledWith('/logs')
  })
})
