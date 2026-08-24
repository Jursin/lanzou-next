import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import router from './router'
import { useAppStore } from './stores/app'
import { usePreferenceStore } from './stores/preference'
import { useTransferStore } from './stores/transfer'
import { useUploadTrafficStore } from './stores/uploadTraffic'

import '@/styles/tokens.css'
import '@/styles/base.css'
import '@/styles/preferences.css'

const app = createApp(App)

app.use(createPinia())
app.use(router)

const appStore = useAppStore()
const preferenceStore = usePreferenceStore()
const transferStore = useTransferStore()
const trafficStore = useUploadTrafficStore()

async function bootstrap() {
  await preferenceStore.load()
  await appStore.loadConfig()
  app.mount('#app')
  trafficStore.restore()
  if ('__TAURI_INTERNALS__' in window) {
    await appStore.setupLoginListener()
    await transferStore.init()
    await transferStore.setupListeners()
    if (appStore.isLoggedIn) {
      await appStore.refreshProfile()
    }
  }
}

bootstrap()
