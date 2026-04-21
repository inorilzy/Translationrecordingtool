<script setup lang="ts">
import { nextTick, ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Translation } from '../stores/translation'
import { applyTheme as applyDocumentTheme, defaultSettings, getSettingsSnapshot } from '../lib/settings'

const currentTranslation = ref<Translation | null>(null)
const loading = ref(true)
const error = ref('')
const contentRef = ref<HTMLElement | null>(null)
const appWindow = getCurrentWebviewWindow()
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

async function persistTranslation(nextTranslation: Translation, incrementAccessCount: boolean) {
  return invoke<Translation>('save_translation', {
    translation: nextTranslation,
    incrementAccessCount,
  })
}

async function applyTranslation(payload: Translation, incrementAccessCount: boolean) {
  let nextTranslation: Translation = {
    ...payload
  }

  try {
    nextTranslation = await persistTranslation(nextTranslation, incrementAccessCount)
  } catch (e) {
    console.error('保存翻译失败:', e)
  }

  currentTranslation.value = nextTranslation
  loading.value = false
  error.value = ''

  if (incrementAccessCount) {
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
  unlistenTranslationStarted = await listen('translation-started', () => {
    applyTheme()
    loading.value = true
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

  // 监听 ESC 键关闭窗口
  window.addEventListener('keydown', handleKeyDown)

  // 通知后端前端已就绪
  await appWindow.emit('popup-ready', {})
})

// 清理事件监听
onUnmounted(() => {
  unlistenTranslationStarted?.()
  unlistenTranslationResult?.()
  unlistenTranslationUpdate?.()
  unlistenTheme?.()
  window.removeEventListener('keydown', handleKeyDown)
})

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    close()
  }
}

function close() {
  appWindow.hide()
}

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
    <div v-if="loading" class="loading">
      <div class="spinner"></div>
      <span>翻译中...</span>
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

      <!-- 收藏按钮 -->
      <button @click="toggleFavorite" class="favorite-btn">
        {{ currentTranslation.is_favorite ? '★' : '☆' }} {{ currentTranslation.is_favorite ? '已收藏' : '收藏' }}
      </button>

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
  scrollbar-color: rgba(102, 126, 234, 0.5) transparent;
}

*::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

*::-webkit-scrollbar-track {
  background: transparent;
}

*::-webkit-scrollbar-thumb {
  background: rgba(102, 126, 234, 0.5);
  border-radius: 3px;
}

*::-webkit-scrollbar-thumb:hover {
  background: rgba(102, 126, 234, 0.7);
}

.popup-container {
  width: 100%;
  height: 100%;
  background: var(--color-bg-primary);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
  background: var(--color-bg-primary);
}

.word-section {
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 2px solid var(--color-border);
}

.word-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.word {
  font-size: 24px;
  font-weight: 600;
  color: var(--color-text-primary);
  margin: 0;
  word-break: break-word;
  flex: 1;
}

.audio-btn {
  background: var(--color-primary);
  border: none;
  border-radius: 50%;
  width: 36px;
  height: 36px;
  font-size: 18px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
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

.favorite-btn {
  width: 100%;
  padding: 10px;
  margin-bottom: 16px;
  background: var(--color-primary);
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: var(--shadow-sm);
}

.favorite-btn:hover {
  transform: translateY(-2px);
  background: var(--color-primary-hover);
  box-shadow: var(--shadow-md);
}

.favorite-btn:active {
  transform: translateY(0);
}

.phonetics {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.phonetic {
  font-size: 14px;
  color: var(--color-text-secondary);
  font-family: 'Courier New', monospace;
}

.phonetic .label {
  display: inline-block;
  background: var(--color-primary);
  color: white;
  padding: 2px 6px;
  border-radius: 3px;
  font-size: 11px;
  margin-right: 4px;
  font-weight: 600;
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-secondary);
  margin: 0 0 12px 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.explains-section {
  margin-bottom: 16px;
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
  border-radius: 6px;
  font-size: 14px;
  color: var(--color-text-primary);
  line-height: 1.6;
  border-left: 3px solid var(--color-primary);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 6px 10px;
  background: var(--color-primary-light);
  border: 1px solid var(--color-border);
  border-radius: 999px;
  color: var(--color-primary);
  font-size: 13px;
}

.translation-section {
  margin-bottom: 16px;
}

.translation-text {
  font-size: 16px;
  color: var(--color-text-primary);
  line-height: 1.8;
  margin: 0;
  padding: 12px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
  border-left: 3px solid var(--color-primary);
}

.translation-word-type {
  display: inline-block;
  margin-right: 8px;
  color: var(--color-primary);
  font-weight: 600;
}

.loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--color-text-primary);
  font-size: 14px;
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
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
  gap: 8px;
  color: var(--color-error);
  padding: 20px;
  text-align: center;
  font-size: 14px;
}

.error-icon {
  font-size: 32px;
}
</style>
