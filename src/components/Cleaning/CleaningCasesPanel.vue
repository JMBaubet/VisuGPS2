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
            <!-- Poubelle rouge : supprimer le cas (toujours possible pour les
                 cas « Modification de segment », sinon tant que non validé) -->
            <v-btn
              v-if="c.state === 'pending' || c.kind === 'manual'"
              size="x-small"
              icon="mdi-delete"
              variant="text"
              color="red"
              class="mr-1"
              :title="`Supprimer le cas ${idx + 1}`"
              @click.stop="confirmerSuppressionCas(idx)"
            />
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

      <!-- Modifier un segment : désigner manuellement un segment sur la carte -->
      <div class="pa-3">
        <template v-if="cleaning.createMode">
          <div class="create-help">
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
              title="Annuler la modification"
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
          title="Modifier un segment : désigner sur la carte un point de début puis un point de fin"
          @click="cleaning.toggleCreateMode()"
        >
          Modifier un segment
        </v-btn>
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
            variant="text"
            prepend-icon="mdi-undo-variant"
            title="Retirer toutes les corrections du cas courant"
            @click="cleaning.clearCorrection()"
          >
            Restaurer
          </v-btn>
        </div>

        <!-- Suppression d'un cas (dans la liste, poubelle à gauche de l'état) -->

        <div class="text-caption text-medium-emphasis mb-2">
          <template v-if="deletedCount > 0">
            {{ deletedCount }} point(s) marqué(s) à supprimer
          </template>
          <template v-else>
            Aucune modification — cliquez sur les points de la carte pour les
            supprimer (ou déplacez-les pour les cas de modification de segment).
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
            Valider
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
 * Panneau latéral de la vue de nettoyage : liste des cas (navigation), bouton
 * « Modifier un segment » (désignation manuelle d'un segment sur la carte) et
 * actions de correction + **validation manuelle** du cas courant.
 *
 * La validation est de la responsabilité de l'utilisateur : chaque cas doit
 * être « Validé » ou « Conservé tel quel » (faux positif) avant de pouvoir
 * passer au suivant. La finalisation n'est possible qu'une fois tous les cas
 * validés.
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

/**
 * Confirme puis supprime un cas (poubelle de la liste). Les cas « Modification
 * de segment » sont supprimables même après validation ; les cas détectés
 * seulement tant qu'ils ne sont pas validés (refus géré par le store).
 */
function confirmerSuppressionCas(index: number) {
  const c = cleaning.state?.cases[index]
  if (!c) return
  const ok = window.confirm(
    `Supprimer le cas ${index + 1} (« ${kindLabel(c.kind)} ») ? Les corrections associées seront perdues.`,
  )
  if (ok) cleaning.removeCase(index)
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
      return 'Modification de segment'
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
      return 'Validé'
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

/* Consigne du mode « Modifier un segment ». */
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
