import { describe, expect, it, vi } from 'vitest'
import { submitTranslation } from './TranslatePage.vue'

describe('TranslatePage', () => {
  it('trims input before calling translateText', async () => {
    const translateText = vi.fn().mockResolvedValue(undefined)

    await submitTranslation('  hello world  ', translateText)

    expect(translateText).toHaveBeenCalledWith('hello world')
  })

  it('does not call translateText for blank input', async () => {
    const translateText = vi.fn().mockResolvedValue(undefined)

    await submitTranslation('   ', translateText)

    expect(translateText).not.toHaveBeenCalled()
  })
})
