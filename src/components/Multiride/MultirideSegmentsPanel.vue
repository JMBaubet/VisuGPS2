<template>
  <v-navigation-drawer permanent width="360" class="mrl-panel">
    <div class="mrl-panel-body">
      <div class="mrl-panel-list">
        <v-expansion-panels variant="accordion" class="ma-1" v-model="openPanels">
          <v-expansion-panel>
            <v-expansion-panel-title>
              <v-icon icon="mdi-chart-box" class="mr-2" />
              Synthèse
            </v-expansion-panel-title>
            <v-expansion-panel-text>
              <v-list density="compact">
                <v-list-item title="Segments répétés">
                  <template #append>
                    <b>{{ segmentNumbers.length }}</b>
                  </template>
                </v-list-item>
                <v-list-item title="Emprunts">
                  <template #append>{{ passages.length }}</template>
                </v-list-item>
                <v-list-item title="Faux positifs">
                  <template #append>{{ falsePositiveCount }}</template>
                </v-list-item>
                <v-list-item title="Fusions manuelles">
                  <template #append>{{ mergedCount }}</template>
                </v-list-item>
                <v-list-item title="Km répétés">
                  <template #append>{{ repeatedKm.toFixed(2) }} km</template>
                </v-list-item>
                <v-list-item title="Trace">
                  <template #append>{{ traceLengthKm.toFixed(2) }} km</template>
                </v-list-item>
                <v-list-item v-if="analysisDurationMs > 0" title="Temps d'analyse">
                  <template #append>{{ analysisDurationMs }} ms</template>
                </v-list-item>
                <v-list-item title="Paramètres actifs" :subtitle="paramsSummary" />
              </v-list>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>

        <v-divider class="my-1" />

        <v-list v-if="segmentNumbers.length > 0" nav class="py-0">
          <v-list-item
            v-for="segment in segmentNumbers"
            :key="segment"
            :active="segment === selectedSegment"
            class="mrl-segment"
            :class="{ 'mrl-segment--fp': isFalsePositive(segment) }"
            @click="emit('select', segment)"
          >
            <!-- Titre : la longueur et le début de l'emprunt de référence.
                 L'état d'ajustement est porté à droite par un badge ; un
                 segment n'en porte qu'un, la fusion et le faux positif
                 s'excluant. -->
            <v-list-item-title :class="{ 'mrl-strike': isFalsePositive(segment) }">
              {{ titleOf(segment) }}
            </v-list-item-title>

            <template #append>
              <v-chip
                v-if="isFalsePositive(segment)"
                size="x-small"
                variant="tonal"
                color="warning"
              >
                FP
              </v-chip>
              <v-chip
                v-else-if="isMerged(segment)"
                size="x-small"
                variant="tonal"
                color="info"
              >
                fusionné
              </v-chip>
            </template>

            <!-- Ruban multi-rails : un rail par emprunt, positionné sur la
                 trace entière et coloré selon son sens. -->
            <div class="mrl-ribbon">
              <div
                v-for="passage in segmentPassages(segment)"
                :key="passage.passage"
                class="mrl-rail"
              >
                <div
                  class="mrl-rail-bar"
                  :style="{
                    left: railLeft(passage),
                    width: railWidth(passage),
                    background: sensColor(passage.sens),
                    opacity: passage.fauxPositif ? 0.3 : 1,
                  }"
                  :title="railTitle(passage)"
                />
              </div>
            </div>

            <!-- Emprunts répétés — les Aller et les Retour —, par leur seul
                 début : la référence est portée par le titre, la longueur et
                 les bornes de points ne décident de rien. -->
            <div class="mrl-passages">
              <div
                v-for="passage in repeatPassages(segment)"
                :key="passage.passage"
                class="mrl-passage"
              >
                <span class="mrl-passage-sens" :style="{ color: sensColor(passage.sens) }">
                  {{ sensLabel(passage.sens) }}
                </span>
                <span class="mrl-passage-range">{{ formatStart(passage.kmEntree) }}</span>
              </div>
            </div>
          </v-list-item>
        </v-list>

        <v-empty-state
          v-else
          icon="mdi-check-circle"
          title="Aucun passage multiple"
          text="La trace ne repasse sur aucun troncçon."
        />
      </div>
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Panneau latéral des passages multiples : synthèse de la détection, puis un
 * bloc par segment — en-tête, **ruban multi-rails** et emprunts répétés.
 *
 * La restitution d'un segment se limite à ce qui décide : la longueur et le
 * début de son emprunt de **référence** en titre, puis chaque Aller/Retour par
 * son seul début. Les bornes de points et les kilomètres de sortie ne servent
 * pas la décision et ne sont plus affichés ; ils restent dans le fichier, qui
 * est le contrat de sortie du module.
 *
 * Le ruban situe chaque emprunt sur la trace entière : un rail par emprunt,
 * positionné en pourcentage des kilomètres, coloré selon le sens. C'est la
 * lecture que la spécification demande (« mini-ribbon »), et elle rend visible
 * d'un coup d'œil qu'un tronçon est emprunté plusieurs fois loin les uns des
 * autres.
 *
 * La ligne ne porte **aucune action** : la gestion d'un segment — fusion, faux
 * positif, annulation — appartient à la fenêtre d'action, que le clic ouvre
 * (`MultirideActionPanel`), comme dans la vue Audit. La sélection remonte au
 * parent, qui la publie dans le store : la carte s'y synchronise et cadre
 * l'étendue du segment.
 */
