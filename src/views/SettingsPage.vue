<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart'
import { useMessage } from 'naive-ui'
import type { SelectOption } from 'naive-ui'
import {
  CheckCircle2,
  CloudCog,
  ExternalLink,
  Eye,
  EyeOff,
  Keyboard,
  MonitorCog,
  Palette,
  Play,
  RefreshCw,
  Settings2,
  ShieldQuestion,
} from '@lucide/vue'
import { useSettingsStore, type OcrServiceStatus } from '../stores/settings'

const FIXED_OCR_ENGINE = 'native_onnx'
const FIXED_OCR_MODEL_PROFILE = 'small'

const store = useSettingsStore()
const notify = useMessage()

const config = ref({
  apiKey: '',
  apiSecret: '',
  translationProvider: 'youdao',
  microsoftTranslatorKey: '',
  microsoftTranslatorRegion: '',
  ocrEndpoint: 'http://127.0.0.1:8866/ocr',
  ocrEngine: 'native_onnx',
  ocrModelProfile: 'small',
  ocrPreloadOnStartup: true,
  globalShortcut: 'Ctrl+Q',
  screenshotShortcut: 'Ctrl+Shift+Q',
  enableTray: true,
  enableAutostart: false,
  theme: 'light'
})

const capturingShortcut = ref<'global' | 'screenshot' | null>(null)
const autostartLoading = ref(true)
const ocrStatusLoading = ref(false)
const ocrWarmupLoading = ref(false)
const ocrStatus = ref<OcrServiceStatus | null>(null)
const showYoudaoSecret = ref(false)
const showMicrosoftKey = ref(false)

const providerOptions: SelectOption[] = [
  { label: '微软翻译', value: 'microsoft' },
  { label: '有道翻译', value: 'youdao' },
]

const themeOptions: SelectOption[] = [
  { label: 'Light - 浅色', value: 'light' },
  { label: 'Dark - 深色 (VSCode)', value: 'dark' },
  { label: 'One Dark Pro', value: 'one-dark' },
  { label: 'GitHub Light', value: 'github-light' },
  { label: 'GitHub Dark', value: 'github-dark' },
]

const microsoftRegionOptions: SelectOption[] = [
  { label: 'eastasia', value: 'eastasia' },
  { label: 'global / 留空', value: '' },
  { label: 'eastus', value: 'eastus' },
  { label: 'westeurope', value: 'westeurope' },
  { label: 'southeastasia', value: 'southeastasia' },
]

const providerName = computed(() => (
  config.value.translationProvider === 'microsoft' ? 'Microsoft Translator' : '有道翻译'
))

const providerConfigured = computed(() => {
  if (config.value.translationProvider === 'microsoft') {
    return config.value.microsoftTranslatorKey.trim().length > 0
  }
  return config.value.apiKey.trim().length > 0 && config.value.apiSecret.trim().length > 0
})

const providerStatus = computed(() => (
  providerConfigured.value ? '已配置' : '缺少密钥'
))

const providerStatusType = computed(() => (
  providerConfigured.value ? 'success' : 'warning'
))

const ocrStatusType = computed(() => {
  if (ocrStatusLoading.value) return 'info'
  return ocrStatus.value?.running ? 'success' : 'warning'
})

const ocrStatusText = computed(() => {
  if (ocrStatusLoading.value) return '检查中'
  if (!ocrStatus.value) return '未检查'
  return ocrStatus.value.running ? '运行中' : '未运行'
})

const ocrVersionText = computed(() => {
  const onnxVersion = ocrStatus.value?.onnxruntimeVersion || '1.20.1'
  const ppocrVersion = ocrStatus.value?.ppocrVersion || 'PP-OCRv6'
  const device = ocrStatus.value?.device?.toUpperCase() || 'CPU'
  return `ONNX Runtime ${onnxVersion} / ${ppocrVersion} Small / ${device}`
})

const ocrModelText = computed(() => {
  return ocrStatus.value?.modelDir
    ? '使用内置 PP-OCRv6 Small ONNX 模型，无需 Python OCR 服务'
    : '未检测到内置 PP-OCRv6 Small 模型目录'
})

