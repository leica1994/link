<template>
  <nav class="sidebar">
    <RouterLink to="/" class="nav-item" :class="{ active: route.path === '/' }">
      <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/>
      </svg>
      <span>主页</span>
    </RouterLink>
    <RouterLink to="/settings" class="nav-item" :class="{ active: route.path === '/settings' }">
      <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M12 2v2M12 20v2M4.93 4.93a10 10 0 0 0 0 14.14M2 12h2M20 12h2"/>
      </svg>
      <span>设置</span>
    </RouterLink>
  </nav>
</template>

<script setup lang="ts">
import { useRoute, RouterLink } from 'vue-router'
const route = useRoute()
</script>
