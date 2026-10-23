<script setup lang="ts">
import { computed, nextTick, ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { LogicalSize } from '@tauri-apps/api/dpi'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Translation } from '../stores/translation'
import { applyTheme as applyDocumentTheme, defaultSettings, getSettingsSnapshot } from '../lib/settings'
import { createPopupControls } from './popup-window-controls'

const POPUP_WIDTH = 420
const POPUP_HEADER_HEIGHT = 30
const POPUP_MIN_HEIGHT = 150
const POPUP_MAX_HEIGHT = 380
const POPUP_BODY_MAX_HEIGHT = POPUP_MAX_HEIGHT - POPUP_HEADER_HEIGHT

const currentTranslation = ref<Translation | null>(null)
const loading = ref(true)
const loadingMessage = ref('翻译中...')
const error = ref('')
const bodyRef = ref<HTMLElement | null>(null)

// Window controls (ESC, close, drag, popup-ready)
const controls = createPopupControls()

const isSentence = computed(() => {
  const text = currentTranslation.value?.source_text?.trim() ?? ''
  if (!text) {
    return false
  }
  return /[\s,，.。!！?？;；:：]/.test(text)
    || /[\u3400-\u9fff]/.test(text)
    || text.length > 24
})

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

async function resizePopupToContent() {
  await nextTick()
  const bodyHeight = Math.min(bodyRef.value?.scrollHeight ?? 0, POPUP_BODY_MAX_HEIGHT)
  const height = Math.max(
    POPUP_MIN_HEIGHT,
    Math.min(POPUP_MAX_HEIGHT, POPUP_HEADER_HEIGHT + bodyHeight),
  )

  try {
    await getCurrentWebviewWindow().setSize(new LogicalSize(POPUP_WIDTH, height))
  } catch (cause) {
    console.error('调整弹窗高度失败:', cause)
  }
}

async function applyTranslation(payload: Translation, resetScroll: boolean) {
  currentTranslation.value = { ...payload }
  loading.value = false
  error.value = ''

  if (resetScroll) {
    await nextTick()
    window.scrollTo({ top: 0, left: 0, behavior: 'auto' })
    bodyRef.value?.scrollTo({ top: 0, left: 0, behavior: 'auto' })
  }
}

watch(
  [loading, error, currentTranslation],
  () => {
    void resizePopupToContent()
  },
  { flush: 'post', deep: true },
)

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

  void resizePopupToContent()
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
    <header class="popup-header" data-testid="popup-header">
      <span class="popup-title">翻译</span>
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

    <div ref="bodyRef" class="popup-body">
      <div v-if="loading" class="state state-loading loading">
        <span class="spinner"></span>
        <span>{{ loadingMessage }}</span>
      </div>

      <template v-else-if="currentTranslation">
        <article v-if="isSentence" class="sentence-result">
          <p
            class="sentence-source"
            :title="currentTranslation.source_text"
          >
            {{ currentTranslation.source_text }}
          </p>
          <div class="sentence-rule"></div>
          <p class="sentence-translation">{{ currentTranslation.translated_text }}</p>

          <div class="sentence-actions">
            <button class="text-action" @click="toggleFavorite">
              {{ currentTranslation.is_favorite ? '★ 已收藏' : '☆ 收藏' }}
            </button>
            <button class="text-action" @click="openMainWindow">
              主界面 ↗
            </button>
          </div>
        </article>

        <article v-else class="word-result">
          <div class="word-section">
            <div class="word-header">
              <h2 class="word">{{ currentTranslation.source_text }}</h2>
              <button v-if="currentTranslation.audio_url" @click="playAudio" class="audio-btn" title="播放发音">
                🔊
              </button>
            </div>

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

          <div class="word-actions">
            <button @click="toggleFavorite" class="action-btn favorite-btn">
              {{ currentTranslation.is_favorite ? '★ 已收藏' : '☆ 收藏' }}
            </button>
            <button @click="openMainWindow" class="action-btn main-entry-btn">
              进入主界面
            </button>
          </div>

          <div class="translation-section">
            <h3 class="section-title">翻译</h3>
            <p class="translation-text">
              <span v-if="currentTranslation.word_type" class="translation-word-type">
                {{ currentTranslation.word_type }}
              </span>
              <span>{{ currentTranslation.translated_text }}</span>
            </p>
          </div>

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
        </article>
      </template>

      <div v-else-if="error" class="state state-error error">
        <span class="error-badge">!</span>
        <div class="error-copy">
          <strong>翻译失败</strong>
          <p>{{ error }}</p>
        </div>
      </div>

      <div v-else class="state state-loading loading">
        等待翻译...
      </div>
    </div>
  </div>
</template>

<style scoped>
* {
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb-light) transparent;
}

*::-webkit-scrollbar {
  width: 5px;
  height: 5px;
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
  height: 30px;
  flex-shrink: 0;
  padding-left: 12px;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-border);
  cursor: var(--cursor-grab);
  user-select: none;
  -webkit-user-select: none;
}

