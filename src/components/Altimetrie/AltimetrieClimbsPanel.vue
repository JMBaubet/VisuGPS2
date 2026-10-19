<template>
  <v-navigation-drawer permanent width="360" class="alt-panel">
    <div class="alt-panel__body">
      <div class="alt-panel__header">
        <v-icon icon="mdi-terrain" size="18" class="mr-2" />
        <span class="alt-panel__title">Montées détectées</span>
        <v-chip size="x-small" variant="tonal" color="primary" class="ml-auto">
          {{ climbs.length }}
        </v-chip>
      </div>

      <v-divider />

      <div class="alt-panel__list">
        <template v-if="climbs.length > 0">
          <AltimetrieClimbCard
            v-for="climb in climbs"
            :key="climb.id"
            :climb="climb"
            :profile="profile"
            :alt-min="altMin"
            :alt-max="altMax"
            :total-distance-m="totalDistanceM"
            :selected="climb.id === selectedId"
            @click="emit('select', climb.id)"
            @delete="emit('delete', climb.id)"
            @split="emit('split', climb.id)"
            @merge="emit('merge', climb.id)"
          />
        </template>

        <v-empty-state
          v-else
          icon="mdi-chart-bell-curve"
          title="Aucune montée détectée"
          text="La trace ne présente pas de montée selon les paramètres actuels."
        />
      </div>

      <v-divider />

      <div class="alt-panel__footer">
        <v-btn
          block
          color="primary"
          variant="tonal"
          prepend-icon="mdi-plus"
          :disabled="creating"
          @click="emit('create')"
        >
          Créer une montée
        </v-btn>
      </div>
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Panneau gauche de la vue Altimétrie : liste des montées détectées, avec leur
 * position sur la trace complète et leurs métriques. Une carte sélectionnée est
 * synchronisée avec la carte centrale et le widget de profil.
 */
import AltimetrieClimbCard from './AltimetrieClimbCard.vue'
import type { AltimetrieClimb, AltimetrieProfilePoint } from '../../stores/altimetrie'

defineProps<{
  climbs: AltimetrieClimb[]
  profile: AltimetrieProfilePoint[]
  altMin: number
  altMax: number
  totalDistanceM: number
  selectedId: number | null
  /** Une création manuelle est en cours : le bouton est verrouillé. */
  creating: boolean
}>()

const emit = defineEmits<{
  select: [id: number]
  delete: [id: number]
  split: [id: number]
  merge: [id: number]
  create: []
}>()
</script>

<style scoped>
.alt-panel__body {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.alt-panel__header {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  flex: 0 0 auto;
}

.alt-panel__title {
  font-size: 0.9rem;
  font-weight: 600;
}

.alt-panel__list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
}

.alt-panel__footer {
  flex: 0 0 auto;
  padding: 12px;
}
</style>
