<template>
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-logo">
      <span class="titlebar-title">Link</span>
    </div>
    <div class="titlebar-controls">
      <button class="ctrl-btn" @click="minimize">
        <svg width="12" height="1" viewBox="0 0 12 1"><rect width="12" height="1" fill="currentColor"/></svg>
      </button>
      <button class="ctrl-btn" @click="toggleMaximize">
        <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1" fill="none" stroke="currentColor"/></svg>
      </button>
      <button class="ctrl-btn close-btn" @click="closeWindow">
        <svg width="10" height="10" viewBox="0 0 10 10"><line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1.2"/><line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1.2"/></svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
const win = getCurrentWindow()
const minimize = () => win.minimize()
const toggleMaximize = async () => (await win.isMaximized()) ? win.unmaximize() : win.maximize()
const closeWindow = () => win.close()
</script>
