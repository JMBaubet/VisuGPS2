<template>
  <div class="points-table-panel">
    <div class="px-2 pt-2 pb-1 d-flex align-center">
      <span class="text-subtitle-2 font-weight-medium">
        Points du segment ({{ zone.length }})
      </span>
      <v-spacer />
      <span class="text-caption text-medium-emphasis">
        {{ deletedCount }} supprimé(s)
      </span>
    </div>

    <!-- Aide : pour les cas manuels, les points se déplacent directement sur la carte -->
    <div
      v-if="current?.kind === 'manual'"
      class="move-hint px-2 pb-1"
    >
      <v-icon size="x-small" icon="mdi-cursor-move" class="mr-1" color="green" />
      <span class="text-caption">
        Déplacement : cliquez-glissez les points directement sur la carte.
      </span>
    </div>

    <div class="points-scroll">
      <table class="points-table">
        <thead>
          <tr>
            <th>#</th>
            <th>
              <div class="d-flex flex-column align-center">
                <span class="text-caption">Suppr.</span>
                <v-checkbox
                  :model-value="allDeleted"
                  :indeterminate="someDeleted && !allDeleted"
                  density="compact"
                  hide-details
                  title="Cocher = supprimer tous les points du segment, décocher = tout remettre"
                  @update:model-value="v => cleaning.setZoneDeleted(v === true)"
                />
              </div>
            </th>
            <th>Déplacé</th>
          </tr>
        </thead>
        <tbody>
          <template v-if="current">
            <tr
              v-for="(_, i) in zone"
              :key="current.start_index + i"
              :class="{
                'row-deleted': cleaning.isDeleted(current.start_index + i),
                'row-apex': current.apex_indices.includes(current.start_index + i),
              }"
            >
              <td>{{ current.start_index + i + 1 }}</td>
              <td>
                <v-checkbox
                  :model-value="cleaning.isDeleted(current.start_index + i)"
                  density="compact"
                  hide-details
                  @update:model-value="cleaning.toggleDeletePoint(current.start_index + i)"
                />
              </td>
              <td>
                <!-- Indicateur « Déplacé » (cliquable pour annuler) -->
                <v-btn
                  v-if="cleaning.isMoved(current.start_index + i)"
                  size="x-small"
                  variant="text"
                  color="green"
                  prepend-icon="mdi-check"
                  title="Point déplacé — cliquer pour annuler le déplacement"
                  @click="cleaning.clearMovedPoint(current.start_index + i)"
                >
                  Déplacé
                </v-btn>
              </td>
            </tr>
          </template>
          <tr v-else>
            <td colspan="3" class="text-center text-medium-emphasis text-body-2">
              Aucun segment.
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Tableau simplifié des points de la zone du cas courant : numéro, case à
 * cocher de suppression et indicateur « Déplacé » (cliquable pour annuler).
 * Le déplacement lui-même se fait **directement sur la carte** (cas manuels).
 */
import { computed } from 'vue'
import { useCleaningStore } from '../../stores/cleaning'

const cleaning = useCleaningStore()

const current = computed(() => cleaning.currentCase)

/** Points de la zone du cas courant (avec leur index original implicite). */
const zone = computed(() => cleaning.currentZone)

/** Nombre de points marqués à supprimer dans le segment courant. */
const deletedCount = computed(() => {
  const c = cleaning.currentCase
  if (!c) return 0
  return c.correction.delete_ranges.reduce((acc, [from, to]) => acc + (to - from + 1), 0)
})

/** Tous les points du segment sont marqués à supprimer (case d'en-tête). */
const allDeleted = computed(() => cleaning.isZoneFullyDeleted())

/** Au moins un point du segment est marqué à supprimer (état intermédiaire). */
const someDeleted = computed(() => deletedCount.value > 0)
</script>

<style scoped>
.points-table-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.points-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.points-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}

.points-table th,
.points-table td {
  border-bottom: 0.5px solid rgba(127, 127, 127, 0.25);
  padding: 1px 6px;
  text-align: right;
  white-space: nowrap;
}

.points-table th {
  position: sticky;
  top: 0;
  background: rgb(var(--v-theme-surface));
  text-align: center;
  font-weight: 500;
}

.points-table td:first-child {
  text-align: center;
  font-weight: 500;
}

/* Colonne « Suppr. » : cases à cocher centrées. */
.points-table td:nth-child(2) {
  text-align: center;
}

/* Aligne parfaitement les cases à cocher de l'en-tête et des lignes :
   mêmes marges/paddings → mêmes centres horizontaux. */
.points-table :deep(.v-checkbox) {
  margin: 0;
  padding: 0;
}

.points-table td:last-child {
  text-align: center;
}

.row-deleted {
  opacity: 0.4;
  text-decoration: line-through;
}

.row-apex {
  background: color-mix(in srgb, #e53935 14%, transparent);
}

/* Aide au déplacement (cas manuels). */
.move-hint {
  color: #2e7d32;
  display: flex;
  align-items: center;
}
</style>
