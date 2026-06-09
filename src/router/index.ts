import { createRouter, createWebHashHistory } from 'vue-router'
import Home from '../views/Home.vue'
import Translate from '../views/Translate.vue'
import Dubbing from '../views/Dubbing.vue'
import Settings from '../views/Settings.vue'

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: Home },
    { path: '/translate', name: 'Translate', component: Translate, meta: { keepAlive: true } },
    { path: '/dubbing', name: 'Dubbing', component: Dubbing, meta: { keepAlive: true } },
    { path: '/settings', component: Settings },
  ],
})
