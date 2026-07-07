<script setup lang="ts">
// RGBA : v-color-picker en mode hexa (#RRGGBBAA) — Vuetify 3 gère nativement
// le canal alpha en hex étendu. Sous le picker : chaîne rgba() lisible (read-only)
// et un carré d'aperçu. Le défaut est rappelé.
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'
import { isValidHexAlpha } from '../../utils/materialColors'

const props = defineProps<{
  def: SettingDefinition
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

// v-color-picker peut émettre une valeur transitoire invalide ; on garantit
// la cohérence du stockage en normalisant côté draft via une valeur par défaut.
const safeValue = computed(() =>
  isValidHexAlpha(props.modelValue) ? props.modelValue : '#000000FF'
)

function normalizeColor(value: any): string {
  // Si c'est une chaîne
  if (typeof value === 'string') {
    const hex = value.replace('#', '')
    // 6 caractères => ajouter FF (alpha opaque)
    if (hex.length === 6) return '#' + hex.toUpperCase() + 'FF'
    // 8 caractères => conserver (en majuscules)
    if (hex.length === 8) return '#' + hex.toUpperCase()
    // Sinon, valeur invalide -> noir opaque
    return '#000000FF'
  }
  // Si c'est un tableau [r, g, b, a] (ou [r, g, b])
  if (Array.isArray(value) && value.length >= 3) {
    const [r, g, b, a = 1] = value
    const toHex = (n: number) =>
      Math.min(255, Math.max(0, Math.round(n))).toString(16).padStart(2, '0').toUpperCase()
    const alpha = Math.round(a * 255)
    return '#' + toHex(r) + toHex(g) + toHex(b) + toHex(alpha)
  }
  // Fallback
  return '#000000FF'
}
</script>

<template>
  <div>
    <v-color-picker
      :model-value="safeValue"
      mode="rgba"
      width="100%"  
      @update:model-value="emit('update:modelValue', normalizeColor($event))"
    ></v-color-picker>

  </div>
</template>

<style scoped>

</style>
