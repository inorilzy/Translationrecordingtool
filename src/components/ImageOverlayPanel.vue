<script lang="ts">
export function overlayBlockStyle(block: {
  x: number
  y: number
  width: number
  height: number
}, imageWidth: number, imageHeight: number) {
  const width = Math.max(imageWidth, 1)
  const height = Math.max(imageHeight, 1)
  const fontSize = Math.round(Math.max(12, Math.min(28, block.height * 0.72)) * 10) / 10

  return {
    left: `${(block.x / width) * 100}%`,
    top: `${(block.y / height) * 100}%`,
    width: `${(block.width / width) * 100}%`,
    height: `${(block.height / height) * 100}%`,
    fontSize: `${fontSize}px`,
  }
}
</script>

<script setup lang="ts">
import { computed } from 'vue'
import type { ImageOverlayTranslation } from '../stores/translation'

const props = defineProps<{
  overlay: ImageOverlayTranslation
}>()

const emit = defineEmits<{
  close: []
}>()

const aspectRatio = computed(() => {
  const width = Math.max(props.overlay.imageWidth, 1)
  const height = Math.max(props.overlay.imageHeight, 1)
  return `${width} / ${height}`
})
</script>

<template>
  <section class="overlay-panel" aria-label="原图对照翻译">
    <div class="overlay-heading">
      <div>
        <h2>原图对照</h2>
        <p>译文已叠在原文字位置，可对照查看</p>
      </div>
      <button type="button" class="btn btn-secondary close-btn" @click="emit('close')">
        关闭
      </button>
    </div>

    <div class="overlay-stage" :style="{ aspectRatio }">
      <img
        class="overlay-image"
        :src="overlay.imageBase64"
        alt="OCR 原图"
        draggable="false"
      />
      <div
        v-for="(block, index) in overlay.blocks"
        :key="`${block.x}-${block.y}-${index}`"
        class="overlay-block"
        :style="overlayBlockStyle(block, overlay.imageWidth, overlay.imageHeight)"
        :title="block.sourceText"
      >
        <span>{{ block.translatedText || block.sourceText }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.overlay-panel {
  margin-top: var(--spacing-xl);
  padding: 16px;
  border: 1px solid var(--color-app-panel-border);
  border-radius: var(--radius-lg);
  background: var(--color-app-panel-bg);
  box-shadow: var(--shadow-app-panel);
}

.overlay-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.overlay-heading h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-text-tertiary);
}

.overlay-heading p {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.close-btn {
  padding: 8px 12px;
  font-size: var(--font-size-sm);
}

.overlay-stage {
  position: relative;
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: #0a0a0a;
}

.overlay-image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  user-select: none;
}

.overlay-block {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 2px 4px;
  box-sizing: border-box;
  overflow: hidden;
  border-radius: 4px;
  background: rgba(15, 23, 42, 0.78);
  color: #f8fafc;
  line-height: 1.15;
  font-weight: 600;
  letter-spacing: 0.01em;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.35);
  border: 1px solid rgba(45, 212, 191, 0.35);
  pointer-events: none;
}

.overlay-block span {
  display: -webkit-box;
  overflow: hidden;
  white-space: normal;
  word-break: break-word;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
</style>
