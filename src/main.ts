import { createApp } from 'vue'
import App from './App.vue'
import router from './router/index'
import './styles.css'
import './composables/useTheme'

window.addEventListener('contextmenu', (event) => {
  event.preventDefault()
})

createApp(App).use(router).mount('#app')
