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
let unlistenTheme: (() => void) | null = null

function currentThemeFallback() {
  return document.documentElement.getAttribute('data-theme')
    || defaultSettings.theme
}

function applyTheme(theme = currentThemeFallback()) {
  applyDocumentTheme(theme)
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

  // ESC key listener registered by createPopupControls
})

// 清理事件监听
onUnmounted(() => {
  unlistenTranslationStarted?.()
  unlistenTranslationResult?.()
  unlistenTranslationUpdate?.()
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
        ✕
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
      <span class="error-icon">⚠️</span>
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
}

.popup-header {
  display: flex;
  align-items: center;
  height: 36px;
  flex-shrink: 0;
  background: var(--color-bg-tertiary);
  border-bottom: var(--border-width) solid var(--color-border);
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
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  font-size: var(--font-size-md);
  cursor: var(--cursor-auto);
  transition: all var(--transition-fast);
  -webkit-app-region: no-drag;
  flex-shrink: 0;
}

.close-button:hover {
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
}

.close-button:active {
  background: var(--color-bg-tertiary);
}

.content {
  flex: 1;
  padding: var(--spacing-lg);
  overflow-y: auto;
  background: var(--color-bg-primary);
}

.word-section {
  margin-bottom: var(--spacing-lg);
  padding-bottom: var(--spacing-md);
  border-bottom: var(--border-width-active) solid var(--color-border);
}

.word-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.word {
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0;
  word-break: break-word;
  flex: 1;
}

.audio-btn {
  background: var(--color-primary);
  border: none;
  border-radius: var(--radius-full);
  width: 36px;
  height: 36px;
  font-size: var(--font-size-lg);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--transition-fast);
  box-shadow: var(--shadow-sm);
}

.audio-btn:hover {
  transform: scale(1.1);
  background: var(--color-primary-hover);
  box-shadow: var(--shadow-md);
}

.audio-btn:active {
  transform: scale(0.95);
}

.quick-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
}

.action-btn {
  padding: var(--spacing-sm) var(--spacing-md);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--font-size-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  box-shadow: var(--shadow-sm);
}

.favorite-btn {
  background: var(--color-primary);
  color: var(--color-text-on-primary);
}

.main-entry-btn {
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  border: var(--border-width) solid var(--color-border);
}

.action-btn:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

.favorite-btn:hover {
  background: var(--color-primary-hover);
}

.main-entry-btn:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.action-btn:active {
  transform: translateY(0);
}

.phonetics {
  display: flex;
  gap: var(--spacing-md);
  flex-wrap: wrap;
}

.phonetic {
  font-size: var(--font-size-md);
  color: var(--color-text-secondary);
  font-family: 'Courier New', monospace;
}

.phonetic .label {
  display: inline-block;
  background: var(--color-primary);
  color: var(--color-text-on-primary);
  padding: var(--spacing-xs) 6px;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-xs);
  margin-right: var(--spacing-xs);
  font-weight: var(--font-weight-semibold);
}

.section-title {
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
  margin: 0 0 var(--spacing-md) 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.explains-section {
  margin-bottom: var(--spacing-md);
}

.explains-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.explains-list li {
  padding: var(--spacing-sm) var(--spacing-md);
  margin-bottom: var(--spacing-xs);
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  font-size: var(--font-size-md);
  color: var(--color-text-primary);
  line-height: 1.6;
  border-left: var(--border-width-active) solid var(--color-primary);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-sm);
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 6px var(--spacing-sm);
  background: var(--color-primary-light);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--radius-pill);
  color: var(--color-primary);
  font-size: var(--font-size-sm);
}

.translation-section {
  margin-bottom: var(--spacing-md);
}

.translation-text {
  font-size: 16px;
  color: var(--color-text-primary);
  line-height: 1.8;
  margin: 0;
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  border-left: var(--border-width-active) solid var(--color-primary);
}

.translation-word-type {
  display: inline-block;
  margin-right: var(--spacing-sm);
  color: var(--color-primary);
  font-weight: var(--font-weight-semibold);
}

.loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-md);
  color: var(--color-text-primary);
  font-size: var(--font-size-md);
}

.spinner {
  width: 32px;
  height: 32px;
  border: var(--border-width-active) solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: var(--radius-full);
  animation: spin 0.8s linear infinite;
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
  gap: var(--spacing-sm);
  color: var(--color-error);
  padding: var(--spacing-lg);
  text-align: center;
  font-size: var(--font-size-md);
}

.error-icon {
  font-size: var(--font-size-icon);
}
</style>