const ocrStatusDetail = computed(() => {
  if (!ocrStatus.value) return ''
  return ocrStatus.value.lastError || ocrStatus.value.message
})

onMounted(async () => {
  await store.loadSettings()
  config.value.apiKey = store.apiKey
  config.value.apiSecret = store.apiSecret
  config.value.translationProvider = store.translationProvider
  config.value.microsoftTranslatorKey = store.microsoftTranslatorKey
  config.value.microsoftTranslatorRegion = store.microsoftTranslatorRegion
  config.value.ocrEndpoint = store.ocrEndpoint
  config.value.ocrEngine = FIXED_OCR_ENGINE
  config.value.ocrModelProfile = FIXED_OCR_MODEL_PROFILE
  config.value.ocrPreloadOnStartup = store.ocrPreloadOnStartup
  config.value.globalShortcut = store.globalShortcut
  config.value.screenshotShortcut = store.screenshotShortcut
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

  await refreshOcrStatus()
})

function captureShortcut(event: KeyboardEvent, target: 'global' | 'screenshot') {
  const keys: string[] = []

  if (event.ctrlKey) keys.push('Ctrl')
  if (event.altKey) keys.push('Alt')
  if (event.shiftKey) keys.push('Shift')
  if (event.metaKey) keys.push('Meta')

  const mainKey = event.key
  if (!['Control', 'Alt', 'Shift', 'Meta'].includes(mainKey)) {
    keys.push(mainKey.toUpperCase())
  }

  const isFunctionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(mainKey.toUpperCase())
  if (keys.length >= 2 || isFunctionKey) {
    const newShortcut = keys.join('+')
    if (target === 'global') {
      config.value.globalShortcut = newShortcut
      saveGlobalShortcut()
    } else {
      config.value.screenshotShortcut = newShortcut
      saveScreenshotShortcut()
    }
  }
}

async function saveApiConfig() {
  config.value.ocrEngine = FIXED_OCR_ENGINE
  config.value.ocrModelProfile = FIXED_OCR_MODEL_PROFILE

  const ocrRuntimeChanged = store.ocrEngine !== FIXED_OCR_ENGINE
    || store.ocrModelProfile !== FIXED_OCR_MODEL_PROFILE
    || config.value.ocrPreloadOnStartup !== store.ocrPreloadOnStartup

  if (
    config.value.apiKey === store.apiKey
    && config.value.apiSecret === store.apiSecret
    && config.value.translationProvider === store.translationProvider
    && config.value.microsoftTranslatorKey === store.microsoftTranslatorKey
    && config.value.microsoftTranslatorRegion === store.microsoftTranslatorRegion
    && config.value.ocrEndpoint === store.ocrEndpoint
    && config.value.ocrEngine === store.ocrEngine
    && config.value.ocrModelProfile === store.ocrModelProfile
    && config.value.ocrPreloadOnStartup === store.ocrPreloadOnStartup
  ) {
    return
  }

  try {
    await store.updateApiConfig({
      apiKey: config.value.apiKey,
      apiSecret: config.value.apiSecret,
      translationProvider: config.value.translationProvider,
      microsoftTranslatorKey: config.value.microsoftTranslatorKey,
      microsoftTranslatorRegion: config.value.microsoftTranslatorRegion,
      ocrEndpoint: config.value.ocrEndpoint,
      ocrEngine: FIXED_OCR_ENGINE,
      ocrModelProfile: FIXED_OCR_MODEL_PROFILE,
      ocrPreloadOnStartup: config.value.ocrPreloadOnStartup,
    })
    notify.success('服务配置已保存')
    if (ocrRuntimeChanged) {
      await refreshOcrStatus()
    }
  } catch (e) {
    notify.error(`保存服务配置失败: ${e}`)
  }
}

