<script setup lang="ts">
/**
 * Panneau latéral des passages multiples : synthèse de la détection, puis un
 * bloc par segment — badges, longueur de référence, **ruban multi-rails** et
 * liste des emprunts.
 *
 * Le ruban situe chaque emprunt sur la trace entière : un rail par emprunt,
 * positionné en pourcentage des kilomètres, coloré selon le sens. C'est la
 * lecture que la spécification demande (« mini-ribbon »), et elle rend visible
 * d'un coup d'œil qu'un troncçon est emprunté plusieurs fois loin les uns des
 * autres.
 *
 * La sélection d'un segment remonte au parent, qui la publie dans le store : la
 * carte s'y synchronise et cadre l'étendue du segment.
 */
import { ref, computed } from 'vue'
import type { MultirideParams, MultiridePassage } from '../../stores/multiride'
import { sensColor, sensLabel } from './multirideMapLayers'

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
  /** `true` dès qu'un ajustement a été porté à la détection. */
  hasAdjustments: boolean
}>()

const emit = defineEmits<{
  select: [segment: number]
  merge: [segment: number]
  'toggle-fp': [segment: number]
  reset: []
}>()

/** Panneaux de synthèse dépliés au premier affichage. */
const openPanels = ref<number[]>([0])

/** Segments distincts marqués faux positifs. */
const falsePositiveCount = computed(
  () => props.segmentNumbers.filter((n) => isFalsePositive(n)).length,
)

/** Segments ayant subi une fusion manuelle. */
const mergedCount = computed(
  () => props.segmentNumbers.filter((n) => segmentPassages(n).some((p) => p.fusionne)).length,
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

/** `true` si le segment a été écarté par l'utilisateur. */
function isFalsePositive(segment: number): boolean {
  return segmentPassages(segment).every((p) => p.fauxPositif)
}

/** Emprunt de référence du segment (le premier). */
function referenceOf(segment: number): MultiridePassage | null {
  return segmentPassages(segment).find((p) => p.sens === 'reference') ?? null
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

        <!-- Réinitialisation : visible dès qu'un ajustement existe (F-16). -->
        <v-btn
          v-if="hasAdjustments"
          class="ma-2"
          block
          size="small"
          variant="tonal"
          color="warning"
          prepend-icon="mdi-restore"
          @click="emit('reset')"
        >
          Réinitialiser les modifications
        </v-btn>

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
            <v-list-item-title class="d-flex align-center">
              <span :class="{ 'mrl-strike': isFalsePositive(segment) }">
                Segment {{ segment }}
              </span>
              <v-chip
                v-if="isFalsePositive(segment)"
                class="ml-2"
                size="x-small"
                variant="tonal"
                color="warning"
              >
                FP
              </v-chip>
              <v-chip
                v-if="segmentPassages(segment).some((p) => p.fusionne)"
                class="ml-2"
                size="x-small"
                variant="tonal"
                color="info"
              >
                fusionné
              </v-chip>
            </v-list-item-title>

            <v-list-item-subtitle>
              {{ segmentPassages(segment).length }} emprunt(s) ·
              {{ (referenceOf(segment)?.longueurKm ?? 0).toFixed(2) }} km
            </v-list-item-subtitle>

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

            <!-- Liste des emprunts : bornes de points, bornes kilométriques et
                 sens. -->
            <div class="mrl-passages">
              <div
                v-for="passage in segmentPassages(segment)"
                :key="passage.passage"
                class="mrl-passage"
              >
                <span class="mrl-passage-sens" :style="{ color: sensColor(passage.sens) }">
                  {{ sensLabel(passage.sens) }}
                </span>
                <span class="mrl-passage-range">
                  points {{ passage.pointEntree }}–{{ passage.pointSortie }} · km
                  {{ passage.kmEntree.toFixed(2) }}–{{ passage.kmSortie.toFixed(2) }} ·
                  {{ passage.longueurKm.toFixed(2) }} km
                </span>
              </div>
            </div>

            <!-- Ajustements du segment : fusion avec le précédent (sauf le
                 premier, qui n'en a pas) et marquage faux positif. Le clic ne
                 doit pas remonter comme une sélection de segment. -->
            <div class="mrl-actions">
              <v-btn
                v-if="segment > 1"
                size="x-small"
                variant="text"
                prepend-icon="mdi-arrow-collapse-up"
                :title="`Fusionner le segment ${segment} avec le précédent`"
                @click.stop="emit('merge', segment)"
              >
                Fusionner S{{ segment }}
              </v-btn>
              <v-btn
                size="x-small"
                variant="text"
                :prepend-icon="isFalsePositive(segment) ? 'mdi-close-circle-outline' : 'mdi-cancel'"
                :title="
                  isFalsePositive(segment)
                    ? 'Réintégrer ce segment dans le résultat'
                    : 'Détecté à tort : exclure ce segment du résultat'
                "
                @click.stop="emit('toggle-fp', segment)"
              >
                {{ isFalsePositive(segment) ? 'Retirer FP' : 'Faux positif' }}
              </v-btn>
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

.mrl-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 4px;
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
