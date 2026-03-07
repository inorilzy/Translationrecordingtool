<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useTranslationStore } from '../stores/translation'
import NavigationBar from '../components/NavigationBar.vue'
import TranslationCard from '../components/TranslationCard.vue'
import EmptyState from '../components/EmptyState.vue'

const store = useTranslationStore()
const searchQuery = ref('')

const filteredHistory = computed(() => {
  if (!searchQuery.value) return store.history
  const query = searchQuery.value.toLowerCase()
  return store.history.filter(item =>
    item.source_text.toLowerCase().includes(query) ||
    item.translated_text.toLowerCase().includes(query)
  )
})

onMounted(() => {
  store.loadHistory()
})
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>历史记录</h1>
    </div>

    <!-- 搜索框 -->
    <div class="search-box">
      <input
        v-model="searchQuery"
        class="input search-input"
        placeholder="搜索翻译记录..."
      />
    </div>

    <!-- 历史列表 -->
    <div v-if="filteredHistory.length > 0" class="history-list">
      <TranslationCard
        v-for="item in filteredHistory"
        :key="item.id"
        :translation="item"
        :show-favorite="true"
      />
    </div>

    <EmptyState
      v-else
      message="暂无历史记录"
      icon="📝"
    />
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

.search-box {
  margin-bottom: var(--spacing-lg);
}

.search-input {
  width: 100%;
}

.history-list {
  display: flex;
  flex-direction: column;
}
</style>