async function warmupOcrService() {
  ocrWarmupLoading.value = true
  try {
    const result = await store.warmupOcrService()
    await refreshOcrStatus()
    notify.success(result)
  } catch (e) {
    await refreshOcrStatus()
    notify.error(`OCR 预热失败: ${e}`)
  } finally {
    ocrWarmupLoading.value = false
  }
}

async function saveGlobalShortcut() {
  if (config.value.globalShortcut === store.globalShortcut) {
    return
  }

  if (config.value.globalShortcut === config.value.screenshotShortcut) {
    notify.error('两个快捷键不能相同')
    config.value.globalShortcut = store.globalShortcut
    return
  }

  try {
    await store.updateGlobalShortcut(config.value.globalShortcut)
    notify.success('快捷键已更新')
  } catch (e) {
    notify.error(`更新快捷键失败: ${e}`)
    config.value.globalShortcut = store.globalShortcut
  }
}

async function saveScreenshotShortcut() {
  if (config.value.screenshotShortcut === store.screenshotShortcut) {
    return
  }

  if (config.value.screenshotShortcut === config.value.globalShortcut) {
    notify.error('两个快捷键不能相同')
    config.value.screenshotShortcut = store.screenshotShortcut
    return
  }

  try {
    await store.updateScreenshotShortcut(config.value.screenshotShortcut)
    notify.success('截图快捷键已更新')
  } catch (e) {
    notify.error(`更新截图快捷键失败: ${e}`)
    config.value.screenshotShortcut = store.screenshotShortcut
  }
}

async function saveTrayBehavior() {
  try {
    await store.updateTrayBehavior(config.value.enableTray)
    notify.success('托盘行为已更新')
  } catch (e) {
    notify.error(`更新托盘行为失败: ${e}`)
  }
}

async function saveAutostartBehavior() {
  try {
    if (config.value.enableAutostart) {
      await enableAutostart()
    } else {
      await disableAutostart()
    }
    notify.success('开机启动已更新')
  } catch (e) {
    console.warn('开机启动设置失败（开发模式下正常）:', e)
  }
}

async function refreshOcrStatus() {
  ocrStatusLoading.value = true
  try {
    ocrStatus.value = await store.getOcrServiceStatus()
  } catch (e) {
    console.warn('读取 OCR 服务状态失败:', e)
  } finally {
    ocrStatusLoading.value = false
  }
}

async function changeTheme() {
  try {
    await store.updateTheme(config.value.theme)
  } catch (e) {
    notify.error(`切换主题失败: ${e}`)
    config.value.theme = store.theme
    return
  }

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

  notify.success(`已切换到 ${themeNames[config.value.theme]} 主题`)
}
</script>

