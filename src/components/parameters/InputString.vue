<script setup lang="ts">
// Texte libre (type "string" — décision 12) : valeur non chiffrée, sans borne.
// Champ simple, pré-rempli avec la valeur courante.
import type { SettingDefinition } from '../../stores/settings'

defineProps<{
  def: SettingDefinition
  /** Valeur du draft : texte saisi localement. */
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

function onInput(v: string) {
  emit('update:modelValue', v)
}
</script>

<template>
  <v-text-field
    :model-value="modelValue"
    :placeholder="def.default != null ? String(def.default) : ''"
    variant="outlined"
    density="comfortable"
    hide-details="auto"
    @update:model-value="onInput($event)"
  ></v-text-field>
</template>
