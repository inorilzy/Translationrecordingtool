import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import router from "./router";
import App from "./App.vue";

// 初始化主题
function initTheme() {
  const savedTheme = localStorage.getItem('theme') || 'light'
  document.documentElement.setAttribute('data-theme', savedTheme)
}

async function bootstrap() {
  initTheme()

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

  if (windowLabel === "popup" && router.currentRoute.value.path !== "/popup") {
    await router.replace("/popup");
  }

  if (windowLabel !== "popup" && router.currentRoute.value.path === "/popup") {
    await router.replace("/history");
  }

  await router.isReady();
  app.mount("#app");
}

void bootstrap();
