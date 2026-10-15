import { createApp } from 'vue'

import '@orchester/design/tokens.css'
import '@orchester/design/index.css'
import './styles/app.css'

import App from './App.vue'
import { appI18n } from './i18n'
import { appRouter } from './router'
import { createAppStores } from './stores/app'

const app = createApp(App)
const stores = createAppStores()

app.use(appI18n).use(appRouter).use(stores)
// Finish the initial navigation before bootstrap removes its one-time token.
// Otherwise the router can put the original fragment back after exchange.
void appRouter.isReady().then(() => app.mount('#app'))
