<script setup lang="ts">
import { nextTick, ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Translation } from '../stores/translation'
import { applyTheme as applyDocumentTheme, defaultSettings, getSettingsSnapshot } from '../lib/settings'
import { createPopupControls } from './popup-window-controls'

const currentTranslation = ref<Translation | null>(null)
const loading = ref(true)
const loadingMessage = ref('翻译中...')
const error = ref('')
const contentRef = ref<HTMLElement | null>(null)

// Window controls (ESC, close, drag, popup-ready)
const controls = createPopupControls()

let unlistenTranslationResult: (() => void) | null = null
let unlistenTranslationUpdate: (() => void) | null = null
let unlistenTranslationStarted: (() => void) | null = null
let unlistenTranslationFailed: (() => void) | null = null
let unlistenTheme: (() => void) | null = null

function currentThemeFallback() {
  return document.documentElement.getAttribute('data-theme')
    || defaultSettings.theme
}

function applyTheme(theme = currentThemeFallback()) {
  applyDocumentTheme(theme)
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}


async function applyTranslation(payload: Translation, resetScroll: boolean) {
  currentTranslation.value = { ...payload }
  loading.value = false
  error.value = ''

  if (resetScroll) {
    await nextTick()
    window.scrollTo({ top: 0, left: 0, behavior: 'auto' })
    contentRef.value?.scrollTo({ top: 0, left: 0, behavior: 'auto' })
  }
}

onMounted(async () => {
  try {
    const settings = await getSettingsSnapshot()
    applyTheme(settings.theme)
  } catch (e) {
    console.error('加载弹窗主题失败，回退到默认主题:', e)
    applyTheme()
  }

  // 监听主题变化事件
  unlistenTheme = await listen<{ theme: string }>('theme-changed', (event) => {
    applyTheme(event.payload.theme)
  })

  // 监听翻译结果
  unlistenTranslationStarted = await listen<{ message?: string }>('translation-started', (event) => {
    applyTheme()
    loading.value = true
    loadingMessage.value = event.payload?.message || '翻译中...'
    error.value = ''
    currentTranslation.value = null
  })

  unlistenTranslationResult = await listen<Translation>('translation-result', async (event) => {
    applyTheme()
    await applyTranslation(event.payload, true)
  })

  unlistenTranslationUpdate = await listen<Translation>('translation-update', async (event) => {
    applyTheme()
    await applyTranslation(event.payload, false)
  })

  unlistenTranslationFailed = await listen<{ message?: string }>('translation-failed', (event) => {
    applyTheme()
    loading.value = false
    error.value = event.payload?.message || '翻译失败'
    currentTranslation.value = null
  })

  try {
    await controls.signalReady()
  } catch (cause) {
    console.error('弹窗就绪信号发送失败:', cause)
    loading.value = false
    currentTranslation.value = null
    error.value = `弹窗无法通知后端已就绪，请关闭后重试：${errorMessage(cause)}`
  }

  // ESC key listener registered by createPopupControls
})

// 清理事件监听
onUnmounted(() => {
  unlistenTranslationStarted?.()
  unlistenTranslationResult?.()
  unlistenTranslationUpdate?.()
  unlistenTranslationFailed?.()
  unlistenTheme?.()
  // ESC key listener cleanup handled by createPopupControls
})

async function toggleFavorite() {
  if (!currentTranslation.value?.id) {
    return
  }

  const newState = currentTranslation.value.is_favorite ? 0 : 1

  try {
    await invoke('toggle_favorite', {
      id: currentTranslation.value.id,
      isFavorite: newState === 1,
    })
    currentTranslation.value.is_favorite = newState
  } catch (e) {
    console.error('更新收藏状态失败:', e)
  }
}

async function openMainWindow() {
  try {
    await invoke('open_main_translate_window')
    controls.close()
  } catch (e) {
    console.error('打开主界面失败:', e)
  }
}

