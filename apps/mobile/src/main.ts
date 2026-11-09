import { createApp } from 'vue'
import App from './App.vue'
import LaunchScreen from './components/LaunchScreen.vue'
import './style.css'
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
