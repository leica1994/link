<template>
  <nav class="sidebar">
    <RouterLink to="/" class="nav-item" :class="{ active: route.path === '/' || route.path.startsWith('/tasks') }">
      <House class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>主页</span>
    </RouterLink>
    <RouterLink to="/translate" class="nav-item" :class="{ active: route.path === '/translate' }">
      <Languages class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>翻译</span>
    </RouterLink>
    <RouterLink to="/dubbing" class="nav-item" :class="{ active: route.path === '/dubbing' }">
      <MicVocal class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>配音</span>
    </RouterLink>
    <RouterLink to="/subtitle-style" class="nav-item" :class="{ active: route.path === '/subtitle-style' }">
      <Type class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>字幕</span>
    </RouterLink>
    <RouterLink
      to="/youtube-monitor"
      class="nav-item"
      :class="{ active: route.path.startsWith('/youtube-monitor') }"
    >
      <Radar class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>监控</span>
    </RouterLink>
    <RouterLink to="/settings" class="nav-item" :class="{ active: route.path === '/settings' }">
      <Settings class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>设置</span>
    </RouterLink>
  </nav>
</template>

<script setup lang="ts">
import { House, Languages, MicVocal, Radar, Settings, Type } from 'lucide-vue-next'
import { useRoute, RouterLink } from 'vue-router'

const route = useRoute()
</script>
