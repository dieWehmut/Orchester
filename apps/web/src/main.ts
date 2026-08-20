import { createApp } from 'vue'

import '@orchester/design/tokens.css'
import '@orchester/design/index.css'
import './styles/app.css'

import App from './App.vue'
import { appI18n } from './i18n'
import { appRouter } from './router'
import { createAppStores } from './stores/app'
import { createAppPinia } from './stores/pinia'

const app = createApp(App)
const stores = createAppStores()

app.use(appI18n).use(appRouter).use(createAppPinia()).use(stores).mount('#app')
