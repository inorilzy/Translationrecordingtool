<script lang="ts">
export async function submitTranslation(
  inputText: string,
  translateText: (text: string) => Promise<unknown>,
) {
  const text = inputText.trim()
  if (!text) return
  await translateText(text)
}

export function findPasteImageFile(clipboardData: DataTransfer | null): File | null {
  const items = clipboardData?.items
  if (!items) {
    return null
  }

  let hasText = false
  let imageFile: File | null = null

  for (const item of Array.from(items)) {
    if (item.kind === 'string' && (item.type === 'text/plain' || item.type === 'text/html')) {
      hasText = true
    }
    if (item.kind === 'file' && item.type.startsWith('image/')) {
      imageFile = item.getAsFile() ?? imageFile
    }
  }

  // Prefer normal text paste when the clipboard also carries text.
  if (hasText || !imageFile) {
    return null
  }

  return imageFile
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        resolve(reader.result)
        return
      }
      reject(new Error('无法读取粘贴的图片'))
    }
    reader.onerror = () => reject(new Error('无法读取粘贴的图片'))
    reader.readAsDataURL(file)
  })
}
</script>

<script setup lang="ts">
import { computed } from 'vue'
import { useTranslationStore } from '../stores/translation'
import { useSettingsStore } from '../stores/settings'
import TranslationCard from '../components/TranslationCard.vue'
import ImageOverlayPanel from '../components/ImageOverlayPanel.vue'

const store = useTranslationStore()
const settings = useSettingsStore()
const inputText = computed({
  get: () => store.manualInputText,
  set: (value: string) => store.setManualInputText(value),
})

async function handleTranslate() {
  await submitTranslation(inputText.value, store.translateText)
}

async function handleScreenshotTranslate() {
  const result = await store.translateScreenshot()
  if (result?.source_text) {
    inputText.value = result.source_text
  }
}

async function handlePaste(event: ClipboardEvent) {
  const imageFile = findPasteImageFile(event.clipboardData)
  if (!imageFile) {
    return
  }

  event.preventDefault()

  try {
    const imageBase64 = await readFileAsDataUrl(imageFile)
    const result = await store.translateImage(imageBase64)
    if (result?.source_text) {
      inputText.value = result.source_text
    }
  } catch (error) {
    store.error = error instanceof Error ? error.message : String(error)
  }
}
</script>

<template>
  <div class="page-container translate-page">
    <header class="page-header">
      <div class="eyebrow">WORKBENCH</div>
      <h1>手动翻译</h1>
      <p class="subtitle">输入文本，或粘贴图片 / 截图后可在原图位置查看译文</p>
    </header>

    <section class="workbench" aria-label="翻译工作台">
      <label class="field-label" for="translate-input">原文</label>
      <textarea
        id="translate-input"
        v-model="inputText"
        @keydown.ctrl.enter="handleTranslate"
        @keydown.meta.enter="handleTranslate"
        @paste="handlePaste"
        placeholder="输入文本，或直接粘贴图片识别后翻译；Ctrl/Cmd+Enter 提交"
        class="translate-input"
        rows="7"
        autofocus
      />

      <div class="toolbar">
        <button
          @click="handleTranslate"
          :disabled="store.loading || !inputText.trim()"
          class="btn btn-primary translate-btn"
        >
          {{ store.loading ? '翻译中...' : '翻译' }}
        </button>
        <button
          @click="store.translateFromClipboard"
          :disabled="store.loading"
          class="btn btn-secondary clipboard-btn"
        >
          {{ store.loading ? '翻译中...' : `剪贴板 (${settings.globalShortcut})` }}
        </button>
        <button
          @click="handleScreenshotTranslate"
          :disabled="store.loading"
          class="btn btn-secondary screenshot-btn"
        >
          {{ store.loading ? '处理中...' : `截图 OCR (${settings.screenshotShortcut})` }}
        </button>
      </div>
    </section>

    <div v-if="store.error" class="error-message">{{ store.error }}</div>

    <ImageOverlayPanel
      v-if="store.imageOverlay"
      :overlay="store.imageOverlay"
      @close="store.clearImageOverlay()"
    />

    <section v-if="store.currentTranslation" class="result-panel" aria-label="翻译结果">
      <div class="result-heading">
        <h2>结果</h2>
        <span class="result-hint">点击卡片查看详情</span>
      </div>
      <TranslationCard
        :translation="store.currentTranslation"
        :show-favorite="true"
      />
    </section>
  </div>
</template>

<style scoped>

.translate-page {
  max-width: 760px;
}

.page-header {
  margin-bottom: var(--spacing-lg);
}

.eyebrow {
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.14em;
  color: var(--color-app-accent);
  margin-bottom: 8px;
}

.page-header h1 {
  margin: 0;
  font-family: var(--font-family-display);
  font-size: 28px;
  font-weight: 650;
  letter-spacing: -0.03em;
  color: var(--color-app-text-strong);
  line-height: 1.15;
}

.subtitle {
  margin: 8px 0 0;
  max-width: 48ch;
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
  line-height: 1.55;
}

.workbench {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 18px;
  border: 1px solid var(--color-app-panel-border);
  border-radius: var(--radius-lg);
  background: var(--color-app-panel-bg);
  box-shadow: var(--shadow-app-panel);
}

.field-label {
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--color-text-tertiary);
}

.translate-input {
  width: 100%;
  min-height: 160px;
  padding: 14px 16px;
  font-size: 15px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-primary);
  resize: vertical;
  font-family: inherit;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  line-height: 1.65;
}

.translate-input:focus {
  outline: none;
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px var(--color-primary-alpha);
}

.translate-input::placeholder {
  color: var(--color-text-tertiary);
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.translate-btn {
  min-width: 108px;
  padding: 10px 18px;
}

.clipboard-btn,
.screenshot-btn {
  padding: 10px 14px;
  font-size: var(--font-size-sm);
}

.result-panel {
  margin-top: var(--spacing-xl);
}

.result-heading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.result-heading h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-text-tertiary);
}

.result-hint {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
</style>
