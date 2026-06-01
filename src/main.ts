import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import 'virtual:uno.css'
import VueScan, { type VueScanOptions } from 'z-vue-scan'

const pinia = createPinia()
const app = createApp(App)

const isProduction = import.meta.env.PROD
if (!isProduction) {
  app.use<VueScanOptions>(VueScan, {})
}

app.use(pinia).mount('#app')
