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
      <div class="source-row">
        <div class="source-text">{{ translation.source_text }}</div>
        <span v-if="showFavorite && translation.is_favorite" class="favorite-badge" title="已收藏">★</span>
      </div>
      <div class="translated-text">{{ translation.translated_text }}</div>

      <div v-if="!compact" class="card-meta">
        <span v-if="translation.phonetic" class="phonetic">
          /{{ translation.phonetic }}/
        </span>
        <span v-if="translation.word_type" class="word-type">
          {{ translation.word_type }}
        </span>
      </div>

      <div class="card-footer">
        <span class="time">{{ formatTime(translation.created_at) }}</span>
        <span v-if="translation.access_count > 1" class="access-count">
          ×{{ translation.access_count }}
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
  padding: 14px 16px;
  cursor: pointer;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast), transform var(--transition-fast);
  margin-bottom: 12px;
}

.translation-card:hover {
  box-shadow: var(--shadow-sm);
  border-color: var(--color-app-accent-border);
  transform: translateY(-1px);
}

.translation-card.compact {
  padding: 10px 14px;
}

.card-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.source-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.source-text {
  font-family: var(--font-family-display);
  font-weight: 650;
  letter-spacing: -0.02em;
  color: var(--color-app-text-strong);
  font-size: 16px;
  line-height: 1.35;
}

.translated-text {
  color: var(--color-text-secondary);
  font-size: 14px;
  line-height: 1.55;
}

.card-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  font-size: 12px;
  color: var(--color-text-tertiary);
}

.phonetic {
  font-family: var(--font-family-mono);
  color: var(--color-primary);
}

.word-type {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  background: var(--color-chip-bg);
  border: 1px solid var(--color-chip-border);
  color: var(--color-app-accent-strong);
  font-weight: 600;
}

.card-footer {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--color-text-tertiary);
  align-items: center;
  padding-top: 2px;
}

.time {
  flex: 1;
}

.access-count {
  color: var(--color-text-secondary);
  font-family: var(--font-family-mono);
  font-weight: 600;
}

.favorite-badge {
  color: var(--color-warning);
  font-size: 14px;
  line-height: 1;
}
</style>
