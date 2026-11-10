import { createApp } from 'vue'
import { initFontAppearance } from '@orchester/design/fonts'
import '@orchester/design/fonts.css'
import App from './App.vue'
import LaunchScreen from './components/LaunchScreen.vue'
import './style.css'
initFontAppearance()
createApp(App).mount('#app')

// The real app starts loading independently while this one-shot presentation
// plays. Remove the overlay completely so returning from background is instant.
const appRoot = document.getElementById('app')!
const launchRoot = document.createElement('div')
appRoot.inert = true
document.body.append(launchRoot)
const launchApp = createApp(LaunchScreen, {
  onFinish: () => {
    appRoot.inert = false
    launchApp.unmount()
    launchRoot.remove()
  },
})
launchApp.mount(launchRoot)
