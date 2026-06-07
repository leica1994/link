import { computed, ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark'

const storageKey = 'link-theme'
const fallbackTheme: ThemeMode = 'light'

const readStoredTheme = (): ThemeMode => {
  const stored = localStorage.getItem(storageKey)
  return stored === 'dark' || stored === 'light' ? stored : fallbackTheme
}

const currentTheme = ref<ThemeMode>(readStoredTheme())

const applyTheme = (theme: ThemeMode) => {
  document.documentElement.dataset.theme = theme
  document.documentElement.style.colorScheme = theme
}

applyTheme(currentTheme.value)

watch(currentTheme, (theme) => {
  applyTheme(theme)
  localStorage.setItem(storageKey, theme)
})

export const useTheme = () => {
  const isDark = computed(() => currentTheme.value === 'dark')
  const themeLabel = computed(() => (isDark.value ? '深色模式' : '浅色模式'))

  const setTheme = (theme: ThemeMode) => {
    currentTheme.value = theme
  }

  return {
    currentTheme,
    isDark,
    setTheme,
    themeLabel,
  }
}
