<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Translation } from '../stores/translation'
import type Database from '@tauri-apps/plugin-sql'
import { openTranslationsDatabase } from '../lib/database'

const currentTranslation = ref<Translation | null>(null)
const loading = ref(true)
const error = ref('')
const appWindow = getCurrentWebviewWindow()

let db: Database | null = null
let unlistenTranslationResult: (() => void) | null = null

function serializeStringList(items?: string[]) {
  return items ? JSON.stringify(items) : null
}

onMounted(async () => {
  // 初始化数据库
  try {
    db = await openTranslationsDatabase()
  } catch (e) {
    console.error('数据库初始化失败:', e)
  }

  // 监听翻译结果
  unlistenTranslationResult = await listen<Translation>('translation-result', async (event) => {
    const nextTranslation: Translation = {
      ...event.payload
    }

    // 保存到数据库
    if (db) {
      try {
        const explainsJson = serializeStringList(nextTranslation.explains)
        const examplesJson = serializeStringList(nextTranslation.examples)
        const synonymsJson = serializeStringList(nextTranslation.synonyms)

        await db.execute(
          `INSERT INTO translations (source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 0)
           ON CONFLICT(source_text, source_lang, target_lang)
           DO UPDATE SET
             translated_text = $2,
             phonetic = $3,
             us_phonetic = $4,
             uk_phonetic = $5,
             audio_url = $6,
             explains = $7,
             examples = $8,
             synonyms = $9,
             word_type = $12,
             access_count = access_count + 1,
             created_at = $13`,
          [
            nextTranslation.source_text,
            nextTranslation.translated_text,
            nextTranslation.phonetic,
            nextTranslation.us_phonetic,
            nextTranslation.uk_phonetic,
            nextTranslation.audio_url,
            explainsJson,
            examplesJson,
            synonymsJson,
            nextTranslation.source_lang,
            nextTranslation.target_lang,
            nextTranslation.word_type,
            nextTranslation.created_at,
            nextTranslation.access_count,
          ]
        )

        // 查询刚保存的记录，获取 id 和 is_favorite
        const results = await db.select<Translation[]>(
          'SELECT * FROM translations WHERE source_text = $1 AND source_lang = $2 AND target_lang = $3',
          [
            nextTranslation.source_text,
            nextTranslation.source_lang,
            nextTranslation.target_lang,
          ]
        )
        if (results.length > 0) {
          nextTranslation.id = results[0].id
          nextTranslation.is_favorite = results[0].is_favorite
        }
      } catch (e) {
        console.error('保存翻译失败:', e)
      }
    }

    currentTranslation.value = nextTranslation
    loading.value = false
  })

  // 监听 ESC 键关闭窗口
  window.addEventListener('keydown', handleKeyDown)

  // 通知后端前端已就绪
  await appWindow.emit('popup-ready', {})
})

// 清理事件监听
onUnmounted(() => {
  unlistenTranslationResult?.()
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
  if (!db || !currentTranslation.value) {
    return
  }

  const newState = currentTranslation.value.is_favorite ? 0 : 1

  try {
    await db.execute(
      'UPDATE translations SET is_favorite = $1 WHERE source_text = $2 AND source_lang = $3 AND target_lang = $4',
      [
        newState,
        currentTranslation.value.source_text,
        currentTranslation.value.source_lang,
        currentTranslation.value.target_lang,
      ]
    )
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

    <div v-else-if="currentTranslation" class="content">
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
        <p class="translation-text">{{ currentTranslation.translated_text }}</p>
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
  background: white;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
  background: white;
}

.word-section {
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 2px solid #f0f0f0;
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
  color: #333;
  margin: 0;
  word-break: break-word;
  flex: 1;
}

.audio-btn {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
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
  box-shadow: 0 2px 8px rgba(102, 126, 234, 0.3);
}

.audio-btn:hover {
  transform: scale(1.1);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.5);
}

.audio-btn:active {
  transform: scale(0.95);
}

.favorite-btn {
  width: 100%;
  padding: 10px;
  margin-bottom: 16px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 2px 8px rgba(102, 126, 234, 0.3);
}

.favorite-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.5);
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
  color: #666;
  font-family: 'Courier New', monospace;
}

.phonetic .label {
  display: inline-block;
  background: #667eea;
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
  color: #666;
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
  background: #f8f9fa;
  border-radius: 6px;
  font-size: 14px;
  color: #333;
  line-height: 1.6;
  border-left: 3px solid #667eea;
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
  background: rgba(102, 126, 234, 0.12);
  border: 1px solid rgba(102, 126, 234, 0.2);
  border-radius: 999px;
  color: #4f5fc7;
  font-size: 13px;
}

.translation-section {
  margin-bottom: 16px;
}

.translation-text {
  font-size: 16px;
  color: #333;
  line-height: 1.8;
  margin: 0;
  padding: 12px;
  background: #f8f9fa;
  border-radius: 8px;
  border-left: 3px solid #667eea;
}

.loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: white;
  font-size: 14px;
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
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
  color: white;
  padding: 20px;
  text-align: center;
  font-size: 14px;
}

.error-icon {
  font-size: 32px;
}
</style>
