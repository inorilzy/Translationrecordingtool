<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import NavigationBar from '../components/NavigationBar.vue'

const logFiles = ref<string[]>([])
const selectedFile = ref<string>('')
const logContent = ref<string>('')
const loading = ref(false)
const error = ref<string>('')
const logDirPath = ref<string>('')

async function loadLogFiles() {
  try {
    loading.value = true
    error.value = ''
    logFiles.value = await invoke<string[]>('get_log_files')

    if (logFiles.value.length > 0) {
      selectedFile.value = logFiles.value[0]
      await loadLogContent(selectedFile.value)
    }

    logDirPath.value = await invoke<string>('get_log_dir_path')
  } catch (e) {
    error.value = `加载日志文件列表失败: ${e}`
  } finally {
    loading.value = false
  }
}

async function loadLogContent(filename: string) {
  try {
    loading.value = true
    error.value = ''
    logContent.value = await invoke<string>('read_log_file', { filename })
  } catch (e) {
    error.value = `读取日志文件失败: ${e}`
    logContent.value = ''
  } finally {
    loading.value = false
  }
}

async function onFileChange() {
  if (selectedFile.value) {
    await loadLogContent(selectedFile.value)
  }
}

async function exportLogs() {
  if (!logContent.value) return

  const blob = new Blob([logContent.value], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = selectedFile.value || 'app.log'
  a.click()
  URL.revokeObjectURL(url)
}

async function openLogDir() {
  if (logDirPath.value) {
    await invoke('plugin:opener|open_path', { path: logDirPath.value })
  }
}

onMounted(() => {
  loadLogFiles()
})
</script>

<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>日志</h1>
      <div class="header-actions">
        <button v-if="logFiles.length > 0" @click="openLogDir" class="btn-secondary">
          打开日志目录
        </button>
        <button v-if="logContent" @click="exportLogs" class="btn-primary">
          导出当前日志
        </button>
      </div>
    </div>

    <div v-if="error" class="error-message">
      {{ error }}
    </div>

    <div v-if="logFiles.length > 0" class="log-selector">
      <label>选择日志文件：</label>
      <select v-model="selectedFile" @change="onFileChange" class="file-select">
        <option v-for="file in logFiles" :key="file" :value="file">
          {{ file }}
        </option>
      </select>
    </div>

    <div class="logs-container">
      <div v-if="loading" class="loading-state">
        加载中...
      </div>
      <div v-else-if="logFiles.length === 0" class="empty-state">
        暂无日志记录
      </div>
      <div v-else-if="logContent" class="log-content">
        <pre>{{ logContent }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page-container {
  padding: var(--spacing-lg);
  max-width: 1200px;
  margin: 0 auto;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-lg);
}

.page-header h1 {
  font-size: var(--font-size-xl);
  color: var(--color-text-primary);
  margin: 0;
}

.header-actions {
  display: flex;
  gap: var(--spacing-sm);
}

.btn-primary,
.btn-secondary {
  padding: var(--spacing-sm) var(--spacing-md);
  border-radius: var(--border-radius-sm);
  border: none;
  cursor: pointer;
  font-size: var(--font-size-sm);
  transition: all 0.2s;
}

.btn-primary {
  background: var(--color-primary);
  color: white;
}

.btn-primary:hover {
  opacity: 0.9;
}

.btn-secondary {
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
}

.btn-secondary:hover {
  background: var(--color-bg-tertiary);
}

.error-message {
  background: #fee;
  color: #c33;
  padding: var(--spacing-md);
  border-radius: var(--border-radius-sm);
  margin-bottom: var(--spacing-md);
}

.log-selector {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.log-selector label {
  color: var(--color-text-primary);
  font-size: var(--font-size-md);
}

.file-select {
  padding: var(--spacing-sm) var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius-sm);
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  font-size: var(--font-size-sm);
  cursor: pointer;
}

.logs-container {
  background: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
  padding: var(--spacing-md);
  max-height: 600px;
  overflow-y: auto;
}

.loading-state,
.empty-state {
  text-align: center;
  color: var(--color-text-secondary);
  padding: var(--spacing-xl);
}

.log-content {
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 12px;
}

.log-content pre {
  margin: 0;
  white-space: pre-wrap;
  word-wrap: break-word;
  color: var(--color-text-primary);
  line-height: 1.5;
}
</style>
