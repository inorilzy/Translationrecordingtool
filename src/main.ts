import { createApp } from "vue";
import { createPinia } from "pinia";
import naive from 'naive-ui'
import '@unocss/reset/tailwind.css'
import 'virtual:uno.css'
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { bootstrapTheme } from './lib/theme-bootstrap'
import router from "./router";
import App from "./App.vue";

export async function bootstrap() {
  await bootstrapTheme()

  const pinia = createPinia();
  const app = createApp(App);

  let windowLabel = 'main';
  try {
    const currentWindow = getCurrentWebviewWindow();
    if (currentWindow) {
      windowLabel = currentWindow.label;
    }
  } catch (e) {
    console.error('获取窗口标签失败:', e);
  }

  app.use(pinia);
  app.use(router);
  app.use(naive);

  if (windowLabel === "popup" && router.currentRoute.value.path !== "/popup") {
    await router.replace("/popup");
  }

  if (windowLabel === "screenshot-selection" && router.currentRoute.value.path !== "/screenshot-selection") {
    await router.replace("/screenshot-selection");
  }

  if (windowLabel !== "popup" && windowLabel !== "screenshot-selection" && router.currentRoute.value.path === "/popup") {
    await router.replace("/history");
  }

  await router.isReady();
  app.mount("#app");
}

void bootstrap();
