<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useTranslationStore } from '../stores/translation'
import NavigationBar from '../components/NavigationBar.vue'
import EmptyState from '../components/EmptyState.vue'
import type { Translation } from '../stores/translation'

const router = useRouter()
const store = useTranslationStore()
const favorites = ref<Translation[]>([])

onMounted(async () => {
  await loadFavorites()
})

async function loadFavorites() {
  favorites.value = await store.loadFavorites()
}

function goToDetail(id: number | undefined) {
  if (id) {
    router.push(`/detail/${id}`)
  }
}

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString('zh-CN')
}

async function removeFavorite(id: number | undefined, event: Event) {
  event.stopPropagation()
  if (id) {
    await store.toggleFavorite(id, false)
    await loadFavorites()
  }
}
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>收藏列表</h1>
    </div>

    <EmptyState
      v-if="favorites.length === 0"
      message="暂无收藏"
      icon="⭐"
    />

    <div v-else class="favorites-grid">
      <div
        v-for="item in favorites"
        :key="item.id"
        class="favorite-card"
        @click="goToDetail(item.id)"
      >
        <div class="card-content">
          <div class="source">{{ item.source_text }}</div>
          <div class="translation">{{ item.translated_text }}</div>
          <div class="meta">
            <span v-if="item.phonetic" class="phonetic">[{{ item.phonetic }}]</span>
            <span class="time">{{ formatTime(item.created_at) }}</span>
          </div>
        </div>
        <button
          @click="removeFavorite(item.id, $event)"
          class="remove-btn"
          title="取消收藏"
        >
          ★
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page-header {
  margin-bottom: var(--spacing-lg);
}

.page-header h1 {
  font-size: var(--font-size-lg);
  color: var(--color-text-primary);
}

.favorites-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--spacing-lg);
}

.favorite-card {
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--spacing-lg);
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
}

.favorite-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

.card-content {
  margin-bottom: var(--spacing-sm);
}

.source {
  font-size: var(--font-size-md);
  font-weight: 500;
  margin-bottom: var(--spacing-sm);
  color: var(--color-text-primary);
}

.translation {
  font-size: var(--font-size-sm);
  color: var(--color-primary);
  margin-bottom: var(--spacing-sm);
}

.meta {
  font-size: 12px;
  color: var(--color-text-tertiary);
  display: flex;
  gap: var(--spacing-sm);
}

.phonetic {
  font-style: italic;
  color: var(--color-primary);
}

.remove-btn {
  position: absolute;
  top: var(--spacing-sm);
  right: var(--spacing-sm);
  background: none;
  border: none;
  font-size: 20px;
  color: var(--color-warning);
  cursor: pointer;
  padding: var(--spacing-xs);
  transition: color 0.2s;
}

.remove-btn:hover {
  color: #f57c00;
}
</style>
