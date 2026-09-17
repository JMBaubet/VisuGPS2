<template>
  <v-chip :color="chipColor" size="small" variant="tonal">
    <v-icon start :icon="chipIcon" />
    {{ done }}/{{ total }}
    <span v-if="fp > 0" class="ml-1">· {{ fp }} FP</span>
  </v-chip>
</template>

<script setup lang="ts">
/**
 * Badge d'avancement des passages multiples : segments ajustés sur le total,
 * avec le décompte des faux positifs.
 *
 * Copie conforme d'`AuditProgressChip` — mêmes props, mêmes couleurs, même
 * rendu — pour que l'avancement se lise de la même façon dans les deux vues :
 * rouge tant que rien n'est ajusté, vert quand tout l'est, orange ou ambre en
 * cours, et le rappel des faux positifs.
 *
 * Le vocabulaire diffère d'une vue à l'autre : `corrected`, qui compte les
 * anomalies corrigées côté audit, compte ici les segments **fusionnés**. C'est
 * l'ajustement qui tient lieu de correction, et la barrière n'exigeant rien —
 * une détection juste se valide telle quelle —, ce compteur reste un repère,
 * jamais une condition.
 */
import { computed } from 'vue'

const props = defineProps<{
  /** Segments sans ajustement. */
  pending: number
  /** Segments fusionnés avec leur précédent. */
  corrected: number
  /** Segments marqués faux positifs. */
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