<template>
  <main class="settings-page">
    <header class="settings-title-row">
      <div class="title-icon">
        <Settings2 :size="22" :stroke-width="2.1" />
      </div>
      <h1>设置</h1>
    </header>

    <section class="settings-panel">
      <div class="panel-hero">
        <div class="hero-icon">
          <CloudCog :size="24" :stroke-width="2.1" />
        </div>
        <div>
          <h2>翻译与 OCR 服务</h2>
          <p>单词查询优先使用本地词典；句子、截图 OCR 结果会使用这里选择的在线翻译服务。</p>
          <div class="provider-pill">
            <CheckCircle2 :size="16" :stroke-width="2.2" />
            <span>当前使用：{{ providerName }}</span>
            <n-tag :type="providerStatusType" size="small" round>{{ providerStatus }}</n-tag>
          </div>
        </div>
      </div>

      <n-divider />

      <div class="settings-grid">
        <label class="field-label">句子翻译服务</label>
        <n-select
          v-model:value="config.translationProvider"
          :options="providerOptions"
          size="medium"
          @update:value="saveApiConfig"
        />

        <template v-if="config.translationProvider === 'microsoft'">
          <label class="field-label">
            Microsoft Translator Key
            <n-tooltip trigger="hover">
            <template #trigger>
                <ShieldQuestion :size="15" class="field-help" />
              </template>
              Azure Translator 资源的 Key 1 或 Key 2。
            </n-tooltip>
          </label>
          <n-input
            v-model:value="config.microsoftTranslatorKey"
            :type="showMicrosoftKey ? 'text' : 'password'"
            size="medium"
            placeholder="请输入 Microsoft Translator Key"
            show-password-on="mousedown"
            @blur="saveApiConfig"
          >
            <template #suffix>
              <button class="icon-button" type="button" @click="showMicrosoftKey = !showMicrosoftKey">
                <component :is="showMicrosoftKey ? EyeOff : Eye" :size="19" />
              </button>
            </template>
          </n-input>

          <label class="field-label">
            Microsoft Translator Region
            <n-tooltip trigger="hover">
            <template #trigger>
                <ShieldQuestion :size="15" class="field-help" />
              </template>
              东亚资源填写 eastasia；Global 资源可留空。
            </n-tooltip>
          </label>
          <n-select
            v-model:value="config.microsoftTranslatorRegion"
            :options="microsoftRegionOptions"
            size="medium"
            filterable
            tag
            placeholder="例如 eastasia；global 资源可留空"
            @blur="saveApiConfig"
            @update:value="saveApiConfig"
          />

          <div />
          <n-button
            text
            tag="a"
            href="https://portal.azure.com/#create/Microsoft.CognitiveServicesTextTranslation"
            target="_blank"
            type="primary"
            class="external-link"
          >
            <template #icon><ExternalLink :size="16" /></template>
            创建微软翻译资源
          </n-button>
        </template>

        <template v-else>
          <label class="field-label">有道 AppID</label>
          <n-input
            v-model:value="config.apiKey"
            size="medium"
            placeholder="请输入有道翻译应用ID"
            @blur="saveApiConfig"
          />

          <label class="field-label">有道 App Secret</label>
          <n-input
            v-model:value="config.apiSecret"
            :type="showYoudaoSecret ? 'text' : 'password'"
            size="medium"
            placeholder="请输入有道翻译 App Secret"
            @blur="saveApiConfig"
          >
            <template #suffix>
              <button class="icon-button" type="button" @click="showYoudaoSecret = !showYoudaoSecret">
                <component :is="showYoudaoSecret ? EyeOff : Eye" :size="19" />
              </button>
            </template>
          </n-input>

          <div />
          <n-button
            text
            tag="a"
            href="https://ai.youdao.com/console/#/"
            target="_blank"
            type="primary"
            class="external-link"
          >
            <template #icon><ExternalLink :size="16" /></template>
            点击这里获取有道翻译 API 密钥
          </n-button>
        </template>
      </div>

      <n-divider />

      <div class="settings-grid ocr-grid">
        <label class="field-label">OCR 运行时</label>
        <div class="ocr-runtime-card">
          <div class="ocr-runtime-main">
            <n-tag type="success" size="small" round>内置</n-tag>
            <strong>ONNX Runtime + PP-OCRv6 Small</strong>
          </div>
          <p>当前版本固定使用原生 ONNX OCR，不启动 Python/PaddleOCR/RapidOCR 服务。</p>
        </div>

        <label class="field-label">启动时预热 OCR</label>
        <div class="switch-inline-row">
          <n-switch v-model:value="config.ocrPreloadOnStartup" @update:value="saveApiConfig" />
          <span>应用启动后后台初始化 OCR，第一次截图等待更短。</span>
        </div>

        <div />
        <div class="ocr-status-card">
          <div class="ocr-status-row">
            <n-tag :type="ocrStatusType" size="small" round>{{ ocrStatusText }}</n-tag>
            <span class="ocr-version">{{ ocrVersionText }}</span>
            <n-button
              quaternary
              circle
              size="small"
              :loading="ocrStatusLoading"
              @click="refreshOcrStatus"
            >
              <template #icon><RefreshCw :size="15" /></template>
            </n-button>
          </div>
          <div class="ocr-model-line" :class="{ 'is-muted': !ocrStatus?.modelDir }">
            {{ ocrModelText }}
          </div>
          <div v-if="ocrStatus?.modelDir" class="ocr-path-line">{{ ocrStatus.modelDir }}</div>
          <div class="ocr-action-row">
            <n-button
              size="small"
              secondary
              :loading="ocrWarmupLoading"
              @click="warmupOcrService"
            >
              <template #icon><Play :size="14" /></template>
              预热
            </n-button>
          </div>
        </div>

        <div v-if="ocrStatusDetail" />
        <p v-if="ocrStatusDetail" class="ocr-detail" :class="{ 'is-error': ocrStatus?.lastError }">
          {{ ocrStatusDetail }}
        </p>
      </div>
    </section>

    <section class="settings-panel compact-panel">
      <div class="compact-heading">
        <Keyboard :size="19" :stroke-width="2" />
        <h2>快捷键设置</h2>
      </div>
      <div class="settings-grid">
        <label class="field-label">全局翻译快捷键</label>
        <n-input
          :value="config.globalShortcut"
          size="medium"
          placeholder="点击后按下快捷键组合"
          readonly
          @focus="capturingShortcut = 'global'"
          @blur="capturingShortcut = null"
          @keydown.prevent="captureShortcut($event, 'global')"
        >
          <template #prefix><Keyboard :size="16" /></template>
        </n-input>
        <div />
        <p class="field-note">{{ capturingShortcut === 'global' ? '正在监听快捷键组合' : `当前快捷键：${config.globalShortcut}` }}</p>

        <label class="field-label">截图 OCR 快捷键</label>
        <n-input
          :value="config.screenshotShortcut"
          size="medium"
          placeholder="点击后按下快捷键组合"
          readonly
          @focus="capturingShortcut = 'screenshot'"
          @blur="capturingShortcut = null"
          @keydown.prevent="captureShortcut($event, 'screenshot')"
        >
          <template #prefix><Keyboard :size="16" /></template>
        </n-input>
        <div />
        <p class="field-note">{{ capturingShortcut === 'screenshot' ? '正在监听截图快捷键' : `当前快捷键：${config.screenshotShortcut}` }}</p>
      </div>
    </section>

    <section class="settings-panel compact-panel two-column-panels">
      <div>
        <div class="compact-heading">
          <Palette :size="19" :stroke-width="2" />
          <h2>外观</h2>
        </div>
        <n-select
          v-model:value="config.theme"
          :options="themeOptions"
          size="medium"
          @update:value="changeTheme"
        />
      </div>

      <div>
        <div class="compact-heading">
          <MonitorCog :size="19" :stroke-width="2" />
          <h2>窗口行为</h2>
        </div>
        <div class="switch-list">
          <div class="switch-row">
            <div>
              <strong>关闭主窗口时最小化到托盘</strong>
              <p>开启后点击右上角关闭只会隐藏到托盘。</p>
            </div>
            <n-switch v-model:value="config.enableTray" @update:value="saveTrayBehavior" />
          </div>
          <div class="switch-row">
            <div>
              <strong>开机启动</strong>
              <p>应用会在系统登录后自动启动。</p>
            </div>
            <n-switch
              v-model:value="config.enableAutostart"
              :loading="autostartLoading"
              :disabled="autostartLoading"
              @update:value="saveAutostartBehavior"
            />
          </div>
        </div>
      </div>
    </section>
  </main>
