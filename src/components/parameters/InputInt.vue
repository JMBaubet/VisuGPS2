<script setup lang="ts">
// Entier : v-text-field type="number" + curseur v-slider si min/max sont définis.
// L'unité (def.unit) est affichée via le suffixe ; le défaut est rappelé en hint.
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'

const props = defineProps<{
  def: SettingDefinition
  modelValue: number
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: number): void
}>()

const hasRange = computed(() => props.def.min != null && props.def.max != null)
const hasUnit = computed(() => !!props.def.unit)
const rangeHint = computed(() => {
  const parts: string[] = []
  parts.push(`Défaut : ${props.def.default}`)
  if (props.def.min != null) parts.push(`Min : ${props.def.min}`)
  if (props.def.max != null) parts.push(`Max : ${props.def.max}`)
  return parts.join(' — ')
})

// v-slider émet un number ; v-text-field émet une string (via .number).
function onSlider(v: number) {
  emit('update:modelValue', v)
}
function onInput(v: number | undefined) {
  emit('update:modelValue', Number(v ?? props.def.default))
}
</script>

<template>
  <div>
    <div v-if="hasRange">
      <div class="d-flex align-center">
        <v-slider
          :model-value="modelValue"
          :min="def.min ?? 0"
          :max="def.max ?? 100"
          :step="def.step ?? 1"
          color="primary"
          hide-details
          class="align-center mr-4"
          @update:model-value="onSlider"
        ></v-slider>
        <v-text-field
          :model-value="modelValue"
          type="number"
          density="compact"
          variant="outlined"
          hide-details
          :min="def.min ?? undefined"
          :max="def.max ?? undefined"
          :step="def.step ?? 1"
          :suffix="hasUnit ? def.unit ?? undefined : undefined"
          style="width: 120px"
          @update:model-value="onInput(Number($event))"
        ></v-text-field>
      </div>
      <div class="text-caption text-medium-emphasis mt-1 pl-1">{{ rangeHint }}</div>
    </div>

    <v-text-field
      v-else
      :model-value="modelValue"
      type="number"
      variant="outlined"
      density="comfortable"
      :min="def.min ?? undefined"
      :max="def.max ?? undefined"
      :step="def.step ?? 1"
      :suffix="hasUnit ? def.unit ?? undefined : undefined"
      :hint="rangeHint"
      persistent-hint
      hide-details="auto"
      @update:model-value="onInput(Number($event))"
    ></v-text-field>
  </div>
</template>
