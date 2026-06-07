<template>
  <div class="app-layout">
    <TitleBar />
    <div class="app-body">
      <Sidebar />
      <main class="app-content">
        <RouterView />
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import TitleBar from './components/TitleBar.vue'
import Sidebar from './components/Sidebar.vue'
import { RouterView } from 'vue-router'
</script>
