<script setup lang="ts">
// Décimal : v-text-field type="number" avec step décimal (défaut 0.1).
// Semblable à InputInt mais adapté aux valeurs flottantes (min/max/step).
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'

const props = defineProps<{
  def: SettingDefinition
  modelValue: number
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: number): void
}>()

const step = computed(() => props.def.step ?? 0.1)
const hasUnit = computed(() => !!props.def.unit)
const rangeHint = computed(() => {
  const parts: string[] = []
  parts.push(`Défaut : ${props.def.default}`)
  if (props.def.min != null) parts.push(`Min : ${props.def.min}`)
  if (props.def.max != null) parts.push(`Max : ${props.def.max}`)
  return parts.join(' — ')
})

function onInput(v: number | undefined) {
  emit('update:modelValue', Number(v ?? props.def.default))
}
</script>

<template>
  <v-text-field
    :model-value="modelValue"
    type="number"
    variant="outlined"
    density="comfortable"
    :min="def.min ?? undefined"
    :max="def.max ?? undefined"
    :step="step"
    :suffix="hasUnit ? def.unit ?? undefined : undefined"
      :hint="rangeHint"
      persistent-hint
    hide-details="auto"
    @update:model-value="onInput(Number($event))"
  ></v-text-field>
</template>
