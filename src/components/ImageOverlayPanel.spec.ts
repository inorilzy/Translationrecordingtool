import { describe, expect, it } from 'vitest'
import { overlayBlockStyle } from './ImageOverlayPanel.vue'

describe('overlayBlockStyle', () => {
  it('converts image coordinates into percentage styles', () => {
    const style = overlayBlockStyle(
      { x: 50, y: 25, width: 100, height: 20 },
      200,
      100,
    )

    expect(style.left).toBe('25%')
    expect(style.top).toBe('25%')
    expect(style.width).toBe('50%')
    expect(style.height).toBe('20%')
    expect(Number.parseFloat(style.fontSize)).toBeCloseTo(14.4, 5)
  })
})
