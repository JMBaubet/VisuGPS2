<template>
  <v-navigation-drawer permanent width="360" class="alt-panel">
    <div class="alt-panel__body">
      <v-tabs
        v-model="tab"
        density="compact"
        color="primary"
        class="alt-panel__tabs"
        show-arrows="false"
      >
        <v-tab value="tunnels" prepend-icon="mdi-tunnel">
          Tunnels
          <v-chip size="x-small" variant="tonal" class="ml-2">
            {{ tunnelCount }}
          </v-chip>
        </v-tab>
        <v-tab value="climbs" prepend-icon="mdi-terrain">
          Montées
          <v-chip size="x-small" variant="tonal" class="ml-2">
            {{ climbs.length }}
          </v-chip>
        </v-tab>
      </v-tabs>

      <v-divider />

      <div ref="listRef" class="alt-panel__list">
        <template v-if="activeTab === 'climbs'">
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
        </template>

        <template v-else>
          <AltimetrieOsmPanel
            :requirements="tunnelRequirements"
            :progress="tunnelProgress"
            :pending="tunnelPending"
            :preparing="tunnelPreparing"
            :has-file="tunnelHasFile"
            :error="tunnelError"
            @force-download="emit('force-download-tunnels')"
          />

          <template v-if="tunnels.length > 0">
            <AltimetrieTunnelCard
              v-for="(group, index) in tunnels"
              :key="group.id"
              :data-tunnel-id="group.id"
              :group="group"
              :index="index"
              :profile="profile"
              :alt-min="altMin"
              :alt-max="altMax"
              :total-distance-m="totalDistanceM"
              :selected="selectedTunnelIds.includes(group.id)"
              :can-merge="canMerge(group)"
              :previous-gap-m="previousGapM(group)"
              @click="emit('select-tunnel', group.id)"
              @delete="emit('delete-tunnel', group.id)"
              @merge="emit('merge-tunnel', group.id)"
              @split="emit('split-tunnel', group.id)"
            />
          </template>

          <v-empty-state
            v-else-if="tunnelPreparing || tunnelPending"
            icon="mdi-cloud-download-outline"
            title="Préparation des tunnels"
            :text="
              tunnelPreparing
                ? 'Téléchargement et analyse des données OSM en cours…'
                : 'En attente dans la file de préparation…'
            "
          />

          <v-empty-state
            v-else
            icon="mdi-tunnel-outline"
            title="Aucun tunnel"
            text="Aucun tunnel détecté. Créez-en un manuellement ou relancez la détection."
          />
        </template>
      </div>

      <v-divider />

      <div class="alt-panel__footer">
        <template v-if="activeTab === 'climbs'">
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
        </template>

        <template v-else>
          <v-btn
            block
            color="primary"
            variant="tonal"
            prepend-icon="mdi-plus"
            :disabled="
              tunnelCreating ||
              tunnelPending ||
              tunnelLoading ||
              !canCreateTunnel
            "
            :title="
              canCreateTunnel
                ? 'Créer un tunnel'
                : 'Création manuelle disponible avec le profil de tunnels'
            "
            @click="emit('create-tunnel')"
          >
            Créer un tunnel
          </v-btn>
          <v-btn
            block
            variant="text"
            prepend-icon="mdi-refresh"
            :disabled="tunnelPending || tunnelCreating || tunnelLoading"
            @click="emit('relaunch-tunnels')"
          >
            Relancer la détection
          </v-btn>
        </template>
      </div>
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Panneau gauche de la vue Altimétrie : deux onglets, **Tunnels** et
 * **Montées**. Chaque onglet garde le même fonctionnement : position sur la
 * trace complète, métriques, sélection synchronisée avec la carte et le widget.
 *
 * L'onglet Tunnels porte en tête le panneau « Données OSM » (PBF chargés, en
 * cours de téléchargement, manquants) et les actions de fusion/séparation.
 */
