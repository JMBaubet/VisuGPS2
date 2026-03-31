import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'

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
    // Synchroniser le thème avec toutes les fenêtres
    emit('theme-changed', isDarkMode.value)
  }

  // Écouter les changements de thème émis par une autre fenêtre
  listen<boolean>('theme-changed', (event) => {
    isDarkMode.value = event.payload
  })

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
