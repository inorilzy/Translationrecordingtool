<template>
  <div class="page-container">
    <NavigationBar />

    <div class="page-header">
      <h1>设置</h1>
    </div>

    <div class="settings-content">
      <section class="setting-section">
        <h2>翻译与 OCR 服务</h2>
        <p class="section-hint">单词查询优先使用本地词典；句子、截图 OCR 结果会使用这里选择的在线翻译服务。</p>
        <div class="form-group">
          <label>句子翻译服务</label>
          <select v-model="config.translationProvider" @change="saveApiConfig" class="input theme-select">
            <option value="youdao">有道翻译</option>
            <option value="microsoft">微软翻译</option>
          </select>
        </div>
        <div class="form-group">
          <label>有道 App Key</label>
          <input
            v-model="config.apiKey"
            @blur="saveApiConfig"
            type="text"
            placeholder="请输入有道翻译 App Key"
            class="input"
          />
        </div>
        <div class="form-group">
          <label>有道 App Secret</label>
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
        <div class="form-group separated-group">
          <label>Microsoft Translator Key</label>
          <input
            v-model="config.microsoftTranslatorKey"
            @blur="saveApiConfig"
            type="password"
            placeholder="请输入 Microsoft Translator Key"
            class="input"
          />
        </div>
        <div class="form-group">
          <label>Microsoft Translator Region</label>
          <input
            v-model="config.microsoftTranslatorRegion"
            @blur="saveApiConfig"
            type="text"
            placeholder="例如 eastasia；global 资源可留空"
            class="input"
          />
        </div>
        <small>
          <a href="https://portal.azure.com/#create/Microsoft.CognitiveServicesTextTranslation" target="_blank">
            创建微软翻译资源
          </a>
        </small>
        <div class="form-group separated-group">
          <label>Paddle OCR HTTP 地址</label>
          <input
            v-model="config.ocrEndpoint"
            @blur="saveApiConfig"
            type="text"
            placeholder="例如 http://127.0.0.1:8866/ocr"
            class="input"
          />
          <div class="inline-actions">
            <small>截图会以 PNG base64 JSON 发送到该地址，字段名为 image。</small>
            <button
              type="button"
              class="btn btn-secondary service-test-btn"
              :disabled="ocrCheckLoading || !config.ocrEndpoint.trim()"
              @click="checkOcrService"
            >
              {{ ocrCheckLoading ? '检查中...' : '测试 OCR 服务' }}
            </button>
          </div>
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
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart'
import { useSettingsStore } from '../stores/settings'
import NavigationBar from '../components/NavigationBar.vue'

const store = useSettingsStore()

const config = ref({
  apiKey: '',
  apiSecret: '',
  translationProvider: 'youdao',
  microsoftTranslatorKey: '',
  microsoftTranslatorRegion: '',
  ocrEndpoint: 'http://127.0.0.1:8866/ocr',
  globalShortcut: 'Ctrl+Q',
  enableTray: true,
  enableAutostart: false,
  theme: 'light'
})

const isCapturing = ref(false)
const autostartLoading = ref(true)
const ocrCheckLoading = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error'>('success')

onMounted(async () => {
  await store.loadSettings()
  config.value.apiKey = store.apiKey
  config.value.apiSecret = store.apiSecret
  config.value.translationProvider = store.translationProvider
  config.value.microsoftTranslatorKey = store.microsoftTranslatorKey
  config.value.microsoftTranslatorRegion = store.microsoftTranslatorRegion
  config.value.ocrEndpoint = store.ocrEndpoint
  config.value.globalShortcut = store.globalShortcut
  config.value.enableTray = store.enableTray
  config.value.theme = store.theme

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
  if (
    config.value.apiKey === store.apiKey
    && config.value.apiSecret === store.apiSecret
    && config.value.translationProvider === store.translationProvider
    && config.value.microsoftTranslatorKey === store.microsoftTranslatorKey
    && config.value.microsoftTranslatorRegion === store.microsoftTranslatorRegion
    && config.value.ocrEndpoint === store.ocrEndpoint
  ) {
    return // 没有变化，不保存
  }

  try {
    await store.updateApiConfig({
      apiKey: config.value.apiKey,
      apiSecret: config.value.apiSecret,
      translationProvider: config.value.translationProvider,
      microsoftTranslatorKey: config.value.microsoftTranslatorKey,
      microsoftTranslatorRegion: config.value.microsoftTranslatorRegion,
      ocrEndpoint: config.value.ocrEndpoint,
    })
    showMessage('服务配置已保存', 'success')
  } catch (e) {
    showMessage(`保存服务配置失败: ${e}`, 'error')
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
    await store.updateTrayBehavior(config.value.enableTray)
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

async function checkOcrService() {
  ocrCheckLoading.value = true
  try {
    const result = await store.checkOcrService(config.value.ocrEndpoint)
    showMessage(result, 'success')
  } catch (e) {
    showMessage(`OCR 服务检查失败: ${e}`, 'error')
  } finally {
    ocrCheckLoading.value = false
  }
}

async function changeTheme() {
  try {
    await store.updateTheme(config.value.theme)
  } catch (e) {
    showMessage(`切换主题失败: ${e}`, 'error')
    config.value.theme = store.theme
    return
  }

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
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  line-height: 1.6;
}

.form-group {
  margin-bottom: var(--spacing-md);
}

.form-group:last-child {
  margin-bottom: 0;
}

.separated-group {
  margin-top: var(--spacing-lg);
}

.form-group label {
  display: block;
  margin-bottom: var(--spacing-sm);
  font-weight: var(--font-weight-medium);
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
  font-size: var(--font-size-xs);
}

.inline-actions {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  justify-content: space-between;
  margin-top: var(--spacing-xs);
}

.inline-actions small {
  margin-top: 0;
}

.service-test-btn {
  flex: 0 0 auto;
  padding: var(--spacing-xs) var(--spacing-md);
  font-size: var(--font-size-xs);
}

.message {
  margin-top: var(--spacing-md);
  padding: var(--spacing-md);
  border-radius: var(--radius-sm);
  animation: fadeIn 0.3s ease-in;
}

.success-message {
  background: var(--color-success-bg);
  color: var(--color-success);
  border: var(--border-width) solid var(--color-border);
}

.error-message {
  background: var(--color-error-bg);
  color: var(--color-error);
  border: var(--border-width) solid var(--color-border);
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
  background-color: var(--toggle-bg-inactive);
  transition: 0.3s;
  border-radius: var(--radius-pill);
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: var(--toggle-thumb);
  transition: 0.3s;
  border-radius: var(--radius-full);
  box-shadow: 0 1px 3px var(--toggle-thumb-shadow);
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
  background: linear-gradient(to right, var(--color-border) 0%, var(--color-primary) 100%);
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
  box-shadow: var(--shadow-sm);
}

.slider::-moz-range-thumb {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--color-primary);
  cursor: pointer;
  border: none;
  box-shadow: var(--shadow-sm);
}

.slider-value {
  min-width: 80px;
  text-align: right;
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  font-size: var(--font-size-sm);
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  margin-top: var(--spacing-xs);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.theme-select {
  width: 100%;
  cursor: pointer;
}
</style>
