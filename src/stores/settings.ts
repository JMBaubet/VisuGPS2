import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// Union des types de paramètres pris en charge.
// `string` reste autorisé pour tolérer d'éventuels types non listés (ex: "monitor").
export type SettingType =
  | 'int'
  | 'float'
  | 'bool'
  | 'secret'
  | 'list'
  | 'rgba'
  | 'material_primary'
  | 'material_extended'
  | 'monitor'
  | (string & {})

export interface SettingDefinition {
  path: string
  description: string
  /** Texte Markdown rendu dans la carte (anciennement `doc`). */
  documentation: string
  type: SettingType
  default: any
  value: any
  min?: number | null
  max?: number | null
  step?: number | null
  /** Indique un paramètre sensible pour la stabilité (anciennement `critique`). */
  critical?: boolean | null
  /** Unité affichée (ex: "ms", "s"). */
  unit?: string | null
  /** Choix autorisés pour un paramètre de type `list`. */
  choices?: any[] | null
  is_overridden: boolean
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<SettingDefinition[]>([])
  const loading = ref(false)

  // Brouillons d'édition locaux (drafts), keyés par path.
  // Un draft représente la valeur en cours de saisie, non encore persistée.
  // Tant que draft[path] === currentValue(path), rien n'est considéré comme modifié.
  const drafts = ref<Record<string, any>>({})

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
      const def = getParamDef(path)
      if (def && value === def.default) {
        await invoke('reset_setting', { path })
      } else {
        await invoke('update_setting', { path, value })
      }
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

  // --- API de haut niveau consommée par ParameterCard ---------------------

  /** Définition complète d'un paramètre (recherche par path). */
  function getParamDef(key: string): SettingDefinition | undefined {
    return settings.value.find((s) => s.path === key)
  }

  /** Valeur actuellement persistée (source de vérité côté backend). */
  function currentValue(key: string): any {
    return getParamDef(key)?.value
  }

  /** Initialise (ou réinitialise) le draft à partir de la valeur persistée. */
  function initDraft(key: string) {
    drafts.value[key] = currentValue(key)
  }

  /** Lit la valeur du draft (ou la valeur persistée si aucun draft). */
  function getDraft(key: string): any {
    return key in drafts.value ? drafts.value[key] : currentValue(key)
  }

  /** true si la valeur en cours de saisie diffère de la valeur persistée. */
  function isModified(key: string): boolean {
    if (!(key in drafts.value)) return false
    return drafts.value[key] !== currentValue(key)
  }

  /** true si la valeur en cours de saisie est égale à la valeur par défaut. */
  function isDefault(key: string): boolean {
    const def = getParamDef(key)
    if (!def) return true
    return getDraft(key) === def.default
  }

  /** Persiste la valeur du draft, puis resynchronise le draft. */
  async function saveParam(key: string) {
    await updateSetting(key, drafts.value[key])
    initDraft(key)
  }

  /** Rétablit la valeur par défaut côté backend, puis resynchronise le draft. */
  async function resetParam(key: string) {
    await resetSetting(key)
    initDraft(key)
  }

  /** Nettoyage du draft lors d'une fermeture sans enregistrement. */
  function closeWithoutSaving(key: string) {
    delete drafts.value[key]
  }

  return {
    // état
    settings,
    loading,
    drafts,
    // persistance basique (conservée pour SettingsEditMonitor & usage interne)
    loadSettings,
    updateSetting,
    resetSetting,
    getSettingValue,
    // API haut niveau (ParameterCard)
    getParamDef,
    currentValue,
    initDraft,
    getDraft,
    isModified,
    isDefault,
    saveParam,
    resetParam,
    closeWithoutSaving,
  }
})
