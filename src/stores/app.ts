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

export interface ModeInfo {
  nom: string
  descrition: string
  création: string
  révision?: string
}

export const useAppStore = defineStore('app', () => {
  const isDarkMode = ref(false)
  const displays = ref<MonitorInfo[]>([])
  const loading = ref(false)
  const isScreenBisOpen = ref(false)

  // Environnements et modes d'exécution
  const isDev = ref(false)
  const activeMode = ref('OPE')
  const activeModeDev = ref('OPE')
  const activeModeProd = ref('PROD')
  const isDebug = ref(false)
  const modes = ref<ModeInfo[]>([])
  const showModeDialog = ref(false)

  const theme = computed(() => isDarkMode.value ? 'dark' : 'light')

  // Description du mode actif
  const activeModeDescription = computed(() => {
    const found = modes.value.find(m => m.nom === activeMode.value)
    return found ? found.descrition : activeMode.value
  })

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

  // Actions de gestion des modes d'exécution
  async function loadExecutionEnv() {
    try {
      const env = await invoke<{ is_dev: boolean; active_mode_dev: string; active_mode_prod: string }>('get_execution_env')
      isDev.value = env.is_dev
      activeModeDev.value = env.active_mode_dev
      activeModeProd.value = env.active_mode_prod
      activeMode.value = env.is_dev ? env.active_mode_dev : env.active_mode_prod
    } catch (error) {
      console.error('Failed to load execution env:', error)
    }
  }

  async function loadModes() {
    try {
      const result = await invoke<ModeInfo[]>('get_modes')
      modes.value = result
    } catch (error) {
      console.error('Failed to load modes:', error)
    }
  }

  async function createMode(nom: string, descrition: string) {
    try {
      await invoke('create_mode', { nom, descrition })
      await loadModes()
    } catch (error) {
      console.error('Failed to create mode:', error)
      throw error
    }
  }

  async function updateMode(oldNom: string, newNom: string, descrition: string) {
    try {
      await invoke('update_mode', { oldNom, newNom, descrition })
      if (activeMode.value === oldNom) {
        activeMode.value = newNom
      }
      await loadModes()
    } catch (error) {
      console.error('Failed to update mode:', error)
      throw error
    }
  }

  async function deleteMode(nom: string) {
    try {
      console.log("deleteMode: ", nom)
      await invoke('delete_mode', { nom })
      await loadModes()
    } catch (error) {
      console.error('Failed to delete mode:', error)
      throw error
    }
  }

  async function selectMode(nom: string) {
    try {
      await invoke('select_mode', { nom })
    } catch (error) {
      console.error('Failed to select mode:', error)
      throw error
    }
  }

  function toggleDebug() {
    isDebug.value = !isDebug.value
  }

  return {
    isDarkMode,
    theme,
    toggleDarkMode,
    displays,
    loading,
    loadDisplays,
    isScreenBisOpen,
    isDev,
    activeMode,
    activeModeDev,
    activeModeProd,
    isDebug,
    modes,
    showModeDialog,
    activeModeDescription,
    loadExecutionEnv,
    loadModes,
    createMode,
    updateMode,
    deleteMode,
    selectMode,
    toggleDebug
  }
})
