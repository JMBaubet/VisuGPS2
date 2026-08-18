<template>
  <div class="cases-panel">
    <!-- En-tête -->
    <div class="px-2 pt-2 pb-1 d-flex align-center">
      <span class="text-subtitle-1 font-weight-medium">Anomalies détectées</span>
      <v-spacer />
      <span class="text-caption text-medium-emphasis">
        {{ cleaning.validatedCount }}/{{ cleaning.state?.cases.length ?? 0 }} validés
      </span>
    </div>
    <v-divider class="mx-2" />

    <!-- Liste des cas -->
    <div class="cases-list">
      <template v-if="cleaning.hasCases">
        <v-list-item
          v-for="(c, idx) in cleaning.state!.cases"
          :key="c.id"
          :active="idx === cleaning.currentCaseIndex"
          class="case-item"
          @click="cleaning.goToCase(idx)"
        >
          <template #prepend>
            <v-icon
              :icon="kindIcon(c.kind)"
              :color="kindColor(c.kind)"
              size="small"
            />
          </template>
          <v-list-item-title class="text-body-2">
            Cas {{ idx + 1 }} — {{ kindLabel(c.kind) }}
          </v-list-item-title>
          <v-list-item-subtitle class="text-caption">
            Points {{ c.start_index + 1 }} → {{ c.end_index + 1 }}
            <template v-if="c.apex_indices.length"> · apex {{ c.apex_indices.map(a => a + 1).join(', ') }}</template>
          </v-list-item-subtitle>
          <template #append>
            <v-chip
              size="x-small"
              :color="stateColor(c.state)"
              variant="tonal"
            >
              {{ stateLabel(c.state) }}
            </v-chip>
          </template>
        </v-list-item>
      </template>
      <div v-else class="pa-4 text-center text-medium-emphasis text-body-2">
        Aucune anomalie détectée avec cette tolérance.
      </div>
    </div>

    <v-divider />

    <!-- Actions du cas courant -->
    <div class="pa-3 current-case-actions">
      <template v-if="current">
        <div class="text-body-2 font-weight-medium mb-2">
          Cas {{ cleaning.currentCaseIndex + 1 }} — {{ kindLabel(current.kind) }}
        </div>

        <!-- Écart de cap mesuré -->
        <div class="text-caption text-medium-emphasis mb-2">
          Écart de cap mesuré : {{ current.bearing_delta_deg.toFixed(1) }}° (seuil
          {{ cleaning.toleranceDeg.toFixed(1) }}°)
        </div>

        <!-- Outils de correction -->
        <div class="d-flex flex-wrap ga-1 mb-2">
          <v-btn
            size="small"
            variant="tonal"
            prepend-icon="mdi-vector-polyline"
            title="Pré-remplir la suppression avec la suggestion de détection"
            @click="cleaning.applySuggestion()"
          >
            Appliquer la suggestion
          </v-btn>
          <v-btn
            size="small"
            variant="tonal"
            prepend-icon="mdi-arrow-expand-all"
            title="Marquer toute la zone du cas comme à supprimer"
            @click="cleaning.addDeleteRange(current.start_index, current.end_index)"
          >
            Tout supprimer
          </v-btn>
          <v-btn
            size="small"
            variant="text"
            prepend-icon="mdi-undo-variant"
            title="Retirer toutes les corrections du cas courant"
            @click="cleaning.clearCorrection()"
          >
            Restaurer
          </v-btn>
        </div>

        <!-- Création manuelle d'un cas -->
        <template v-if="cleaning.createMode">
          <div class="create-help mb-2">
            <v-icon size="small" icon="mdi-draw" class="mr-1" color="green" />
            <span class="text-caption">
              <template v-if="cleaning.createStartIndex === null">
                Cliquez sur la carte pour le <b>point de début</b> du segment,
                puis sur le <b>point de fin</b>.
              </template>
              <template v-else>
                Point de début posé (n° {{ cleaning.createStartIndex + 1 }}) —
                cliquez maintenant sur le <b>point de fin</b>.
              </template>
            </span>
            <v-btn
              size="x-small"
              icon="mdi-close"
              variant="text"
              title="Annuler la création"
              @click="cleaning.cancelCreate()"
            />
          </div>
        </template>
        <v-btn
          v-else
          size="small"
          variant="tonal"
          color="green"
          prepend-icon="mdi-draw"
          title="Créer manuellement un cas : désigner sur la carte un point de début puis un point de fin"
          @click="cleaning.toggleCreateMode()"
        >
          Créer une anomalie
        </v-btn>

        <!-- Suppression d'un cas manuel non validé -->
        <v-btn
          v-if="current.kind === 'manual' && current.state === 'pending'"
          size="small"
          variant="text"
          color="red"
          prepend-icon="mdi-delete-outline"
          class="mt-1"
          title="Supprimer ce cas (création manuelle) avant validation"
          @click="supprimerCasCourant"
        >
          Supprimer ce cas
        </v-btn>

        <div class="text-caption text-medium-emphasis mb-2">
          <template v-if="deletedCount > 0">
            {{ deletedCount }} point(s) marqué(s) à supprimer
          </template>
          <template v-else>
            Aucune modification — cliquez sur les points de la carte ou
            « Appliquer la suggestion ».
          </template>
        </div>

        <!-- Validation manuelle (obligatoire pour passer au cas suivant) -->
        <div class="d-flex ga-2">
          <v-btn
            color="green"
            variant="flat"
            prepend-icon="mdi-check"
            :disabled="current.state === 'corrected'"
            @click="cleaning.validateCurrentCase('corrected')"
          >
            Corriger &amp; valider
          </v-btn>
          <v-btn
            color="blue"
            variant="tonal"
            prepend-icon="mdi-check-decagram-outline"
            :disabled="current.state === 'kept'"
            :title="'Conserver la trace telle quelle sur ce segment (faux positif)'"
            @click="cleaning.validateCurrentCase('kept')"
          >
            Conserver tel quel
          </v-btn>
        </div>
      </template>
      <div v-else class="text-center text-medium-emphasis text-body-2 py-4">
        Chargez un état de nettoyage.
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Panneau latéral de la vue de nettoyage : liste des cas détectés (navigation)
 * et actions de correction + **validation manuelle** du cas courant.
 *
 * La validation est de la responsabilité de l'utilisateur : chaque cas doit
 * être « Corrigé & validé » ou « Conservé tel quel » (faux positif) avant de
 * pouvoir passer au suivant. La finalisation n'est possible qu'une fois tous
 * les cas validés.
 */
