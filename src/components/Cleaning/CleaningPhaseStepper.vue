<template>
  <div class="phase-stepper">
    <template v-for="(p, idx) in phases" :key="p.id">
      <v-chip
        class="phase-step"
        :color="chipColor(p)"
        :variant="p.id === cleaning.currentPhase ? 'flat' : 'tonal'"
        size="small"
        :disabled="!canClick(p)"
        :title="chipTitle(p)"
        @click="cleaning.goToPhase(p.id)"
      >
        <span class="phase-step__num mr-1">{{ p.num }}</span>
        <span class="phase-step__label">{{ p.label }}</span>
        <v-icon
          size="x-small"
          class="phase-step__badge"
          :color="phaseValidated(p) ? 'white' : ''"
          :icon="phaseValidated(p) ? 'mdi-check' : 'mdi-close'"
        />
      </v-chip>
      <v-icon
        v-if="idx < phases.length - 1"
        icon="mdi-arrow-right"
        size="small"
        class="phase-step__arrow"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Widget « boîte à états » de la barre d'outils du nettoyage : affiche les
 * 3 étapes du pipeline (Pts hors trace → Rond-Points → Aller/Retour) avec
 * leur validité (✓ validée / ✗ à faire), l'étape courante surlignée, et
 * permet de naviguer entre elles.
 *
 * Règles :
 * - l'étape 3 « Aller/Retour » n'est **pas cliquable** (non implémentée —
 *   elle produira un autre type de fichier, pas une modification du GPX) ;
 * - pour atteindre une étape, l'étape précédente doit être validée.
 */
import { useCleaningStore, CLEANING_PHASES, type CleaningPhaseDef } from '../../stores/cleaning'

const cleaning = useCleaningStore()
const phases = CLEANING_PHASES

/** Étape validée ? (d'après `cleaning_phase` persistée de la trace.) */
function phaseValidated(p: CleaningPhaseDef): boolean {
  return cleaning.phaseValidated(p.id)
}

/** Étape cliquable ? (pas l'étape 3 ; navigation séquentielle.) */
function canClick(p: CleaningPhaseDef): boolean {
  if (p.id === 'out_and_back') return false
  if (p.id === 'roundabout') return cleaning.phaseValidated('spike')
  return true
}

function chipColor(p: CleaningPhaseDef): string {
  if (p.id === cleaning.currentPhase) return 'primary'
  if (phaseValidated(p)) return 'green'
  return 'grey-darken-1'
}

function chipTitle(p: CleaningPhaseDef): string {
  if (p.id === 'out_and_back') {
    return 'Étape « Aller/Retour » à venir : produira un autre type de fichier (pas une modification du GPX)'
  }
  if (phaseValidated(p)) {
    return `Étape ${p.num} validée — cliquer pour revoir la détection sur le GPX courant`
  }
  if (p.id === cleaning.currentPhase) return `Étape ${p.num} en cours`
  return `Aller à l'étape ${p.num}`
}
</script>

<style scoped>
.phase-stepper {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.phase-step {
  max-width: 200px;
}

.phase-step__label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.phase-step__badge {
  margin-left: 6px;
}

.phase-step__arrow {
  color: rgba(0, 0, 0, 0.35);
  flex-shrink: 0;
}
</style>
