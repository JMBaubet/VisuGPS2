<template>
  <v-app-bar>
    <!-- Retour à l'accueil -->
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Retour à l'accueil"
      @click="emit('back')"
    />
    <v-icon icon="mdi-broom" color="primary" class="ml-2" />
    <v-toolbar-title class="ml-2 text-truncate">
      Nettoyage — {{ traceName }}
    </v-toolbar-title>

    <v-spacer />

    <!-- Widget « boîte à états » : avancement des 3 étapes du pipeline -->
    <CleaningPhaseStepper class="mr-4" />

    <!-- Tolérance de cap (détection des rebroussements — étapes 1 et 3).
         Masquée en phase « Rond-Points » : la détection y dépend des
         paramètres Nettoyage.RondPoints.* (réglables dans le drawer). -->
    <v-text-field
      v-if="cleaning.currentPhase !== 'roundabout'"
      v-model="toleranceInput"
      label="Tolérance cap"
      suffix="°"
      type="number"
      density="compact"
      hide-details
      style="max-width: 140px"
      title="Seuil de détection des demi-tours (changement de cap ~180°)"
      @change="onToleranceChange"
    />

    <v-btn
      prepend-icon="mdi-content-save"
      class="ml-2"
      :loading="saving"
      :disabled="!cleaning.hasCases"
      @click="emit('save')"
    >
      Enregistrer
    </v-btn>
    <v-btn
      prepend-icon="mdi-backup-restore"
      variant="text"
      :disabled="!cleaning.hasCases"
      @click="emit('reset')"
    >
      Réinitialiser
    </v-btn>
    <v-btn
      prepend-icon="mdi-check-decagram"
      color="green"
      class="ml-1"
      :disabled="!canValidate"
      :title="validateTitle"
      :loading="validating"
      @click="emit('validate')"
    >
      {{ validateLabel }}
    </v-btn>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre d'outils de la vue de nettoyage : retour, nom de la trace, widget
 * « boîte à états » (3 étapes du pipeline), tolérance de cap (étapes 1/3),
 * et actions Enregistrer (sauvegarde partielle) / Réinitialiser /
 * **Valider l'étape** (applique les corrections de la phase, réécrit le GPX
 * et passe à l'étape suivante).
 */
import { ref, computed } from 'vue'
import { useCleaningStore, CLEANING_PHASES } from '../../stores/cleaning'
import { useTracesStore } from '../../stores/traces'
import CleaningPhaseStepper from './CleaningPhaseStepper.vue'

const cleaning = useCleaningStore()
const tracesStore = useTracesStore()

const props = withDefaults(
  defineProps<{
    /** État de chargement du bouton Enregistrer (piloté par la vue). */
    saving?: boolean
    /** État de chargement du bouton Valider l'étape (piloté par la vue). */
    validating?: boolean
  }>(),
  { saving: false, validating: false },
)
void props // exposé au template par nom (saving / validating)

const emit = defineEmits<{
  (e: 'back'): void
  (e: 'save'): void
  (e: 'reset'): void
  (e: 'validate'): void
}>()

/** Nom de la trace sélectionnée (pour le titre). */
const traceName = computed(() => {
  const t = tracesStore.traces.find(t => t.id === cleaning.selectedTraceId)
  return t ? t.name : '…'
})

/** Étape affichée validable ? (étape courante du pipeline, tous cas validés —
 * on ne re-valide jamais une étape déjà franchie). */
const canValidate = computed(() => cleaning.canValidatePhase)

const currentPhaseNum = computed(
  () => CLEANING_PHASES.find(p => p.id === cleaning.currentPhase)?.num ?? 1,
)

const validateLabel = computed(() => {
  if (cleaning.currentPhase === 'out_and_back') return 'Étape 3 à venir'
  return `Valider l'étape ${currentPhaseNum.value}`
})

const validateTitle = computed(() => {
  if (cleaning.currentPhase === 'out_and_back') {
    return 'Étape « Aller/Retour » non implémentée : elle produira un autre type de fichier'
  }
  if (cleaning.phaseValidated(cleaning.currentPhase)) {
    return 'Cette étape a déjà été validée — aucune nouvelle validation nécessaire (revenir à l\'étape courante)'
  }
  if (cleaning.allValidated) {
    return 'Appliquer les corrections de cette étape, enregistrer le GPX et passer à l\'étape suivante'
  }
  return 'Tous les cas de cette étape doivent être validés avant de valider l\'étape'
})

/** Saisie locale de la tolérance (appliquée au change). */
const toleranceInput = ref(String(cleaning.toleranceDeg))

function onToleranceChange() {
  const v = parseFloat(toleranceInput.value)
  if (Number.isNaN(v) || v <= 0) return
  cleaning.toleranceDeg = v
  void cleaning.reDetect()
}
</script>
