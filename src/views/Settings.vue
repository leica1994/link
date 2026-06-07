<template>
  <div class="page settings-page">
    <h1 class="page-title">设置</h1>

    <div class="settings-stack">
      <section class="settings-section" aria-labelledby="general-settings-title">
        <div id="general-settings-title" class="section-heading">
          <SlidersHorizontal aria-hidden="true" />
          <span>设置</span>
        </div>

        <div class="settings-panel">
          <div class="setting-row">
            <Moon class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">主题模式</div>
              <div class="setting-subtitle">{{ themeLabel }} · 立即生效</div>
            </div>
            <div class="theme-switch" role="group" aria-label="主题模式">
              <button
                class="theme-option"
                :class="{ active: currentTheme === 'light' }"
                type="button"
                :aria-pressed="currentTheme === 'light'"
                @click="setTheme('light')"
              >
                <Sun :stroke-width="2.1" aria-hidden="true" />
                浅色
              </button>
              <button
                class="theme-option"
                :class="{ active: currentTheme === 'dark' }"
                type="button"
                :aria-pressed="currentTheme === 'dark'"
                @click="setTheme('dark')"
              >
                <Moon :stroke-width="2.1" aria-hidden="true" />
                深色
              </button>
            </div>
          </div>

          <button class="setting-row setting-row-button" type="button">
            <CircleHelp class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">关于与支持</span>
              <span class="setting-subtitle">版本信息 · 帮助反馈</span>
            </span>
            <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
          </button>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronRight, CircleHelp, Moon, SlidersHorizontal, Sun } from 'lucide-vue-next'
import { useTheme } from '../composables/useTheme'

const { currentTheme, setTheme, themeLabel } = useTheme()
</script>
