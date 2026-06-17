import { describe, expect, it } from 'vitest'
import { isNavigationPathActive } from './NavigationBar.vue'

describe('NavigationBar', () => {
  it('marks the current route as active only for exact matches', () => {
    expect(isNavigationPathActive('/translate', '/translate')).toBe(true)
    expect(isNavigationPathActive('/translate', '/history')).toBe(false)
  })
})
