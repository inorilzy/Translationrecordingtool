<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useTranslationStore } from '../stores/translation'
import { useSettingsStore } from '../stores/settings'
import NavigationBar from '../components/NavigationBar.vue'
import TranslationCard from '../components/TranslationCard.vue'

const router = useRouter()
const store = useTranslationStore()
const settings = useSettingsStore()
const inputText = ref('')

const recentItems = computed(() => store.history.slice(0, 5))

function goToHistory() {
  router.push('/history')
}

async function handleTranslate() {
  const text = inputText.value.trim()
  if (!text) return
  await store.translateText(text)
}
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="header">
      <h1>选词翻译工具</h1>
    </div>

    <!-- 翻译输入区 -->
    <div class="translate-section">
      <div class="input-group">
        <textarea
          v-model="inputText"
          @keydown.ctrl.enter="handleTranslate"
          placeholder="输入要翻译的文本，支持 Ctrl+Enter 快速翻译"
          class="translate-input"
          rows="3"
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
        </div>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="store.error" class="error-message">{{ store.error }}</div>

    <!-- 当前翻译结果 -->
    <div v-if="store.currentTranslation" class="current-result">
      <h3>翻译结果</h3>
      <TranslationCard :translation="store.currentTranslation" />
    </div>

    <!-- 最近 5 条翻译 -->
    <div v-if="recentItems.length > 0" class="recent-translations">
      <div class="section-header">
        <h3>最近翻译</h3>
        <button @click="goToHistory" class="btn-secondary view-all-btn">
          查看全部 →
        </button>
      </div>
      <TranslationCard
        v-for="item in recentItems"
        :key="item.id"
        :translation="item"
        compact
      />
    </div>
  </div>
</template>

<style scoped>
.header {
  text-align: center;
  margin-bottom: var(--spacing-xl);
}

.header h1 {
  font-size: var(--font-size-lg);
  color: var(--color-text-primary);
}

.translate-section {
  margin: var(--spacing-xl) 0;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-md);
  max-width: 600px;
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
}

.translate-input:focus {
  outline: none;
  border-color: var(--color-primary);
}

.translate-input::placeholder {
  color: var(--color-text-tertiary);
}

.input-actions {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  justify-content: center;
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

.divider {
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.current-result {
  margin: var(--spacing-lg) 0;
}

.current-result h3 {
  margin-bottom: var(--spacing-md);
  color: var(--color-text-primary);
}

.recent-translations {
  margin-top: var(--spacing-xl);
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-md);
}

.section-header h3 {
  color: var(--color-text-primary);
  font-size: var(--font-size-md);
}

.view-all-btn {
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-sm);
  background: none;
  border: none;
  color: var(--color-primary);
  cursor: pointer;
  transition: color var(--transition-fast);
}

.view-all-btn:hover {
  color: var(--color-primary-hover);
}
</style>

