<script lang="ts">
export async function submitTranslation(
  inputText: string,
  translateText: (text: string) => Promise<void>,
) {
  const text = inputText.trim()
  if (!text) return
  await translateText(text)
}
</script>

<script setup lang="ts">
import { ref } from 'vue'
import { useTranslationStore } from '../stores/translation'
import { useSettingsStore } from '../stores/settings'
import NavigationBar from '../components/NavigationBar.vue'
import TranslationCard from '../components/TranslationCard.vue'

const store = useTranslationStore()
const settings = useSettingsStore()
const inputText = ref('')

async function handleTranslate() {
  await submitTranslation(inputText.value, store.translateText)
}
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="header">
      <h1>手动翻译</h1>
      <p class="subtitle">输入或粘贴文本，获取翻译结果</p>
    </div>

    <!-- 翻译输入区 -->
    <div class="translate-section">
      <div class="input-group">
        <textarea
          v-model="inputText"
          @keydown.ctrl.enter="handleTranslate"
          @keydown.meta.enter="handleTranslate"
          placeholder="输入要翻译的文本，按 Ctrl+Enter 或 Cmd+Enter 快速翻译"
          class="translate-input"
          rows="5"
          autofocus
        />
        <div class="input-actions">
          <button
            @click="handleTranslate"
            :disabled="store.loading || !inputText.trim()"
            class="btn btn-primary translate-btn"
          >
            {{ store.loading ? '翻译中...' : '翻译' }}
          </button>
          <span class="divider">或</span>
          <button
            @click="store.translateFromClipboard"
            :disabled="store.loading"
            class="btn btn-secondary clipboard-btn"
          >
            {{ store.loading ? '翻译中...' : `剪贴板翻译 (${settings.globalShortcut})` }}
          </button>
          <button
            @click="store.translateScreenshot"
            :disabled="store.loading"
            class="btn btn-secondary screenshot-btn"
          >
            {{ store.loading ? '处理中...' : '截图 OCR 翻译' }}
          </button>
        </div>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="store.error" class="error-message">{{ store.error }}</div>

    <!-- 当前翻译结果 -->
    <div v-if="store.currentTranslation" class="current-result">
      <h3>翻译结果</h3>
      <TranslationCard
        :translation="store.currentTranslation"
        :show-favorite="true"
      />
    </div>
  </div>
</template>

<style scoped>
.header {
  text-align: center;
  margin-bottom: var(--spacing-lg);
}

.header h1 {
  font-size: var(--font-size-lg);
  color: var(--color-text-primary);
  margin: 0;
}

.subtitle {
  margin: var(--spacing-xs) 0 0;
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.translate-section {
  margin: var(--spacing-lg) 0;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-md);
  max-width: 640px;
  margin: 0 auto;
}

.translate-input {
  width: 100%;
  padding: var(--spacing-md);
  font-size: var(--font-size-md);
  background: var(--color-bg-secondary);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-primary);
  resize: vertical;
  font-family: inherit;
  transition: border-color var(--transition-fast);
  line-height: 1.6;
}

.translate-input:focus {
  outline: none;
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-alpha);
}

.translate-input::placeholder {
  color: var(--color-text-tertiary);
}

.input-actions {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  justify-content: center;
  flex-wrap: wrap;
}

.translate-btn {
  padding: var(--spacing-sm) var(--spacing-xl);
  font-size: var(--font-size-md);
  min-width: 120px;
}

.clipboard-btn {
  padding: var(--spacing-sm) var(--spacing-lg);
  font-size: var(--font-size-sm);
}

.screenshot-btn {
  padding: var(--spacing-sm) var(--spacing-lg);
  font-size: var(--font-size-sm);
}

.divider {
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.current-result {
  margin: var(--spacing-xl) 0;
}

.current-result h3 {
  margin-bottom: var(--spacing-md);
  color: var(--color-text-primary);
  font-size: var(--font-size-md);
}
</style>
