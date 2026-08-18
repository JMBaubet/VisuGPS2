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

    <!-- Progression de la validation -->
    <span class="text-body-2 text-medium-emphasis mr-4">
      Cas {{ currentIndex + 1 }}/{{ totalCases }} validés
      <span class="font-weight-medium">{{ validatedCount }}</span>
    </span>

    <!-- Tolérance de cap (paramétrable, re-détection à la volée) -->
    <v-text-field
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
      :disabled="!cleaning.allValidated"
      :title="
        cleaning.allValidated
          ? 'Générer le GPX nettoyé et remplacer l\'original'
          : 'Tous les cas doivent être validés avant la finalisation'
      "
      :loading="finalizing"
      @click="emit('finalize')"
    >
      Finaliser
    </v-btn>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre d'outils de la vue de nettoyage : retour, nom de la trace, progression
 * de la validation, tolérance de cap (paramètre `Nettoyage.Cap.toleranceDeg`)
 * et actions Enregistrer / Réinitialiser / Finaliser.
 *
 * La **finalisation** (remplacement du GPX original) n'est possible que quand
 * l'utilisateur a validé **tous** les cas (bouton désactivé sinon).
 */
import { ref, computed } from 'vue'
import { useCleaningStore } from '../../stores/cleaning'
import { useTracesStore } from '../../stores/traces'

const cleaning = useCleaningStore()
const tracesStore = useTracesStore()

const props = withDefaults(
  defineProps<{
    /** État de chargement du bouton Enregistrer (piloté par la vue). */
    saving?: boolean
    /** État de chargement du bouton Finaliser (piloté par la vue). */
    finalizing?: boolean
  }>(),
  { saving: false, finalizing: false },
)
void props // exposé au template par nom (saving / finalizing)

const emit = defineEmits<{
  (e: 'back'): void
  (e: 'save'): void
  (e: 'reset'): void
  (e: 'finalize'): void
}>()

/** Nom de la trace sélectionnée (pour le titre). */
const traceName = computed(() => {
  const t = tracesStore.traces.find(t => t.id === cleaning.selectedTraceId)
  return t ? t.name : '…'
})

const totalCases = computed(() => cleaning.state?.cases.length ?? 0)
const currentIndex = computed(() => cleaning.currentCaseIndex)
const validatedCount = computed(() => cleaning.validatedCount)

/** Saisie locale de la tolérance (appliquée au change). */
const toleranceInput = ref(String(cleaning.toleranceDeg))

function onToleranceChange() {
  const v = parseFloat(toleranceInput.value)
  if (Number.isNaN(v) || v <= 0) return
  cleaning.toleranceDeg = v
  void cleaning.reDetect()
}
</script>
