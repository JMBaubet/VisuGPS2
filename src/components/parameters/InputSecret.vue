<script setup lang="ts">
// Secret : v-text-field basculant entre password et texte clair.
// La valeur persistée est masquée ("********") par le backend : on saisit donc
// une nouvelle valeur dans un champ dédié, jamais pré-rempli avec le secret réel.
import { ref, computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'

const props = defineProps<{
  def: SettingDefinition
  /** Valeur du draft : chaîne saisie localement, vide par défaut. */
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const showPassword = ref(false)

// Une valeur persistée non vide arrive masquée ; sert uniquement à l'affichage.
const hasStoredSecret = computed(() => props.def.value === '********')

const label = computed(() =>
  hasStoredSecret.value
    ? 'Secret configuré (saisissez une nouvelle valeur pour écraser)'
    : 'Saisissez le secret'
)

function onInput(v: string) {
  emit('update:modelValue', v)
}
</script>

<template>
  <v-text-field
    :model-value="modelValue"
    :type="showPassword ? 'text' : 'password'"
    :label="label"
    :placeholder="hasStoredSecret ? '********' : ''"
    variant="outlined"
    density="comfortable"
    :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
    hide-details="auto"
    autocomplete="new-password"
    @click:append-inner="showPassword = !showPassword"
    @update:model-value="onInput($event)"
  ></v-text-field>
</template>