import { computed, nextTick, ref, watch } from 'vue'
import AltimetrieClimbCard from './AltimetrieClimbCard.vue'
import AltimetrieTunnelCard from './AltimetrieTunnelCard.vue'
import AltimetrieOsmPanel from './AltimetrieOsmPanel.vue'
import type { AltimetrieClimb, AltimetrieProfilePoint } from '../../stores/altimetrie'
import type {
  TunnelGroup,
  TunnelProgress,
  TunnelRequirements,
} from '../../stores/tunnels'

const props = defineProps<{
  activeTab: 'climbs' | 'tunnels'
  climbs: AltimetrieClimb[]
  profile: AltimetrieProfilePoint[]
  altMin: number
  altMax: number
  totalDistanceM: number
  selectedId: number | null
  /** Une création manuelle de montée est en cours : le bouton est verrouillé. */
  creating: boolean
  // ── Tunnels ────────────────────────────────────────────────────────
  tunnels: TunnelGroup[]
  selectedTunnelIds: string[]
  tunnelCount: number
  /** Une création manuelle de tunnel est en cours. */
  tunnelCreating: boolean
  /** Une préparation est en cours ou en file pour cette trace. */
  tunnelPending: boolean
  /** Le fichier de tunnels est en cours de chargement. */
  tunnelLoading: boolean
  /** La trace est la préparation en cours. */
  tunnelPreparing: boolean
  tunnelProgress: TunnelProgress | null
  tunnelRequirements: TunnelRequirements | null
  tunnelHasFile: boolean
  /** Le widget de profil sait créer un tunnel (étape suivante). */
  canCreateTunnel: boolean
  mergeGapM: number
  tunnelError: string | null
}>()

const emit = defineEmits<{
  'update:activeTab': [tab: 'climbs' | 'tunnels']
  select: [id: number]
  delete: [id: number]
  split: [id: number]
  merge: [id: number]
  create: []
  'select-tunnel': [id: string]
  'delete-tunnel': [id: string]
  'merge-tunnel': [id: string]
  'split-tunnel': [id: string]
  'create-tunnel': []
  'relaunch-tunnels': []
  'force-download-tunnels': []
}>()

const listRef = ref<HTMLDivElement | null>(null)

/** v-model des onglets : la valeur appartient à la vue, pas au panneau. */
const tab = computed({
  get: () => props.activeTab,
  set: (value: string) => {
    emit('update:activeTab', value === 'tunnels' ? 'tunnels' : 'climbs')
  },
})

/** Écart en mètres avec le groupe précédent, ou `null` pour le premier. */
function previousGapM(group: TunnelGroup): number | null {
  const index = props.tunnels.findIndex(candidate => candidate.id === group.id)
  if (index <= 0) return null
  return (group.startKm - props.tunnels[index - 1].endKm) * 1000
}

/** Fusion proposée seulement si l'écart est positif et sous le réglage. */
function canMerge(group: TunnelGroup): boolean {
  const gap = previousGapM(group)
  return gap !== null && gap >= 0 && gap <= props.mergeGapM
}

async function revealCard(selector: string): Promise<void> {
  await nextTick()
  const list = listRef.value
  if (!list) return

  const card = list.querySelector<HTMLElement>(selector)
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

async function revealSelected(id: number | null): Promise<void> {
  if (id === null) return
  await revealCard(`[data-climb-id="${id}"]`)
}

async function revealSelectedTunnel(ids: string[]): Promise<void> {
  const id = ids[0]
  if (!id) return
  await revealCard(`[data-tunnel-id="${id}"]`)
}

watch(
  () => props.selectedId,
  id => {
    if (props.activeTab === 'climbs') void revealSelected(id)
  },
  { immediate: true, flush: 'post' },
)

watch(
  () => props.selectedTunnelIds.join('|'),
  () => {
    if (props.activeTab === 'tunnels') {
      void revealSelectedTunnel(props.selectedTunnelIds)
    }
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

.alt-panel__tabs {
  flex: 0 0 auto;
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

/* Le défilement appartient à la liste : les onglets et le pied restent fixes. */
.alt-panel :deep(.v-navigation-drawer__content) {
  overflow: hidden;
}

.alt-panel__footer {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 0 0 auto;
  padding: 12px;
}
</style>
