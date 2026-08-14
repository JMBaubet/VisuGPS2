<template>
  <div
    v-if="editionStore.showHeadingChangesPanel && editionStore.hasKeyframes"
    class="heading-changes-panel"
  >
    <!-- En-tête : titre + compteur + fermer -->
    <div class="hcp-header">
      <span class="hcp-title">Changements de cap brutaux</span>
      <span class="hcp-count">{{ editionStore.brutalHeadingChanges.length }}</span>
      <v-spacer />
      <v-btn
        icon
        size="x-small"
        variant="text"
        title="Fermer"
        @click="editionStore.toggleHeadingChangesPanel()"
      >
        <v-icon>mdi-close</v-icon>
      </v-btn>
    </div>

    <!-- Légende : teinte = sens (l'épaisseur des bandes = intensité, sur la timeline) -->
    <div class="hcp-legend">
      <span><i class="dot dot-teal" /> horaire (↻)</span>
      <span><i class="dot dot-purple" /> anti-horaire (↺)</span>
      <span class="hcp-threshold">seuil {{ editionStore.headingChangeThresholdDegPerKm }} °/km</span>
    </div>

    <!-- Tableau des virages brutaux (clic → seek + sélection) -->
    <div ref="tableWrapEl" class="hcp-table-wrap">
      <v-table density="compact" class="hcp-table">
        <thead>
          <tr>
            <th class="ind"></th>
            <th class="num">Départ km</th>
            <th class="num">Arrivée km</th>
            <th class="num">Δ dist km</th>
            <th class="num">Δ cap</th>
            <th class="num">Taux °/km</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="h in editionStore.brutalHeadingChanges"
            :key="h.fromDistanceM"
            class="hcp-row"
            :data-active="h.fromDistanceM === activeDistanceM ? 'true' : undefined"
            :style="{ color: senseColor(h.direction) }"
            @click="goTo(h)"
          >
            <!-- Chevron jaune : le curseur d'avance est sur ce segment -->
            <td class="ind">
              <v-icon
                v-if="h.fromDistanceM === activeDistanceM"
                size="small"
                color="#FFD600"
                title="Curseur d'avance sur ce segment"
              >
                mdi-chevron-right
              </v-icon>
            </td>
            <td class="num">{{ (h.fromDistanceM / 1000).toFixed(2) }}</td>
            <td class="num">{{ (h.toDistanceM / 1000).toFixed(2) }}</td>
            <td class="num">{{ (h.distM / 1000).toFixed(2) }}</td>
            <td class="num">{{ fmtDelta(h.deltaDeg) }}</td>
            <td class="num">{{ h.rateDegPerKm.toFixed(0) }}</td>
          </tr>
        </tbody>
      </v-table>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Panneau « Changements de cap brutaux » (affiché à la demande).
 *
 * Tableau des virages de cap entre keyframes consécutifs dont le taux de
 * rotation dépasse le seuil (`brutalHeadingChanges` du store édition). Clic sur
 * une ligne → seek au début du segment + sélection du keyframe de départ.
 *
 * Un **chevron jaune** marque la ligne du segment sous le **curseur d'avance**
 * (lecture ou seek) ; le tableau se scrolle automatiquement pour toujours le
 * garder visible. Conventions visuelles : teinte = sens (teal horaire /
 * deep-purple anti-horaire), comme sur la timeline.
 */
import { ref, computed, watch, nextTick } from 'vue'
import { useEditionStore } from '../../stores/edition'
import {
  ROTATION_BASE_COLORS,
  type HeadingChange,
  type RotationDirection,
} from '../../algorithms/headingChanges'

const editionStore = useEditionStore()

/** Conteneur scrollable du tableau (pour garder le chevron visible). */
const tableWrapEl = ref<HTMLDivElement | null>(null)

/**
 * Distance (m) du début du segment actuellement sous le curseur d'avance, ou
 * `null` si le curseur n'est sur aucun des segments brutaux listés.
 */
const activeDistanceM = computed(() => {
  const cur = editionStore.currentDistanceM
  return (
    editionStore.brutalHeadingChanges.find(
      h => cur >= h.fromDistanceM && cur < h.toDistanceM,
    )?.fromDistanceM ?? null
  )
})

/**
 * Garde le chevron visible et **centré verticalement** dans la zone visible du
 * tableau : si la ligne active sort de la vue (par le haut ou le bas), on
 * défile pour la centrer ; si elle est déjà entièrement visible, on ne touche
 * pas au scroll (l'utilisateur peut lire librement).
 */
function scrollActiveIntoView() {
  const wrap = tableWrapEl.value
  if (!wrap) return
  const row = wrap.querySelector<HTMLElement>('tr[data-active="true"]')
  if (!row) return
  const wrapRect = wrap.getBoundingClientRect()
  const rowRect = row.getBoundingClientRect()
  // Déjà entièrement visible → rien à faire.
  if (rowRect.top >= wrapRect.top && rowRect.bottom <= wrapRect.bottom) return
  // Sort de la vue → la centrer dans la zone visible.
  const targetTop = wrapRect.top + wrap.clientHeight / 2 - rowRect.height / 2
  wrap.scrollTop += rowRect.top - targetTop
}

// Garder le chevron visible quand le curseur passe sur un autre segment.
watch(activeDistanceM, () => nextTick(scrollActiveIntoView))

// Re-scroller à l'ouverture du panneau (le v-if recrée le DOM).
watch(
  () => editionStore.showHeadingChangesPanel,
  visible => {
    if (visible) nextTick(scrollActiveIntoView)
  },
)

/** Delta de cap signé (« +87° » / « -92° »). */
function fmtDelta(deltaDeg: number): string {
  const sign = deltaDeg > 0 ? '+' : ''
  return `${sign}${deltaDeg.toFixed(0)}°`
}

/** Teinte du sens de rotation (teal / deep-purple). */
function senseColor(direction: RotationDirection): string {
  return ROTATION_BASE_COLORS[direction]
}

/** Seek au début du virage + sélection du keyframe de départ. */
function goTo(h: HeadingChange) {
  editionStore.seekToDistance(h.fromDistanceM)
  editionStore.selectKeyframe(h.fromDistanceM)
}
</script>

<style scoped>
.heading-changes-panel {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 6;
  width: 480px;
  max-width: calc(100% - 24px);
  background: rgba(0, 0, 0, 0.8);
  color: #ffffff;
  border-radius: 6px;
  padding: 8px 10px 6px;
  font-family: monospace;
  pointer-events: auto;
  user-select: none;
}

.hcp-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.hcp-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  opacity: 0.7;
}

