import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAppStore = defineStore('app', () => {
  const isDarkMode = ref(false)

  const theme = computed(() => isDarkMode.value ? 'dark' : 'light')

  function toggleDarkMode() {
    isDarkMode.value = !isDarkMode.value
  }

  return {
    isDarkMode,
    theme,
    toggleDarkMode
  }
})
