<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useTranslationStore } from '../stores/translation'
import type { Translation } from '../stores/translation'

const router = useRouter()
const route = useRoute()
const store = useTranslationStore()
const translation = ref<Translation | null>(null)

onMounted(async () => {
  const id = parseInt(route.params.id as string)
  translation.value = await store.getTranslationById(id)
})

function goBack() {
  router.back()
}

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString('zh-CN')
}

async function toggleFavorite() {
  if (translation.value && translation.value.id) {
    const newState = !translation.value.is_favorite
    await store.toggleFavorite(translation.value.id, newState)
    translation.value.is_favorite = newState ? 1 : 0
  }
}

async function copyTranslation() {
  if (translation.value) {
    try {
      await navigator.clipboard.writeText(translation.value.translated_text)
    } catch (e) {
      console.error('复制失败:', e)
    }
  }
}
</script>

<template>
  <div class="page-container">
    <div class="header">
      <button @click="goBack" class="btn-secondary back-btn">← 返回</button>
      <h1>翻译详情</h1>
    </div>

    <div v-if="!translation" class="loading">加载中...</div>

    <div v-else class="detail-card">
      <div class="detail-section">
        <h3>原文</h3>
        <p class="source-text">{{ translation.source_text }}</p>
      </div>

      <div class="detail-section">
        <h3>译文</h3>
        <p class="translated-text">{{ translation.translated_text }}</p>
      </div>

      <div v-if="translation.phonetic" class="detail-section">
        <h3>音标</h3>
        <p class="phonetic">[{{ translation.phonetic }}]</p>
      </div>

      <div v-if="translation.word_type" class="detail-section">
        <h3>词性/释义</h3>
        <p class="word-type">{{ translation.word_type }}</p>
      </div>

      <div v-if="translation.explains && translation.explains.length > 0" class="detail-section">
        <h3>详细释义</h3>
        <ul class="detail-list">
          <li v-for="(explain, index) in translation.explains" :key="`explain-${index}`">
            {{ explain }}
          </li>
        </ul>
      </div>

      <div v-if="translation.examples && translation.examples.length > 0" class="detail-section">
        <h3>例句</h3>
        <ul class="detail-list">
          <li v-for="(example, index) in translation.examples" :key="`example-${index}`">
            {{ example }}
          </li>
        </ul>
      </div>

      <div v-if="translation.synonyms && translation.synonyms.length > 0" class="detail-section">
        <h3>近义词</h3>
        <div class="chip-list">
          <span v-for="(synonym, index) in translation.synonyms" :key="`synonym-${index}`" class="chip">
            {{ synonym }}
          </span>
        </div>
      </div>

      <div class="detail-section">
        <h3>统计信息</h3>
        <div class="stats">
          <div class="stat-item">
            <span class="stat-label">创建时间：</span>
            <span>{{ formatTime(translation.created_at) }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">查询次数：</span>
            <span>{{ translation.access_count }} 次</span>
          </div>
        </div>
      </div>

      <div class="actions">
        <button @click="toggleFavorite" class="btn favorite-btn">
          {{ translation.is_favorite ? '★ 已收藏' : '☆ 收藏' }}
        </button>
        <button @click="copyTranslation" class="btn btn-secondary copy-btn">
          📋 复制译文
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-xl);
}

.header h1 {
  font-size: var(--font-size-lg);
  color: var(--color-text-primary);
}

.back-btn {
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-sm);
}

.loading {
  text-align: center;
  padding: var(--spacing-xl);
  color: var(--color-text-tertiary);
  font-size: var(--font-size-md);
}

.detail-card {
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-xl);
}

.detail-section {
  margin-bottom: var(--spacing-xl);
}

.detail-section:last-of-type {
  margin-bottom: 0;
}

.detail-section h3 {
  margin: 0 0 var(--spacing-sm) 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.source-text {
  font-size: 20px;
  font-weight: 500;
  color: var(--color-text-primary);
  line-height: 1.6;
  margin: 0;
}

.translated-text {
  font-size: 20px;
  color: var(--color-primary);
  line-height: 1.6;
  margin: 0;
}

.phonetic {
  font-size: var(--font-size-md);
  color: var(--color-text-secondary);
  font-style: italic;
  margin: 0;
}

.word-type {
  font-size: var(--font-size-md);
  color: var(--color-text-secondary);
  margin: 0;
}

.detail-list {
  margin: 0;
  padding-left: 20px;
  color: var(--color-text-secondary);
  line-height: 1.7;
}

.detail-list li + li {
  margin-top: var(--spacing-xs);
}

.chip-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-sm);
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 6px 10px;
  background: rgba(102, 126, 234, 0.12);
  border: 1px solid rgba(102, 126, 234, 0.2);
  border-radius: 999px;
  color: var(--color-primary);
  font-size: var(--font-size-sm);
}

.stats {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
}

.stat-item {
  font-size: var(--font-size-sm);
}

.stat-label {
  color: var(--color-text-tertiary);
  margin-right: var(--spacing-xs);
}

.actions {
  margin-top: var(--spacing-xl);
  padding-top: var(--spacing-lg);
  border-top: 1px solid var(--color-border);
  display: flex;
  gap: var(--spacing-md);
}

.favorite-btn {
  background: white;
  border: 2px solid var(--color-primary);
  color: var(--color-primary);
  font-weight: 500;
}

.favorite-btn:hover {
  background: var(--color-primary);
  color: white;
}

.copy-btn {
  padding: var(--spacing-sm) var(--spacing-md);
}
</style>
