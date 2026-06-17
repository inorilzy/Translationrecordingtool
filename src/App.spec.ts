import { describe, expect, it, vi } from 'vitest'
import { runStartupLoad } from './App.vue'

describe('App', () => {
  it('loads settings and history for the main window', async () => {
    const settingsStore = {
      loadSettings: vi.fn().mockResolvedValue(undefined),
    }
    const translationStore = {
      loadHistory: vi.fn().mockResolvedValue(undefined),
    }

    await runStartupLoad('main', settingsStore, translationStore)

    expect(settingsStore.loadSettings).toHaveBeenCalledTimes(1)
    expect(translationStore.loadHistory).toHaveBeenCalledTimes(1)
  })

  it('skips startup loading for the popup window', async () => {
    const settingsStore = {
      loadSettings: vi.fn().mockResolvedValue(undefined),
    }
    const translationStore = {
      loadHistory: vi.fn().mockResolvedValue(undefined),
    }

    await runStartupLoad('popup', settingsStore, translationStore)

    expect(settingsStore.loadSettings).not.toHaveBeenCalled()
    expect(translationStore.loadHistory).not.toHaveBeenCalled()
  })
})
