import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface SettingDefinition {
  path: string
  description: string
  doc: string
  type: 'Entier' | 'Secret' | string
  default: any
  value: any
  min?: number | null
  max?: number | null
  step?: number | null
  critique?: boolean | null
  is_overridden: boolean
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<SettingDefinition[]>([])
  const loading = ref(false)

  async function loadSettings() {
    loading.value = true
    try {
      const result = await invoke<SettingDefinition[]>('get_settings')
      settings.value = result
    } catch (error) {
      console.error('Failed to load settings:', error)
      settings.value = []
    } finally {
      loading.value = false
    }
  }

  async function updateSetting(path: string, value: any) {
    loading.value = true
    try {
      await invoke('update_setting', { path, value })
      await loadSettings() // recharger pour obtenir les valeurs masquées des secrets
    } catch (error) {
      console.error(`Failed to update setting ${path}:`, error)
      throw error
    } finally {
      loading.value = false
    }
  }

  async function resetSetting(path: string) {
    loading.value = true
    try {
      await invoke('reset_setting', { path })
      await loadSettings()
    } catch (error) {
      console.error(`Failed to reset setting ${path}:`, error)
      throw error
    } finally {
      loading.value = false
    }
  }

  async function getSettingValue(path: string): Promise<any> {
    try {
      return await invoke<any>('get_setting_value', { path })
    } catch (error) {
      console.error(`Failed to get setting value for ${path}:`, error)
      throw error
    }
  }

  return {
    settings,
    loading,
    loadSettings,
    updateSetting,
    resetSetting,
    getSettingValue,
  }
})