function playAudio() {
  if (currentTranslation.value?.audio_url) {
    const audio = new Audio(currentTranslation.value.audio_url)
    audio.play().catch(e => {
      console.error('播放音频失败:', e)
    })
  }
}

</script>

<template>
  <div class="popup-container">
    <!-- Custom header: drag region + close button (no system title bar) -->
    <header class="popup-header" data-testid="popup-header">
      <div class="drag-region" data-testid="drag-region"></div>
      <button
        class="close-button"
        data-testid="close-button"
        @click="controls.close"
        title="关闭"
        aria-label="关闭窗口"
      >
        ×
      </button>
    </header>

    <div v-if="loading" class="loading">
      <div class="spinner"></div>
      <span>{{ loadingMessage }}</span>
    </div>

    <div v-else-if="currentTranslation" ref="contentRef" class="content">
      <!-- 单词/短语 -->
      <div class="word-section">
        <div class="word-header">
          <h2 class="word">{{ currentTranslation.source_text }}</h2>
          <button v-if="currentTranslation.audio_url" @click="playAudio" class="audio-btn" title="播放发音">
            🔊
          </button>
        </div>

        <!-- 音标 -->
        <div v-if="currentTranslation.us_phonetic || currentTranslation.uk_phonetic || currentTranslation.phonetic" class="phonetics">
          <span v-if="currentTranslation.us_phonetic" class="phonetic">
            <span class="label">美</span> [{{ currentTranslation.us_phonetic }}]
          </span>
          <span v-if="currentTranslation.uk_phonetic" class="phonetic">
            <span class="label">英</span> [{{ currentTranslation.uk_phonetic }}]
          </span>
          <span v-if="!currentTranslation.us_phonetic && !currentTranslation.uk_phonetic && currentTranslation.phonetic" class="phonetic">
            [{{ currentTranslation.phonetic }}]
          </span>
        </div>
      </div>

      <div class="quick-actions">
        <button @click="toggleFavorite" class="action-btn favorite-btn">
          {{ currentTranslation.is_favorite ? '★' : '☆' }} {{ currentTranslation.is_favorite ? '已收藏' : '收藏' }}
        </button>
        <button @click="openMainWindow" class="action-btn main-entry-btn">
          进入主界面
        </button>
      </div>

      <!-- 中文翻译 -->
      <div class="translation-section">
        <h3 class="section-title">翻译</h3>
        <p class="translation-text">
          <span v-if="currentTranslation.word_type" class="translation-word-type">
            {{ currentTranslation.word_type }}
          </span>
          <span>{{ currentTranslation.translated_text }}</span>
        </p>
      </div>

      <!-- 基本释义 -->
      <div v-if="currentTranslation.explains && currentTranslation.explains.length > 0" class="explains-section">
        <h3 class="section-title">详细释义</h3>
        <ul class="explains-list">
          <li v-for="(explain, index) in currentTranslation.explains" :key="index">
            {{ explain }}
          </li>
        </ul>
      </div>

      <div v-if="currentTranslation.examples && currentTranslation.examples.length > 0" class="explains-section">
        <h3 class="section-title">例句</h3>
        <ul class="explains-list">
          <li v-for="(example, index) in currentTranslation.examples" :key="`example-${index}`">
            {{ example }}
          </li>
        </ul>
      </div>

      <div v-if="currentTranslation.synonyms && currentTranslation.synonyms.length > 0" class="explains-section">
        <h3 class="section-title">近义词</h3>
        <div class="chips">
          <span v-for="(synonym, index) in currentTranslation.synonyms" :key="`synonym-${index}`" class="chip">
            {{ synonym }}
          </span>
        </div>
      </div>
    </div>

    <div v-else-if="error" class="error">
      <span class="error-icon">!</span>
      <span>{{ error }}</span>
    </div>

    <div v-else class="loading">等待翻译...</div>
  </div>
</template>

<style scoped>
* {
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb-light) transparent;
}

*::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

*::-webkit-scrollbar-track {
  background: transparent;
}

