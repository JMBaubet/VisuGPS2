<template>
  <v-chip :color="chipColor" size="small" variant="tonal">
    <v-icon start :icon="chipIcon" />
    {{ done }}/{{ total }}
    <span v-if="fp > 0" class="ml-1">· {{ fp }} FP</span>
  </v-chip>
</template>

<script setup lang="ts">
/**
 * Badge d'avancement des passages multiples : segments examinés sur le total,
 * avec le décompte des faux positifs.
 *
 * Même rendu et mêmes couleurs qu'`AuditProgressChip` — rouge tant que rien
 * n'est examiné, vert quand tout l'est, orange ou ambre en cours — pour que
 * l'avancement se lise de la même façon dans les deux vues.
 *
 * Les props diffèrent en revanche, et pour une raison de fond : l'audit ne
 * connaît que des anomalies à corriger, quand un segment de passages multiples
 * se contente le plus souvent d'être **approuvé**. `treated` compte donc les
 * segments ayant reçu un geste, quel qu'il soit — approuvé, écarté, fusionné —
 * et `fp` n'en isole que les segments écartés, pour le rappel du suffixe.
 *
 * Ce compteur reste un repère : la barrière n'exige rien, une détection juste
 * se valide telle quelle.
 */
import { computed } from 'vue'

const props = defineProps<{
  /** Segments sans geste — à examiner. */
  pending: number
  /** Segments examinés : approuvés, écartés ou fusionnés. */
  treated: number
  /** Segments écartés (faux positifs), parmi les examinés. */
  fp: number
}>()

const done = computed(() => props.treated)
const total = computed(() => props.treated + props.pending)

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
