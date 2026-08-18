<template>
  <div class="points-table-panel">
    <div class="px-2 pt-2 pb-1 d-flex align-center">
      <span class="text-subtitle-2 font-weight-medium">
        Points du segment ({{ zone.length }})
      </span>
      <v-spacer />
      <span class="text-caption text-medium-emphasis">
        {{ cleaning.isDeletedCount }} supprimé(s)
      </span>
    </div>

    <div class="points-scroll">
      <table class="points-table">
        <thead>
          <tr>
            <th>#</th>
            <th>Lat</th>
            <th>Lon</th>
            <th>Ele</th>
            <th>Suppr.</th>
          </tr>
        </thead>
        <tbody>
          <template v-if="current">
            <tr
              v-for="(p, i) in zone"
              :key="current.start_index + i"
              :class="{
                'row-deleted': cleaning.isDeleted(current.start_index + i),
                'row-apex': current.apex_indices.includes(current.start_index + i),
              }"
            >
              <td>{{ current.start_index + i + 1 }}</td>
              <td>{{ p.lat.toFixed(5) }}</td>
              <td>{{ p.lon.toFixed(5) }}</td>
              <td>{{ p.alt?.toFixed(0) ?? '—' }}</td>
              <td>
                <v-checkbox
                  :model-value="cleaning.isDeleted(current.start_index + i)"
                  density="compact"
                  hide-details
                  @update:model-value="cleaning.toggleDeletePoint(current.start_index + i)"
                />
              </td>
            </tr>
          </template>
          <tr v-else>
            <td colspan="5" class="text-center text-medium-emphasis text-body-2">
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
 * Tableau détaillé des points de la zone du cas courant, avec case à cocher de
 * suppression (index GPX). Complète la sélection visuelle sur la carte.
 */
import { computed } from 'vue'
import { useCleaningStore } from '../../stores/cleaning'

const cleaning = useCleaningStore()

const current = computed(() => cleaning.currentCase)

/** Points de la zone du cas courant (avec leur index original implicite). */
const zone = computed(() => cleaning.currentZone)
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
</style>
