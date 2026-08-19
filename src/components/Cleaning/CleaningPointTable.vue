<template>
  <div class="points-table-panel">
    <div class="px-2 pt-2 pb-1 d-flex align-center">
      <span class="text-subtitle-2 font-weight-medium">
        Points du segment ({{ items.length }})
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
      <v-data-table
        :items="items"
        :headers="headers"
        :items-per-page="-1"
        :item-class="rowClass"
        hide-default-footer
        hover
        density="compact"
        class="points-datatable"
      >
        <!-- Colonne « Suppr. » : libellé à gauche de la case d'en-tête
             (tout supprimer / tout remettre). La case est seule dans un flex
             centré — structure strictement identique aux lignes — donc centrée
             exactement comme elles ; le libellé est hors du flux (position
             absolue) pour ne jamais décaler la case. -->
        <template #header.select>
          <div class="header-select">
            <span class="header-select__label text-caption">Suppr.</span>
            <div class="d-flex align-center justify-center">
              <v-checkbox
                :model-value="allDeleted"
                :indeterminate="someDeleted && !allDeleted"
                density="compact"
                hide-details
                title="Cocher = supprimer tous les points du segment, décocher = tout remettre"
                @update:model-value="v => cleaning.setZoneDeleted(v === true)"
              />
            </div>
          </div>
        </template>

        <!-- Case à cocher de suppression par point. Enveloppée dans le même
             conteneur flex que l'en-tête : sans lui, le `.v-input` de la case
             s'étire sur toute la cellule et la case reste alignée à gauche. -->
        <template #item.select="{ internalItem }">
          <div class="d-flex align-center justify-center">
            <v-checkbox
              :model-value="cleaning.isDeleted(internalItem.raw.i)"
              density="compact"
              hide-details
              @update:model-value="cleaning.toggleDeletePoint(internalItem.raw.i)"
            />
          </div>
        </template>

        <!-- Colonne « Déplacé » : indicateur (cliquable pour annuler) -->
        <template #item.deplace="{ internalItem }">
          <v-btn
            v-if="cleaning.isMoved(internalItem.raw.i)"
            size="x-small"
            variant="text"
            color="green"
            prepend-icon="mdi-check"
            title="Point déplacé — cliquer pour annuler le déplacement"
            @click="cleaning.clearMovedPoint(internalItem.raw.i)"
          >
            Déplacé
          </v-btn>
        </template>
      </v-data-table>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Tableau des points de la zone du cas courant (composant Vuetify
 * `v-data-table`) : numéro, **sélection intégrée** (case d'en-tête
 * « tout sélectionner / tout désélectionner ») et indicateur « Déplacé »
 * (cliquable pour annuler). Le déplacement lui-même se fait directement sur
 * la carte (cas manuels).
 */
import { computed } from 'vue'
import { useCleaningStore } from '../../stores/cleaning'

const cleaning = useCleaningStore()

const current = computed(() => cleaning.currentCase)

/** Points de la zone du cas courant (avec leur index original implicite). */
const zone = computed(() => cleaning.currentZone)

/** Lignes du tableau : index original + identifiant affiché. La zone peut être
 * élargie de la marge (ronds-points) — les index se déduisent de `zoneStart`. */
const items = computed(() => {
  const c = cleaning.currentCase
  if (!c) return []
  return zone.value.map((_, idx) => ({
    i: cleaning.zoneStart + idx,
    id: cleaning.zoneStart + idx + 1,
  }))
})

const headers = [
  { title: 'Ident. du point', key: 'id', align: 'center' },
  { title: 'Suppr.', key: 'select', align: 'center' },
  { title: 'Déplacé', key: 'deplace', align: 'center' },
] as const

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

/** Classe de ligne selon l'état du point (supprimé / apex). */
function rowClass(item: unknown): string {
  const raw = (item as { raw?: { i?: number } })?.raw ?? (item as { i?: number })
  const i = raw?.i ?? -1
  if (!Number.isInteger(i) || i < 0) return ''
  if (cleaning.isDeleted(i)) return 'row-deleted'
  if (cleaning.currentCase?.apex_indices.includes(i)) return 'row-apex'
  return ''
}
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

/* Police compacte pour les lignes du tableau. */
.points-datatable :deep(.v-data-table__td) {
  font-size: 12px;
  padding-top: 2px;
  padding-bottom: 2px;
}

/* Aligne parfaitement les cases à cocher de sélection (en-tête et lignes). */
.points-datatable :deep(.v-checkbox) {
  margin: 0;
  padding: 0;
}

/* En-tête « Suppr. » : la case est seule dans un flex centré (comme les
   lignes) → centrage identique. Le libellé est en position absolue, juste à
   gauche de la case (right = 50 % + largeur case), hors du flux : il ne peut
   pas décaler la case, quelle que soit sa largeur. */
.header-select {
  position: relative;
  display: flex;
  justify-content: center;
}

.header-select__label {
  position: absolute;
  right: calc(50% + 20px);
  top: 50%;
  transform: translateY(-50%);
  white-space: nowrap;
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
