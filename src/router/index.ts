import { createRouter, createWebHashHistory } from 'vue-router'
import Home from '../views/Home.vue'
import Translate from '../views/Translate.vue'
import Dubbing from '../views/Dubbing.vue'
import YoutubeMonitor from '../views/YoutubeMonitor.vue'
import Settings from '../views/Settings.vue'

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: Home },
    { path: '/translate', name: 'Translate', component: Translate, meta: { keepAlive: true } },
    { path: '/dubbing', name: 'Dubbing', component: Dubbing, meta: { keepAlive: true } },
    { path: '/youtube-monitor', name: 'YoutubeMonitor', component: YoutubeMonitor },
    { path: '/youtube-monitor/:channelId', name: 'YoutubeMonitorDetail', component: YoutubeMonitor },
    { path: '/settings', component: Settings },
  ],
})
