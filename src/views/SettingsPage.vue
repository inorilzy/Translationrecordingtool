<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>设置</h1>
    </div>

    <div class="settings-content">
      <section class="setting-section">
        <h2>有道翻译 API</h2>
        <p class="section-hint">配置后可翻译句子。单词查询无需配置，使用免费的 Free Dictionary API。</p>
        <div class="form-group">
          <label>App Key</label>
          <input
            v-model="config.apiKey"
            @blur="saveApiConfig"
            type="text"
            placeholder="请输入有道翻译 App Key"
            class="input"
          />
        </div>
        <div class="form-group">
          <label>App Secret</label>
          <input
            v-model="config.apiSecret"
            @blur="saveApiConfig"
            type="password"
            placeholder="请输入有道翻译 App Secret"
            class="input"
          />
        </div>
        <small>
          <a href="https://ai.youdao.com/console/#/" target="_blank">
            点击这里获取有道翻译 API 密钥
          </a>
        </small>
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
        <h2>外观</h2>
        <div class="form-group">
          <label>主题</label>
          <select v-model="config.theme" @change="changeTheme" class="input theme-select">
            <option value="light">Light - 浅色</option>
            <option value="dark">Dark - 深色 (VSCode)</option>
            <option value="one-dark">One Dark Pro</option>
            <option value="github-light">GitHub Light</option>
            <option value="github-dark">GitHub Dark</option>
          </select>
          <small>选择你喜欢的主题风格</small>
        </div>
      </section>

      <section class="setting-section">
        <h2>窗口行为</h2>
        <div class="form-group">
          <div class="toggle-group">
            <label>关闭主窗口时最小化到托盘</label>
            <label class="toggle-switch">
              <input type="checkbox" v-model="config.enableTray" @change="saveTrayBehavior" />
              <span class="toggle-slider"></span>
            </label>
          </div>
          <small>开启后点击右上角关闭只会隐藏到托盘；关闭后会直接退出应用。</small>
        </div>
        <div class="form-group">
          <div class="toggle-group">
            <label>开机启动</label>
            <label class="toggle-switch">
              <input type="checkbox" v-model="config.enableAutostart" :disabled="autostartLoading" @change="saveAutostartBehavior" />
              <span class="toggle-slider"></span>
            </label>
          </div>
          <small>开启后应用会在系统登录后自动启动。</small>
        </div>
      </section>

      <div v-if="message" :class="['message', messageType === 'success' ? 'success-message' : 'error-message']">
        {{ message }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart'
import { useTranslationStore } from '../stores/translation'
import NavigationBar from '../components/NavigationBar.vue'

const store = useTranslationStore()

const config = ref({
  apiKey: '',
  apiSecret: '',
  globalShortcut: 'Ctrl+Q',
  enableTray: true,
  enableAutostart: false,
  theme: 'light'
})

const isCapturing = ref(false)
const autostartLoading = ref(true)
const message = ref('')
const messageType = ref<'success' | 'error'>('success')

onMounted(async () => {
  config.value.apiKey = store.apiKey
  config.value.apiSecret = store.apiSecret
  config.value.globalShortcut = store.globalShortcut
  config.value.enableTray = localStorage.getItem('enable_tray') !== 'false'
  config.value.theme = localStorage.getItem('theme') || 'light'

  try {
    config.value.enableAutostart = await isAutostartEnabled()
  } catch (e) {
    console.warn('读取开机启动状态失败（开发模式下正常）:', e)
    config.value.enableAutostart = false
  } finally {
    autostartLoading.value = false
  }
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
    const newShortcut = keys.join('+')
    config.value.globalShortcut = newShortcut
    saveGlobalShortcut()
  }
}

async function saveApiConfig() {
  if (config.value.apiKey === store.apiKey && config.value.apiSecret === store.apiSecret) {
    return // 没有变化，不保存
  }

  try {
    await store.updateApiConfig(config.value.apiKey, config.value.apiSecret)
    showMessage('API 配置已保存', 'success')
  } catch (e) {
    showMessage(`保存 API 配置失败: ${e}`, 'error')
  }
}

async function saveGlobalShortcut() {
  if (config.value.globalShortcut === store.globalShortcut) {
    return
  }

  try {
    await store.updateGlobalShortcut(config.value.globalShortcut)
    showMessage('快捷键已更新', 'success')
  } catch (e) {
    showMessage(`更新快捷键失败: ${e}`, 'error')
    // 恢复旧值
    config.value.globalShortcut = store.globalShortcut
  }
}

async function saveTrayBehavior() {
  try {
    localStorage.setItem('enable_tray', config.value.enableTray.toString())
    await invoke('update_tray_behavior', {
      enabled: config.value.enableTray
    })
    showMessage('托盘行为已更新', 'success')
  } catch (e) {
    showMessage(`更新托盘行为失败: ${e}`, 'error')
  }
}

async function saveAutostartBehavior() {
  try {
    if (config.value.enableAutostart) {
      await enableAutostart()
    } else {
      await disableAutostart()
    }
    showMessage('开机启动已更新', 'success')
  } catch (e) {
    console.warn('开机启动设置失败（开发模式下正常）:', e)
    // 开发模式下静默失败
  }
}

async function changeTheme() {
  localStorage.setItem('theme', config.value.theme)
  document.documentElement.setAttribute('data-theme', config.value.theme)

  // 通知弹窗窗口更新主题
  try {
    const webviewWindows = await getAllWebviewWindows()
    webviewWindows.forEach((webviewWindow) => {
      if (webviewWindow.label === 'popup') {
        webviewWindow.emit('theme-changed', { theme: config.value.theme })
      }
    })
  } catch (e) {
    console.warn('通知弹窗更新主题失败:', e)
  }

  const themeNames: Record<string, string> = {
    'light': 'Light 浅色',
    'dark': 'Dark 深色',
    'one-dark': 'One Dark Pro',
    'github-light': 'GitHub Light',
    'github-dark': 'GitHub Dark'
  }

  showMessage(`已切换到 ${themeNames[config.value.theme]} 主题`, 'success')
}

function showMessage(msg: string, type: 'success' | 'error') {
  message.value = msg
  messageType.value = type
  setTimeout(() => {
    message.value = ''
  }, 3000)
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

.message {
  margin-top: var(--spacing-md);
  padding: var(--spacing-md);
  border-radius: var(--border-radius-sm);
  animation: fadeIn 0.3s ease-in;
}

.success-message {
  background: #e8f5e9;
  color: #2e7d32;
  border: 1px solid #a5d6a7;
}

.error-message {
  background: #ffebee;
  color: #c62828;
  border: 1px solid #ef9a9a;
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

.theme-select {
  width: 100%;
  cursor: pointer;
}
</style>
