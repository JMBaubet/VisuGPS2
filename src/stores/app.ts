import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

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

  async function positionWindowOnActiveDisplay() {
    try {
      const activeDisplay = await invoke<MonitorInfo | null>('get_active_display')
      if (!activeDisplay) return

      const mainWindow = WebviewWindow.getByLabel('main')
      if (!mainWindow) return

      const [x, y] = activeDisplay.position
      const [width, height] = activeDisplay.size

      // Centrer la fenêtre sur l'écran actif (par défaut 800x600)
      const windowWidth = 800
      const windowHeight = 600
      const centerX = x + Math.floor((width - windowWidth) / 2)
      const centerY = y + Math.floor((height - windowHeight) / 2)

      await mainWindow.setPosition({ x: centerX, y: centerY })
    } catch (error) {
      console.error('Failed to position window:', error)
    }
  }

  return {
    isDarkMode,
    theme,
    toggleDarkMode,
    displays,
    loading,
    loadDisplays,
    positionWindowOnActiveDisplay
  }
})
