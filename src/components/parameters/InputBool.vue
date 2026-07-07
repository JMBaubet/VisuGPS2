<script setup lang="ts">
// Booléen : v-switch pour un rendu immédiat de l'état activé/désactivé.
// Le défaut est rappelé discrètement en hint.
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'

const props = defineProps<{
  def: SettingDefinition
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const defaultHint = computed(() =>
  props.def.default ? 'Par défaut : activé' : 'Par défaut : désactivé'
)

function onToggle() {
  emit('update:modelValue', !props.modelValue)
}
</script>

<template>
  <v-switch
    :model-value="modelValue"
    color="primary"
    label="Activé"
    :hint="defaultHint"
    persistent-hint
    hide-details="auto"
    @update:model-value="onToggle"
  ></v-switch>
</template>