import { ref, computed } from 'vue'
import type { MultirideParams, MultiridePassage } from '../../stores/multiride'
import { sensColor, sensLabel } from './multirideMapLayers'
import { formatSegmentTitle, formatStart } from './multirideFormat'

const props = defineProps<{
  /** Emprunts détectés, tous segments confondus, triés. */
  passages: MultiridePassage[]
  /** Numéros des segments détectés, dans l'ordre. */
  segmentNumbers: number[]
  /** Segment mis en avant (`null` : aucun). */
  selectedSegment: number | null
  /** Longueur totale de la trace analysée (km). */
  traceLengthKm: number
  /** Longueur cumulée des portions répétées, faux positifs exclus (km). */
  repeatedKm: number
  /** Durée de la dernière détection jouée (ms). */
  analysisDurationMs: number
  /** Paramètres ayant produit la détection. */
  params: MultirideParams | null
}>()

const emit = defineEmits<{
  select: [segment: number]
}>()

/** Panneaux de synthèse dépliés au premier affichage. */
const openPanels = ref<number[]>([0])

/** Segments distincts marqués faux positifs. */
const falsePositiveCount = computed(
  () => props.segmentNumbers.filter((n) => isFalsePositive(n)).length,
)

/** Segments ayant subi une fusion manuelle. */
const mergedCount = computed(
  () => props.segmentNumbers.filter((n) => isMerged(n)).length,
)

/** Paramètres actifs, tels qu'ils ont produit la détection. */
const paramsSummary = computed(() => {
  const params = props.params
  if (!params) return ''
  return [
    `tolérance ${params.toleranceM} m`,
    `longueur min ${params.longueurMinM} m`,
    `pas ${params.pasEchantillonnageM} m`,
    `fusion ${params.fusionReferencesM} m`,
  ].join(' · ')
})

/** Emprunts d'un segment, triés par numéro d'emprunt. */
function segmentPassages(segment: number): MultiridePassage[] {
  return props.passages
    .filter((p) => p.segment === segment)
    .sort((a, b) => a.passage - b.passage)
}

/**
 * Emprunts **répétés** d'un segment : tout sauf la référence, qui est déjà
 * portée par le titre.
 */
function repeatPassages(segment: number): MultiridePassage[] {
  return segmentPassages(segment).filter((p) => p.sens !== 'reference')
}

/** `true` si le segment a été écarté par l'utilisateur. */
function isFalsePositive(segment: number): boolean {
  return segmentPassages(segment).some((p) => p.fauxPositif)
}

/** `true` si le segment a été réuni à son précédent par une fusion. */
function isMerged(segment: number): boolean {
  return segmentPassages(segment).some((p) => p.fusionne)
}

/** En-tête d'un segment, lu sur son emprunt de référence. */
function titleOf(segment: number): string {
  const reference = segmentPassages(segment).find((p) => p.sens === 'reference') ?? null
  return formatSegmentTitle(segment, reference)
}

/** Position d'un emprunt sur la trace entière, en pourcentage. */
function railLeft(passage: MultiridePassage): string {
  if (props.traceLengthKm <= 0) return '0%'
  return `${Math.max(0, (passage.kmEntree / props.traceLengthKm) * 100).toFixed(3)}%`
}

/** Largeur d'un emprunt sur la trace entière, en pourcentage. */
function railWidth(passage: MultiridePassage): string {
  if (props.traceLengthKm <= 0) return '0%'
  const ratio = (passage.longueurKm / props.traceLengthKm) * 100
  // Un emprunt très court reste visible : le ruban doit signaler sa présence.
  return `${Math.max(0.4, ratio).toFixed(3)}%`
}

/** Info-bulle d'un rail : bornes kilométriques et sens. */
function railTitle(passage: MultiridePassage): string {
  return `${sensLabel(passage.sens)} — km ${passage.kmEntree.toFixed(2)} → ${passage.kmSortie.toFixed(2)}`
}
</script>

<style scoped>
.mrl-panel-body {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.mrl-panel-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.mrl-segment {
  align-items: flex-start;
  cursor: pointer;
}

.mrl-segment--fp {
  opacity: 0.55;
}

.mrl-strike {
  text-decoration: line-through;
}

/* Ruban : une piste sombre, un rail par emprunt, la barre positionnée en
   pourcentage de la trace. */
.mrl-ribbon {
  position: relative;
  width: 100%;
  margin-top: 6px;
}

.mrl-rail {
  position: relative;
  height: 6px;
  margin-bottom: 2px;
  background: rgba(127, 127, 127, 0.25);
  border-radius: 2px;
}

.mrl-rail-bar {
  position: absolute;
  top: 0;
  height: 6px;
  border-radius: 2px;
}

.mrl-passages {
  margin-top: 6px;
}

.mrl-passage {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 11px;
  line-height: 1.6;
}

.mrl-passage-sens {
  min-width: 58px;
  font-weight: 500;
}

.mrl-passage-range {
  color: rgba(127, 127, 127, 0.95);
}
</style>