.popup-title {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
}

.drag-region {
  flex: 1;
  height: 100%;
  -webkit-app-region: drag;
}

.close-button {
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--color-text-tertiary);
  font-size: 15px;
  line-height: 1;
  cursor: var(--cursor-auto);
  transition: background var(--transition-fast), color var(--transition-fast);
  -webkit-app-region: no-drag;
  flex-shrink: 0;
}

.close-button:hover {
  background: var(--color-bg-tertiary);
  color: var(--color-text-primary);
}

.close-button:active {
  background: var(--color-border);
}

.popup-body {
  flex: 0 1 auto;
  max-height: 350px;
  padding: 14px 16px;
  overflow-y: auto;
  background: var(--color-bg-primary);
}

.state {
  min-height: 90px;
}

.state-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--color-text-secondary);
  font-size: 12.5px;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: var(--radius-full);
  animation: spin 0.75s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ── Sentence result ───────────────────────────────────────────────────── */

.sentence-result {
  padding: 2px 0;
}

.sentence-source {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: 12.5px;
  line-height: 1.55;
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.sentence-rule {
  height: 1px;
  margin: 12px 0;
  background: var(--color-border);
  opacity: 0.75;
}

.sentence-translation {
  margin: 0;
  color: var(--color-app-text-strong);
  font-family: var(--font-family-display);
  font-size: 17px;
  line-height: 1.62;
  letter-spacing: -0.01em;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.sentence-actions {
  display: flex;
  justify-content: flex-end;
  gap: 14px;
  margin-top: 14px;
}

.text-action {
  padding: 2px 0;
  border: none;
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: color var(--transition-fast);
}

.text-action:hover {
  color: var(--color-primary);
}

/* ── Word result ───────────────────────────────────────────────────────── */

.word-section {
  margin-bottom: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--color-border);
}

.word-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.word {
  font-family: var(--font-family-display);
  font-size: 22px;
  font-weight: 650;
  letter-spacing: -0.025em;
  color: var(--color-app-text-strong);
  margin: 0;
  word-break: break-word;
  flex: 1;
  line-height: 1.25;
}

.audio-btn {
  background: var(--color-primary);
  border: none;
  border-radius: var(--radius-full);
  width: 30px;
  height: 30px;
  font-size: 13px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast), transform var(--transition-fast);
  box-shadow: var(--shadow-sm);
  color: var(--color-text-on-primary);
  flex-shrink: 0;
}

.audio-btn:hover {
  transform: scale(1.04);
  background: var(--color-primary-hover);
}

.audio-btn:active {
  transform: scale(0.97);
}

.phonetics {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 8px;
}

.phonetic {
  font-size: 12.5px;
  color: var(--color-text-secondary);
  font-family: var(--font-family-mono);
}

.phonetic .label {
  display: inline-block;
  background: var(--color-chip-bg);
  color: var(--color-app-accent-strong);
  border: 1px solid var(--color-chip-border);
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  font-size: 10px;
  margin-right: 5px;
  font-weight: 650;
  font-family: var(--font-family-ui);
}

.word-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 7px;
  margin: 12px 0;
}

.action-btn {
  padding: 7px 10px;
  border: none;
  border-radius: var(--radius-md);
  font-size: 12.5px;
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

.section-title {
  font-size: 10px;
  font-weight: 650;
  color: var(--color-text-tertiary);
  margin: 0 0 6px 0;
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.explains-section {
  margin-bottom: 10px;
}

.explains-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.explains-list li {
  padding: 7px 10px;
  margin-bottom: 4px;
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  font-size: 12.5px;
  color: var(--color-text-primary);
  line-height: 1.5;
  border-left: 2px solid var(--color-primary);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 9px;
  background: var(--color-chip-bg);
  border: 1px solid var(--color-chip-border);
  border-radius: var(--radius-pill);
  color: var(--color-app-accent-strong);
  font-size: 11.5px;
  font-weight: 600;
}

.translation-section {
  margin-bottom: 10px;
}

.translation-text {
  font-size: 14.5px;
  color: var(--color-text-primary);
  line-height: 1.65;
  margin: 0;
  padding: 9px 11px;
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
  border-left: 2px solid var(--color-primary);
  overflow-wrap: anywhere;
}

.translation-word-type {
  display: inline-block;
  margin-right: 7px;
  color: var(--color-primary);
  font-weight: 650;
}

/* ── Error ─────────────────────────────────────────────────────────────── */

.state-error {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 0 4px;
}

.error-badge {
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-pill);
  background: var(--color-error-bg);
  color: var(--color-error);
  font-size: 13px;
  font-weight: 700;
  flex-shrink: 0;
}

.error-copy {
  min-width: 0;
}

.error-copy strong {
  display: block;
  margin-bottom: 4px;
  color: var(--color-app-text-strong);
  font-size: 13px;
  font-weight: 650;
}

.error-copy p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 12.5px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
</style>
