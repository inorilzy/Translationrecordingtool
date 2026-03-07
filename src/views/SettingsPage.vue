<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>设置</h1>
    </div>

    <div class="settings-content">
      <section class="setting-section">
        <h2>有道智云 API 配置</h2>
        <p class="section-hint">可选。留空时可使用内置词典查询单个英文单词；句子翻译和未命中的词仍需要在线 API。</p>
        <div class="form-group">
          <label>App Key</label>
          <input v-model="config.apiKey" type="text" placeholder="请输入 App Key" class="input" />
        </div>
        <div class="form-group">
          <label>App Secret</label>
          <input v-model="config.apiSecret" type="password" placeholder="请输入 App Secret" class="input" />
        </div>
      </section>

      <section class="setting-section">
        <h2>快捷键设置</h2>
        <div class="form-group">
          <label>全局翻译快捷键</label>
          <input
            ref="shortcutInput"
            :value="config.globalShortcut"
            @focus="isCapturing = true"
            @blur="isCapturing = false"
            @keydown.prevent="captureShortcut"
            placeholder="点击后按下快捷键组合"
            readonly
            class="input"
          />
          <small>当前快捷键：{{ config.globalShortcut }}</small>
        </div>
      </section>

      <section class="setting-section">
        <h2>窗口行为</h2>
        <div class="form-group">
          <div class="toggle-group">
            <label>关闭主窗口时最小化到托盘</label>
            <label class="toggle-switch">
              <input type="checkbox" v-model="config.enableTray" />
              <span class="toggle-slider"></span>
            </label>
          </div>
          <small>开启后点击右上角关闭只会隐藏到托盘；关闭后会直接退出应用。</small>
        </div>
      </section>

      <div class="actions">
        <button @click="saveSettings" class="btn btn-primary">保存设置</button>
        <button @click="resetDefaults" class="btn btn-secondary">恢复默认</button>
      </div>

      <div v-if="message" :class="['message', messageType === 'success' ? 'success-message' : 'error-message']">
        {{ message }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useTranslationStore } from '../stores/translation'
import NavigationBar from '../components/NavigationBar.vue'

const store = useTranslationStore()

const config = ref({
  apiKey: '',
  apiSecret: '',
  globalShortcut: 'Ctrl+Q',
  enableTray: true
})

const isCapturing = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error'>('success')

onMounted(() => {
  config.value.apiKey = store.apiKey
  config.value.apiSecret = store.apiSecret
  config.value.globalShortcut = store.globalShortcut
  config.value.enableTray = localStorage.getItem('enable_tray') !== 'false'
})

function captureShortcut(event: KeyboardEvent) {
  const keys: string[] = []

  if (event.ctrlKey) keys.push('Ctrl')
  if (event.altKey) keys.push('Alt')
  if (event.shiftKey) keys.push('Shift')
  if (event.metaKey) keys.push('Meta')

  const mainKey = event.key
  if (!['Control', 'Alt', 'Shift', 'Meta'].includes(mainKey)) {
    keys.push(mainKey.toUpperCase())
  }

  if (keys.length >= 2) {
    config.value.globalShortcut = keys.join('+')
  }
}

async function saveSettings() {
  try {
    // 更新 API 配置
    await store.updateApiConfig(config.value.apiKey, config.value.apiSecret)

    // 更新快捷键
    const oldShortcut = store.globalShortcut
    if (config.value.globalShortcut !== oldShortcut) {
      await store.updateGlobalShortcut(config.value.globalShortcut)
    }

    localStorage.setItem('enable_tray', config.value.enableTray.toString())
    await invoke('update_tray_behavior', {
      enabled: config.value.enableTray
    })

    message.value = '设置保存成功'
    messageType.value = 'success'

    // 3秒后自动消失
    setTimeout(() => {
      message.value = ''
    }, 3000)
  } catch (e) {
    message.value = `保存失败: ${e}`
    messageType.value = 'error'

    // 5秒后自动消失
    setTimeout(() => {
      message.value = ''
    }, 5000)
  }
}

async function resetDefaults() {
  config.value.apiKey = ''
  config.value.apiSecret = ''
  config.value.globalShortcut = 'Ctrl+Q'
  config.value.enableTray = true

  // 持久化到 localStorage
  localStorage.setItem('youdao_app_key', '')
  localStorage.setItem('youdao_app_secret', '')
  localStorage.setItem('global_shortcut', 'Ctrl+Q')
  localStorage.setItem('enable_tray', 'true')

  // 同步到 Rust
  try {
    await store.updateApiConfig('', '')
    await store.updateGlobalShortcut('Ctrl+Q')
    await invoke('update_tray_behavior', {
      enabled: true
    })
    message.value = '已恢复默认设置'
    messageType.value = 'success'
    setTimeout(() => {
      message.value = ''
    }, 3000)
  } catch (e) {
    message.value = `恢复默认设置失败: ${e}`
    messageType.value = 'error'
    setTimeout(() => {
      message.value = ''
    }, 5000)
  }
}
</script>

<style scoped>
.page-header {
  margin-bottom: var(--spacing-xl);
}

.page-header h1 {
  font-size: var(--font-size-lg);
  color: var(--color-text-primary);
}

.settings-content {
  max-width: 600px;
}

.setting-section {
  margin-bottom: var(--spacing-xl);
  padding: var(--spacing-lg);
  background: var(--color-bg-secondary);
  border-radius: var(--radius-md);
}

.setting-section h2 {
  margin: 0 0 var(--spacing-md) 0;
  font-size: var(--font-size-md);
  color: var(--color-text-primary);
}

.section-hint {
  margin: 0 0 var(--spacing-md) 0;
  font-size: 12px;
  color: var(--color-text-tertiary);
  line-height: 1.6;
}

.form-group {
  margin-bottom: var(--spacing-md);
}

.form-group:last-child {
  margin-bottom: 0;
}

.form-group label {
  display: block;
  margin-bottom: var(--spacing-sm);
  font-weight: 500;
  color: var(--color-text-primary);
  font-size: var(--font-size-sm);
}

.form-group input {
  width: 100%;
  box-sizing: border-box;
}

.form-group small {
  display: block;
  margin-top: var(--spacing-xs);
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.actions {
  display: flex;
  gap: var(--spacing-md);
  margin-top: var(--spacing-lg);
}

.message {
  margin-top: var(--spacing-md);
  animation: fadeIn 0.3s ease-in;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 开关按钮样式 */
.toggle-group {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 24px;
  margin: 0;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: #ccc;
  transition: 0.3s;
  border-radius: 24px;
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: 0.3s;
  border-radius: 50%;
}

.toggle-switch input:checked + .toggle-slider {
  background-color: var(--color-primary);
}

.toggle-switch input:checked + .toggle-slider:before {
  transform: translateX(24px);
}

/* 下拉选择框样式 */
.select-input {
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%23333' d='M6 9L1 4h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;
  padding-right: 36px;
  cursor: pointer;
}

/* 滑块样式 */
.slider-group {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-top: var(--spacing-sm);
}

.slider {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: linear-gradient(to right, #e0e0e0 0%, var(--color-primary) 100%);
  outline: none;
  -webkit-appearance: none;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--color-primary);
  cursor: pointer;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}

.slider::-moz-range-thumb {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--color-primary);
  cursor: pointer;
  border: none;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}

.slider-value {
  min-width: 80px;
  text-align: right;
  font-weight: 500;
  color: var(--color-text-primary);
  font-size: var(--font-size-sm);
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  margin-top: var(--spacing-xs);
  font-size: 12px;
  color: var(--color-text-tertiary);
}
</style>
