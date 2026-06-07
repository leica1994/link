<template>
  <nav class="sidebar">
    <RouterLink to="/" class="nav-item" :class="{ active: route.path === '/' }">
      <House class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>主页</span>
    </RouterLink>
    <RouterLink to="/translate" class="nav-item" :class="{ active: route.path === '/translate' }">
      <Languages class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>翻译</span>
    </RouterLink>
    <RouterLink to="/settings" class="nav-item" :class="{ active: route.path === '/settings' }">
      <Settings class="nav-icon" :stroke-width="2.4" aria-hidden="true" />
      <span>设置</span>
    </RouterLink>
  </nav>
</template>

<script setup lang="ts">
import { House, Languages, Settings } from 'lucide-vue-next'
import { useRoute, RouterLink } from 'vue-router'

const route = useRoute()
</script>