</template>

<style scoped>
.settings-page {
  width: min(760px, calc(100vw - 270px));
  margin: 0 auto;
  padding: 42px 0 34px;
}

.settings-title-row {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 20px;
}

.title-icon {
  width: 42px;
  height: 42px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  color: var(--color-app-accent);
  background: var(--color-app-accent-tint);
}

.settings-title-row h1 {
  margin: 0;
  color: var(--color-app-text-strong);
  font-size: 28px;
  font-weight: 760;
  letter-spacing: 0;
}

.settings-panel {
  margin-bottom: 16px;
  padding: 22px 24px;
  border: 1px solid var(--color-app-panel-border);
  border-radius: 8px;
  background: var(--color-app-panel-bg);
  box-shadow: var(--shadow-app-panel);
}

.panel-hero {
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: 16px;
  align-items: center;
}

.hero-icon {
  width: 48px;
  height: 48px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-on-primary);
  border-radius: 12px;
  background: linear-gradient(135deg, var(--color-app-accent-light), var(--color-app-accent-strong));
  box-shadow: var(--shadow-app-accent);
}

.panel-hero h2,
.compact-heading h2 {
  margin: 0;
  color: var(--color-app-text-strong);
  font-size: 20px;
  font-weight: 720;
  letter-spacing: 0;
}

