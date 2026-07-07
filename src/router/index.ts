import { createRouter, createWebHashHistory } from 'vue-router'
import Home from '../views/Home.vue'
import Translate from '../views/Translate.vue'
import Dubbing from '../views/Dubbing.vue'
import SubtitleStyle from '../views/SubtitleStyle.vue'
import SubtitleBurn from '../views/SubtitleBurn.vue'
import ContentCopy from '../views/ContentCopy.vue'
import YoutubeMonitor from '../views/YoutubeMonitor.vue'
import Settings from '../views/Settings.vue'

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'Home', component: Home },
    { path: '/tasks/:taskId', name: 'HomeTaskDetail', component: Home },
    { path: '/translate', name: 'Translate', component: Translate, meta: { keepAlive: true } },
    { path: '/dubbing', name: 'Dubbing', component: Dubbing, meta: { keepAlive: true } },
    { path: '/subtitle-style', name: 'SubtitleStyle', component: SubtitleStyle },
    { path: '/subtitle-burn', name: 'SubtitleBurn', component: SubtitleBurn, meta: { keepAlive: true } },
    { path: '/copywriting', name: 'ContentCopy', component: ContentCopy, meta: { keepAlive: true } },
    { path: '/youtube-monitor', name: 'YoutubeMonitor', component: YoutubeMonitor },
    { path: '/youtube-monitor/unread', name: 'YoutubeMonitorUnread', component: YoutubeMonitor },
    { path: '/youtube-monitor/:channelId', name: 'YoutubeMonitorDetail', component: YoutubeMonitor },
    { path: '/settings', component: Settings },
  ],
})