.hcp-count {
  font-size: 11px;
  font-weight: 700;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  padding: 0 6px;
}

.hcp-legend {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 11px;
  opacity: 0.85;
  margin: 6px 0 4px;
}

.hcp-threshold {
  margin-left: auto;
  opacity: 0.55;
}

.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 4px;
}

.dot-teal {
  background: #009688;
}

.dot-purple {
  background: #673ab7;
}

.hcp-table-wrap {
  max-height: 50vh;
  overflow-y: auto;
}

.hcp-table {
  background: transparent;
  color: #ffffff;
  font-size: 12px;
  line-height: 1.4;
}

.hcp-table :deep(th) {
  color: rgba(255, 255, 255, 0.6);
  font-weight: 600;
  border-bottom: 1px solid rgba(255, 255, 255, 0.15);
}

/* La couleur du texte des cellules est héritée du `tr` (teinte = sens). */
.hcp-table :deep(td) {
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  cursor: pointer;
}

.hcp-row:hover :deep(td) {
  background: rgba(255, 255, 255, 0.08);
}

.num {
  text-align: right;
}

/* Colonne indicateur (chevron jaune du segment courant). */
.ind {
  width: 18px;
  padding: 0;
  text-align: center;
  vertical-align: middle;
}

/* Le chevron est centré verticalement dans la cellule (la fonte Material Icons
   positionne le glyphe bas dans la boîte de ligne sans cette correction). */
.ind :deep(.v-icon) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  vertical-align: middle;
  line-height: 1;
}
</style>
