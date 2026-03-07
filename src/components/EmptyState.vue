<script setup lang="ts">
interface Props {
  message: string
  icon?: string
}

withDefaults(defineProps<Props>(), {
  icon: '📭'
})
</script>

<template>
  <div class="empty-state">
    <div class="empty-state-icon">{{ icon }}</div>
    <p class="empty-state-message">{{ message }}</p>
  </div>
</template>

<style scoped>
.empty-state {
  text-align: center;
  padding: var(--spacing-xl) var(--spacing-md);
  color: var(--color-text-tertiary);
}

.empty-state-icon {
  font-size: 48px;
  margin-bottom: var(--spacing-md);
}

.empty-state-message {
  font-size: var(--font-size-md);
}
</style>
