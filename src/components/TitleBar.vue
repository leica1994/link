<template>
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-logo">
      <span class="titlebar-title">Link</span>
    </div>
    <div class="titlebar-controls">
      <button class="ctrl-btn" aria-label="最小化" @click="minimize">
        <Minus :size="14" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button class="ctrl-btn" aria-label="最大化" @click="toggleMaximize">
        <Square :size="12" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button class="ctrl-btn close-btn" aria-label="关闭" @click="closeWindow">
        <X :size="14" :stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Window } from '@tauri-apps/api/window'
import { Minus, Square, X } from 'lucide-vue-next'

let appWindow: Window | null = null

const getAppWindow = async () => {
  if (!('__TAURI_INTERNALS__' in window)) {
    return null
  }

  if (!appWindow) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    appWindow = getCurrentWindow()
  }

  return appWindow
}

const minimize = async () => {
  await (await getAppWindow())?.minimize()
}

const toggleMaximize = async () => {
  const win = await getAppWindow()

  if (!win) {
    return
  }

  if (await win.isMaximized()) {
    await win.unmaximize()
  } else {
    await win.maximize()
  }
}

const closeWindow = async () => {
  await (await getAppWindow())?.close()
}
</script>
