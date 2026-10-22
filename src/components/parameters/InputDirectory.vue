<script setup lang="ts">
// Chemin de dossier (type "directory") : champ texte éditable + sélecteur
// natif. Le chemin est persisté comme une chaîne par le backend ; l'existence
// du dossier est vérifiée à l'usage (support OSM débranché = état, pas erreur
// de saisie).
import { invoke } from '@tauri-apps/api/core'
import type { SettingDefinition } from '../../stores/settings'

defineProps<{
  def: SettingDefinition
  /** Chemin du dossier en cours de saisie. */
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

function onInput(value: string) {
  emit('update:modelValue', value)
}

async function pickFolder() {
  try {
    const path = await invoke<string | null>('pick_directory')
    if (path) emit('update:modelValue', path)
  } catch (error) {
    console.error('[settings] Sélection du dossier impossible :', error)
  }
}
</script>

<template>
  <v-text-field
    :model-value="modelValue"
    :placeholder="def.default != null ? String(def.default) : ''"
    variant="outlined"
    density="comfortable"
    hide-details="auto"
    append-inner-icon="mdi-folder-open"
    @update:model-value="onInput($event)"
    @click:append-inner="pickFolder"
  ></v-text-field>
</template>
