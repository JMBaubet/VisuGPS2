import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface MonitorInfo {
  name: string | null
  position: [number, number]
  size: [number, number]
  scale_factor: number
}

export const useAppStore = defineStore('app', () => {
  const isDarkMode = ref(false)
  const displays = ref<MonitorInfo[]>([])
  const loading = ref(false)

  const theme = computed(() => isDarkMode.value ? 'dark' : 'light')

  function toggleDarkMode() {
    isDarkMode.value = !isDarkMode.value
  }

  async function loadDisplays() {
    loading.value = true
    try {
      const result = await invoke<MonitorInfo[]>('get_displays')
      displays.value = result
    } catch (error) {
      console.error('Failed to load displays:', error)
      displays.value = []
    } finally {
      loading.value = false
    }
  }

  return {
    isDarkMode,
    theme,
    toggleDarkMode,
    displays,
    loading,
    loadDisplays
  }
})