import { computed } from 'vue'
import { useCleaningStore, type CleaningCaseKind, type CleaningCaseState } from '../../stores/cleaning'

const cleaning = useCleaningStore()

const current = computed(() => cleaning.currentCase)

const deletedCount = computed(() => {
  const c = cleaning.currentCase
  if (!c) return 0
  return c.correction.delete_ranges.reduce((acc, [from, to]) => acc + (to - from + 1), 0)
})

/** Supprime le cas manuel courant (non validé) de la liste. */
function supprimerCasCourant() {
  const ok = window.confirm(
    'Supprimer ce cas créé manuellement ? Les corrections associées seront perdues.',
  )
  if (ok) cleaning.removeCase(cleaning.currentCaseIndex)
}

function kindIcon(kind: CleaningCaseKind): string {
  switch (kind) {
    case 'spike':
      return 'mdi-dots-hexagon'
    case 'out_and_back':
      return 'mdi-arrow-u-left-bottom'
    case 'manual':
      return 'mdi-draw'
  }
}

function kindColor(kind: CleaningCaseKind): string {
  switch (kind) {
    case 'spike':
      return 'purple'
    case 'out_and_back':
      return 'orange'
    case 'manual':
      return 'green'
  }
}

function kindLabel(kind: CleaningCaseKind): string {
  switch (kind) {
    case 'spike':
      return 'Point hors trace'
    case 'out_and_back':
      return 'Aller-retour'
    case 'manual':
      return 'Manuel (créé à la carte)'
  }
}

function stateColor(state: CleaningCaseState): string {
  switch (state) {
    case 'pending':
      return 'orange'
    case 'corrected':
      return 'green'
    case 'kept':
      return 'blue'
  }
}

function stateLabel(state: CleaningCaseState): string {
  switch (state) {
    case 'pending':
      return 'À traiter'
    case 'corrected':
      return 'Corrigé'
    case 'kept':
      return 'Conservé'
  }
}
</script>

<style scoped>
.cases-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.cases-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.current-case-actions {
  flex: 0 0 auto;
  border-top: 0.5px solid rgba(127, 127, 127, 0.3);
}

/* Consigne du mode « Créer une anomalie ». */
.create-help {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  border-left: 3px solid #00c853;
  background: color-mix(in srgb, #00c853 8%, transparent);
  border-radius: 0 4px 4px 0;
}
</style>
