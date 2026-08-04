<template>
  <v-card flat border class="override-list ma-2">
    <v-card-title class="text-subtitle-1 py-2 d-flex align-center">
      <v-icon icon="mdi-format-list-bulleted" class="mr-2" />
      Séquences
      <v-spacer />
      <v-btn
        size="small" variant="tonal" color="primary"
        prepend-icon="mdi-plus"
        :disabled="!keyframesStore.hasRawKeyframes"
        @click="$emit('create')"
      >
        Nouvelle
      </v-btn>
    </v-card-title>

    <v-divider />

    <!-- État vide -->
    <v-card-text v-if="overrides.length === 0" class="text-center text-medium-emphasis py-6">
      <v-icon icon="mdi-camera-off" size="32" class="mb-2" />
      <div class="text-body-2">Aucune séquence d'override.</div>
      <div class="text-caption">Créez-en une pour modifier le rendu caméra sur une plage.</div>
    </v-card-text>

    <!-- Liste des overrides -->
    <v-list v-else density="compact" nav>
      <v-list-item
        v-for="ov in overrides"
        :key="ov.id"
        :class="['override-item', { 'override-item--active': ov.id === selectedId }]"
        rounded="lg"
        @click="$emit('select', ov)"
      >
        <template #prepend>
          <!-- Bouton activer/désactiver (toggle) -->
          <v-btn
            :icon="ov.disabled ? 'mdi-toggle-switch-off-outline' : 'mdi-toggle-switch-outline'"
            :color="ov.disabled ? '' : 'success'"
            variant="text"
            size="small"
            title="Activer / désactiver"
            @click.stop="keyframesStore.toggleOverride(ov.id)"
          />
        </template>

        <v-list-item-title class="text-body-2 font-weight-medium">
          {{ ov.name }}
        </v-list-item-title>
        <v-list-item-subtitle class="text-caption">
          {{ formatRange(ov) }} · {{ formatParams(ov) }}
        </v-list-item-subtitle>

        <template #append>
          <v-btn
            icon="mdi-pencil-outline" variant="text" size="small"
            title="Éditer"
            @click.stop="$emit('edit', ov)"
          />
          <v-btn
            icon="mdi-delete-outline" variant="text" size="small"
            color="red-darken-3" title="Supprimer"
            @click.stop="confirmDelete(ov)"
          />
        </template>
      </v-list-item>
    </v-list>

    <!-- Dialogue de confirmation de suppression -->
    <v-dialog v-model="deleteDialog" max-width="380">
      <v-card>
        <v-card-title class="text-h6 text-red-darken-3">
          <v-icon icon="mdi-delete" class="mr-2" />
          Supprimer la séquence ?
        </v-card-title>
        <v-card-text>
          « {{ pendingDelete?.name }} » sera définitivement supprimée.
          Le rendu caméra reprendra la trajectoire automatique sur cette plage.
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="deleteDialog = false">Annuler</v-btn>
          <v-btn color="red-darken-3" variant="tonal" @click="doDelete">Supprimer</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Liste latérale des overrides de montage (§10.4).
 *
 * Pour chaque override :
 *  - un toggle d'activation (comparaison de rendu) ;
 *  - le nom + plage (mm:ss) + résumé des paramètres ;
 *  - un bouton Éditer (recharge l'OverridePanel et place le curseur) ;
 *  - un bouton Supprimer (avec confirmation).
 *
 * Les actions de mutation passent par le store `keyframes` (qui persiste et
 * déclenche la refusion via `useLivePreview`). Les actions d'UI (sélection,
 * édition, création) sont émises vers la vue parent.
 */
import { ref, computed } from 'vue'
import { useKeyframesStore } from '../../stores/keyframes'
import { formatDuration } from '../../utils/format'
import type { Override } from '../../utils/keyframes'

defineProps<{
  /** id de l'override actuellement sélectionné (pour le surlignage), ou null. */
  selectedId?: string | null
}>()

defineEmits<{
  (e: 'create'): void
  (e: 'select', override: Override): void
  (e: 'edit', override: Override): void
}>()

const keyframesStore = useKeyframesStore()

/** Overrides triés par début de plage (tous, y compris désactivés). */
const overrides = computed(() =>
  [...keyframesStore.overrides.overrides].sort((a, b) => a.start_time - b.start_time),
)

// --- Suppression avec confirmation ---

const deleteDialog = ref(false)
const pendingDelete = ref<Override | null>(null)

function confirmDelete(ov: Override) {
  pendingDelete.value = ov
  deleteDialog.value = true
}

async function doDelete() {
  if (pendingDelete.value) {
    await keyframesStore.removeOverride(pendingDelete.value.id)
  }
  deleteDialog.value = false
  pendingDelete.value = null
}

// --- Formatage ---

/** Plage formatée "mm:ss → mm:ss". */
function formatRange(ov: Override): string {
  return `${formatDuration(ov.start_time / 1000)} → ${formatDuration(ov.end_time / 1000)}`
}

/** Résumé compact des paramètres modifiés (ex: "zoom 16.5, pitch 70"). */
function formatParams(ov: Override): string {
  const parts: string[] = []
  const p = ov.params
  if (p.zoom !== undefined) parts.push(`zoom ${p.zoom}`)
  if (p.zoom_offset !== undefined) parts.push(`Δzoom ${p.zoom_offset >= 0 ? '+' : ''}${p.zoom_offset}`)
  if (p.pitch !== undefined) parts.push(`pitch ${p.pitch}°`)
  if (p.pitch_offset !== undefined) parts.push(`Δpitch ${p.pitch_offset >= 0 ? '+' : ''}${p.pitch_offset}°`)
  if (p.bearing !== undefined) parts.push(`bearing ${p.bearing}°`)
  if (p.bearing_offset !== undefined) parts.push(`Δbearing ${p.bearing_offset >= 0 ? '+' : ''}${p.bearing_offset}°`)
  if (p.speed_multiplier !== undefined) parts.push(`×${p.speed_multiplier}`)
  return parts.length > 0 ? parts.join(', ') : 'aucun'
}
</script>

<style scoped>
.override-item {
  transition: background-color 0.15s ease;
}

.override-item--active {
  background-color: color-mix(in srgb, rgb(var(--v-theme-primary)) 14%, transparent);
}
</style>
