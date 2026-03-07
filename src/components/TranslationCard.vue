<script setup lang="ts">
import { useRouter } from 'vue-router'
import type { Translation } from '../stores/translation'

interface Props {
  translation: Translation
  showFavorite?: boolean
  compact?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  showFavorite: false,
  compact: false
})

const router = useRouter()

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString('zh-CN')
}

function goToDetail() {
  if (props.translation.id) {
    router.push(`/detail/${props.translation.id}`)
  }
}
</script>

<template>
  <div
    :class="['translation-card', { compact }]"
    @click="goToDetail"
  >
    <div class="card-content">
      <div class="source-text">{{ translation.source_text }}</div>
      <div class="translated-text">{{ translation.translated_text }}</div>

      <div v-if="!compact" class="card-meta">
        <span v-if="translation.phonetic" class="phonetic">
          [{{ translation.phonetic }}]
        </span>
        <span v-if="translation.word_type" class="word-type">
          {{ translation.word_type }}
        </span>
      </div>

      <div class="card-footer">
        <span class="time">{{ formatTime(translation.created_at) }}</span>
        <span v-if="translation.access_count > 1" class="access-count">
          查询 {{ translation.access_count }} 次
        </span>
        <span v-if="showFavorite && translation.is_favorite" class="favorite-badge">
          ⭐
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.translation-card {
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  cursor: pointer;
  transition: all 0.2s;
  margin-bottom: var(--spacing-md);
}

.translation-card:hover {
  box-shadow: var(--shadow-sm);
  border-color: var(--color-primary);
}

.translation-card.compact {
  padding: var(--spacing-sm) var(--spacing-md);
}

.card-content {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
}

.source-text {
  font-weight: 500;
  color: var(--color-text-primary);
  font-size: var(--font-size-md);
}

.translated-text {
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
}

.card-meta {
  display: flex;
  gap: var(--spacing-md);
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.phonetic {
  color: var(--color-primary);
}

.word-type {
  color: var(--color-text-secondary);
}

.card-footer {
  display: flex;
  gap: var(--spacing-md);
  font-size: 12px;
  color: var(--color-text-tertiary);
  align-items: center;
}

.time {
  flex: 1;
}

.access-count {
  color: var(--color-warning);
  font-weight: 500;
}

.favorite-badge {
  font-size: 14px;
}
</style>
