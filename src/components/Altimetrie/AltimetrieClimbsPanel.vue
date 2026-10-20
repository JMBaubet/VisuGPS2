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

      <div ref="listRef" class="alt-panel__list">
        <template v-if="climbs.length > 0">
          <AltimetrieClimbCard
            v-for="climb in climbs"
            :key="climb.id"
            :data-climb-id="climb.id"
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
import { nextTick, ref, watch } from 'vue'
import AltimetrieClimbCard from './AltimetrieClimbCard.vue'
import type { AltimetrieClimb, AltimetrieProfilePoint } from '../../stores/altimetrie'

const props = defineProps<{
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

const listRef = ref<HTMLDivElement | null>(null)

/**
 * Fait défiler la liste jusqu'à la carte sélectionnée.
 *
 * Appelé quand la sélection vient du graphe ou de la carte : la carte peut être
 * hors de la zone visible si la liste déborde. Le défilement est limité à la
 * liste, pour ne pas déplacer l'en-tête ni le pied du panneau.
 */
async function revealSelected(id: number | null): Promise<void> {
  if (id === null) return

  await nextTick()
  const list = listRef.value
  if (!list) return

  const card = list.querySelector<HTMLElement>(
    `[data-climb-id="${id}"]`,
  )
  if (!card) return

  const listRect = list.getBoundingClientRect()
  const cardRect = card.getBoundingClientRect()
  const cardTop = cardRect.top - listRect.top + list.scrollTop
  const cardBottom = cardTop + cardRect.height
  const viewTop = list.scrollTop
  const viewBottom = viewTop + list.clientHeight
  const margin = 8

  if (cardTop < viewTop) {
    list.scrollTo({ top: Math.max(0, cardTop - margin), behavior: 'smooth' })
  } else if (cardBottom > viewBottom) {
    list.scrollTo({
      top: cardBottom - list.clientHeight + margin,
      behavior: 'smooth',
    })
  }
}

watch(
  () => props.selectedId,
  id => {
    void revealSelected(id)
  },
  { immediate: true, flush: 'post' },
)
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

/* Les cartes gardent leur hauteur naturelle : la liste déborde et défile
   au lieu de comprimer les cartes pour les faire tenir. */
.alt-panel__list > * {
  flex: 0 0 auto;
}

/* Le défilement appartient à la liste : l'en-tête et le pied restent fixes. */
.alt-panel :deep(.v-navigation-drawer__content) {
  overflow: hidden;
}

.alt-panel__footer {
  flex: 0 0 auto;
  padding: 12px;
}
</style>