.panel-hero p {
  margin: 5px 0 0;
  color: var(--color-app-text-muted);
  font-size: 13px;
  line-height: 1.55;
}

.provider-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 9px;
  padding: 4px 9px;
  border: 1px solid var(--color-app-accent-border);
  border-radius: 999px;
  color: var(--color-app-accent-strong);
  background: var(--color-app-accent-tint-soft);
  font-size: 12px;
}

.settings-grid {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr);
  gap: 13px 18px;
  align-items: center;
}

.field-label {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--color-app-text-strong);
  font-size: 14px;
  font-weight: 650;
}

.field-help {
  color: var(--color-app-accent-soft);
}

.field-note {
  margin: -5px 0 0;
  color: var(--color-app-text-muted);
  font-size: 13px;
}

.external-link {
  justify-self: start;
  font-size: 14px;
}

.icon-button {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  color: var(--color-app-text-muted);
  background: transparent;
  cursor: pointer;
}

.ocr-runtime-card {
  display: grid;
  gap: 6px;
  padding: 11px 12px;
  border: 1px solid var(--color-app-panel-border);
  border-radius: 8px;
  background: var(--color-app-panel-bg);
}

.ocr-runtime-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ocr-runtime-main strong {
  color: var(--color-app-text-strong);
  font-size: 14px;
  font-weight: 680;
}

.ocr-runtime-card p {
  margin: 0;
  color: var(--color-app-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

.ocr-runtime-card,
.ocr-status-card {
  min-width: 0;
}

.ocr-status-row {
  min-height: 28px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-app-text-muted);
}

.ocr-status-card {
  display: grid;
  gap: 8px;
  padding: 11px 12px;
  border: 1px solid var(--color-app-panel-border);
  border-radius: 8px;
  background: var(--color-app-panel-bg);
}

.ocr-version {
  font-size: 12px;
}

.ocr-model-line,
.ocr-path-line {
  color: var(--color-app-text-muted);
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.ocr-model-line:not(.is-muted) {
  color: var(--color-app-accent-strong);
}

.ocr-action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.switch-inline-row {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--color-app-text-muted);
  font-size: 13px;
  line-height: 1.45;
}

.ocr-detail {
  margin: -4px 0 0;
  color: var(--color-app-text-muted);
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.ocr-detail.is-error {
  color: var(--color-error);
}

.compact-panel {
  padding: 20px 24px;
}

.compact-heading {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-bottom: 14px;
  color: var(--color-app-accent);
}

.two-column-panels {
  display: grid;
  grid-template-columns: 1fr 1.2fr;
  gap: 24px;
}

.switch-list {
  display: grid;
  gap: 14px;
}

.switch-row {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  gap: 16px;
}

.switch-row strong {
  color: var(--color-app-text-strong);
  font-size: 14px;
  font-weight: 650;
}

.switch-row p {
  margin: 4px 0 0;
  color: var(--color-app-text-muted);
  font-size: 12px;
  line-height: 1.5;
}

:deep(.n-input),
:deep(.n-base-selection) {
  --n-border-radius: 8px !important;
  --n-height: 42px !important;
}

:deep(.n-button) {
  --n-border-radius: 8px !important;
}

:deep(.n-divider) {
  margin: 18px 0;
}

@media (max-width: 980px) {
  .settings-page {
    width: calc(100vw - 238px);
  }

  .settings-grid,
  .two-column-panels {
    grid-template-columns: 1fr;
  }

}
</style>
