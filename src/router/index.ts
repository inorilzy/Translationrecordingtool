import { createRouter, createWebHistory } from 'vue-router'
import HomePage from '../views/HomePage.vue'
import PopupWindow from '../views/PopupWindow.vue'
import HistoryPage from '../views/HistoryPage.vue'
import FavoritesPage from '../views/FavoritesPage.vue'
import DetailPage from '../views/DetailPage.vue'
import SettingsPage from '../views/SettingsPage.vue'

const routes = [
  {
    path: '/',
    name: 'Home',
    component: HomePage
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
