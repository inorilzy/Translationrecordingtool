import { createRouter, createWebHistory } from 'vue-router'
import TranslatePage from '../views/TranslatePage.vue'
import PopupWindow from '../views/PopupWindow.vue'
import HistoryPage from '../views/HistoryPage.vue'
import FavoritesPage from '../views/FavoritesPage.vue'
import DetailPage from '../views/DetailPage.vue'
import LogsPage from '../views/LogsPage.vue'
import SettingsPage from '../views/SettingsPage.vue'

export const routes = [
  {
    path: '/',
    redirect: '/translate'
  },
  {
    path: '/translate',
    name: 'Translate',
    component: TranslatePage
  },
  {
    path: '/popup',
    name: 'Popup',
    component: PopupWindow
  },
  {
    path: '/history',
    name: 'History',
    component: HistoryPage
  },
  {
    path: '/favorites',
    name: 'Favorites',
    component: FavoritesPage
  },
  {
    path: '/detail/:id',
    name: 'Detail',
    component: DetailPage
  },
  {
    path: '/logs',
    name: 'Logs',
    component: LogsPage
  },
  {
    path: '/settings',
    name: 'Settings',
    component: SettingsPage
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
