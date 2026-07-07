<script setup lang="ts">
// Liste : v-select si des choix sont définis (sélection contrainte),
// sinon v-combobox autorisant la saisie libre.
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'

const props = defineProps<{
  def: SettingDefinition
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const hasChoices = computed(() => Array.isArray(props.def.choices) && props.def.choices.length > 0)
const items = computed(() =>
  hasChoices.value ? (props.def.choices as (string | number)[]) : []
)
const defaultHint = computed(() => `Défaut : ${props.def.default}`)
</script>

<template>
  <v-select
    v-if="hasChoices"
    :model-value="modelValue"
    :items="items"
    variant="outlined"
    density="comfortable"
    :hint="defaultHint"
    persistent-hint
    hide-details="auto"
    @update:model-value="emit('update:modelValue', $event)"
  ></v-select>

  <v-combobox
    v-else
    :model-value="modelValue"
    variant="outlined"
    density="comfortable"
    :hint="defaultHint"
    persistent-hint
    hide-details="auto"
    @update:model-value="emit('update:modelValue', $event)"
  ></v-combobox>
</template>
