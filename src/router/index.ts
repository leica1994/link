import { createRouter, createWebHashHistory } from 'vue-router'
import Home from '../views/Home.vue'
import Translate from '../views/Translate.vue'
import Settings from '../views/Settings.vue'

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: Home },
    { path: '/translate', component: Translate },
    { path: '/settings', component: Settings },
  ],
})
