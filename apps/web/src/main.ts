import { createApp } from 'vue'

import '@orchester/design/tokens.css'
import '@orchester/design/index.css'
import './styles/app.css'

import App from './App.vue'
import { appRouter } from './router'

createApp(App).use(appRouter).mount('#app')
