<template>
  <v-chip :color="chipColor" size="small" variant="tonal">
    <v-icon start :icon="chipIcon" />
    {{ done }}/{{ total }}
    <span v-if="fp > 0" class="ml-1">· {{ fp }} FP</span>
  </v-chip>
</template>

<script setup lang="ts">
/**
 * Badge de progression de l'audit : anomalies traitées sur le total, avec le
 * décompte des faux positifs. La couleur traduit l'avancement (erreur tant qu'il
 * reste des anomalies à traiter, succès quand tout est traité sans faux
 * positif, avertissement sinon).
 */
import { computed } from 'vue'

const props = defineProps<{
  pending: number
  corrected: number
  fp: number
}>()

const done = computed(() => props.corrected + props.fp)
const total = computed(() => done.value + props.pending)

const chipColor = computed(() => {
  if (props.pending === 0 && done.value === 0) return 'default'
  if (props.pending === 0 && props.fp === 0) return 'success'
  if (props.pending === 0) return 'warning'
  if (done.value > 0) return 'orange'
  return 'error'
})

const chipIcon = computed(() => {
  if (props.pending === 0 && done.value === 0) return 'mdi-information-outline'
  if (props.pending === 0) return 'mdi-check-circle'
  if (done.value > 0) return 'mdi-progress-clock'
  return 'mdi-alert-circle'
})
</script>