*::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb-light);
  border-radius: var(--radius-sm);
}

*::-webkit-scrollbar-thumb:hover {
  background: var(--scrollbar-thumb-hover-light);
}

.popup-container {
  width: 100%;
  height: 100%;
  background: var(--color-bg-primary);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  font-family: var(--font-family-ui);
}

.popup-header {
  display: flex;
  align-items: center;
  height: 34px;
  flex-shrink: 0;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-border);
  cursor: var(--cursor-grab);
  user-select: none;
  -webkit-user-select: none;
}

.drag-region {
  flex: 1;
  height: 100%;
  -webkit-app-region: drag;
}

.close-button {
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  font-size: 16px;
  line-height: 1;
  cursor: var(--cursor-auto);
  transition: background var(--transition-fast), color var(--transition-fast);
  -webkit-app-region: no-drag;
  flex-shrink: 0;
}

.close-button:hover {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.close-button:active {
  background: var(--color-bg-tertiary);
}

.content {
  flex: 1;
  padding: 16px 18px 18px;
  overflow-y: auto;
  background: var(--color-bg-primary);
}

.word-section {
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--color-border);
}

.word-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}

.word {
  font-family: var(--font-family-display);
  font-size: 24px;
  font-weight: 650;
  letter-spacing: -0.03em;
  color: var(--color-app-text-strong);
  margin: 0;
  word-break: break-word;
  flex: 1;
  line-height: 1.2;
}

.audio-btn {
  background: var(--color-primary);
  border: none;
  border-radius: var(--radius-full);
  width: 34px;
  height: 34px;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast), transform var(--transition-fast);
  box-shadow: var(--shadow-sm);
  color: var(--color-text-on-primary);
}

.audio-btn:hover {
  transform: scale(1.04);
  background: var(--color-primary-hover);
}

.audio-btn:active {
  transform: scale(0.97);
}

.quick-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 14px;
}

.action-btn {
  padding: 9px 12px;
  border: none;
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
}

.favorite-btn {
  background: var(--color-primary);
  color: var(--color-text-on-primary);
}

.main-entry-btn {
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
}

.favorite-btn:hover {
  background: var(--color-primary-hover);
}

.main-entry-btn:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.phonetics {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.phonetic {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-family: var(--font-family-mono);
}

.phonetic .label {
  display: inline-block;
  background: var(--color-chip-bg);
  color: var(--color-app-accent-strong);
  border: 1px solid var(--color-chip-border);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  font-size: 11px;
  margin-right: 6px;
  font-weight: 650;
  font-family: var(--font-family-ui);
}

.section-title {
  font-size: 11px;
  font-weight: 650;
  color: var(--color-text-tertiary);
  margin: 0 0 8px 0;
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.explains-section {
  margin-bottom: 12px;
}

.explains-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.explains-list li {
  padding: 8px 12px;
  margin-bottom: 6px;
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  font-size: 13px;
  color: var(--color-text-primary);
  line-height: 1.55;
  border-left: 2px solid var(--color-primary);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 5px 10px;
  background: var(--color-chip-bg);
  border: 1px solid var(--color-chip-border);
  border-radius: var(--radius-pill);
  color: var(--color-app-accent-strong);
  font-size: 12px;
  font-weight: 600;
}

.translation-section {
  margin-bottom: 12px;
}

.translation-text {
  font-size: 15px;
  color: var(--color-text-primary);
  line-height: 1.7;
  margin: 0;
  padding: 10px 12px;
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  border-left: 2px solid var(--color-primary);
}

.translation-word-type {
  display: inline-block;
  margin-right: 8px;
  color: var(--color-primary);
  font-weight: 650;
}

.loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.spinner {
  width: 28px;
  height: 28px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: var(--radius-full);
  animation: spin 0.75s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.error {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--color-error);
  padding: 18px;
  text-align: center;
  font-size: 13px;
}

.error-icon {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: var(--color-error-bg);
  font-weight: 700;
}
</style>
