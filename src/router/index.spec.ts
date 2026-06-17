import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  createRouter: vi.fn((config) => ({
    ...config,
    push: vi.fn(),
  })),
  createWebHistory: vi.fn(() => ({ type: 'history' })),
}))

vi.mock('vue-router', () => ({
  createRouter: mocks.createRouter,
  createWebHistory: mocks.createWebHistory,
}))

describe('router', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('registers translate as the default redirect', async () => {
    const { routes } = await import('./index')

    expect(routes[0]).toEqual({
      path: '/',
      redirect: '/translate',
    })
  })

  it('includes translate and popup routes in the route table', async () => {
    const { routes } = await import('./index')

    expect(routes).toEqual(expect.arrayContaining([
      expect.objectContaining({
        path: '/translate',
        name: 'Translate',
      }),
      expect.objectContaining({
        path: '/popup',
        name: 'Popup',
      }),
    ]))
  })

  it('creates the router with web history and exported routes', async () => {
    const routerModule = await import('./index')

    expect(mocks.createWebHistory).toHaveBeenCalledTimes(1)
    expect(mocks.createRouter).toHaveBeenCalledWith(expect.objectContaining({
      history: { type: 'history' },
      routes: routerModule.routes,
    }))
    expect(typeof routerModule.default.push).toBe('function')
  })
})
