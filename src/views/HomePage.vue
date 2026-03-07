<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useTranslationStore } from '../stores/translation'
import NavigationBar from '../components/NavigationBar.vue'
import TranslationCard from '../components/TranslationCard.vue'

const router = useRouter()
const store = useTranslationStore()

const recentItems = computed(() => store.history.slice(0, 5))

function goToHistory() {
  router.push('/history')
}
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="header">
      <h1>选词翻译工具</h1>
    </div>

    <!-- 翻译按钮 -->
    <div class="translate-section">
      <button
        @click="store.translateFromClipboard"
        :disabled="store.loading"
        class="btn btn-primary translate-btn"
      >
        {{ store.loading ? '翻译中...' : `翻译剪贴板内容 (${store.globalShortcut})` }}
      </button>
      <p class="hint">先复制要翻译的文本，然后点击按钮或按快捷键</p>
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
  text-align: center;
  margin: var(--spacing-xl) 0;
}

.translate-btn {
  width: 100%;
  max-width: 400px;
  padding: 16px 24px;
  font-size: var(--font-size-md);
}

.hint {
  margin-top: var(--spacing-sm);
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
  transition: color 0.2s;
}

.view-all-btn:hover {
  color: var(--color-primary-hover);
}
</style>

