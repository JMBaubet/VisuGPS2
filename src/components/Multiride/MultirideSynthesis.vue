<template>
  <v-expansion-panels variant="accordion" class="ma-2">
    <v-expansion-panel>
      <v-expansion-panel-title>
        <v-icon icon="mdi-chart-box" class="mr-2" />
        Synthèse
      </v-expansion-panel-title>
      <v-expansion-panel-text>
        <v-list density="compact">
          <v-list-item title="Segments répétés">
            <template #append>
              <b>{{ segmentCount }}</b>
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
            <template #append>{{ formatKm(repeatedKm) }}</template>
          </v-list-item>
          <v-list-item v-if="analysisDurationMs > 0" title="Temps d'analyse">
            <template #append>{{ analysisDurationMs }} ms</template>
          </v-list-item>

          <!-- Paramètres actifs : un par ligne, avec son unité. Groupés sous un
               intertitre, ils se lisent sans le tiret qui les séparait. -->
          <v-list-subheader v-if="activeParams.length > 0">
            Paramètres actifs
          </v-list-subheader>
          <v-list-item v-for="param in activeParams" :key="param.label" :title="param.label">
            <template #append>{{ param.value }}</template>
          </v-list-item>
        </v-list>
      </v-expansion-panel-text>
    </v-expansion-panel>
  </v-expansion-panels>
</template>

<script setup lang="ts">
/**
 * Synthèse de la détection des passages multiples : compteurs par famille
 * d'emprunt et par ajustement, kilomètres répétés, et paramètres ayant produit
 * la détection.
 *
 * Elle **ferme le panneau**, sous la liste des segments, comme la synthèse de
 * l'audit : c'est une lecture de contexte, pas une décision — elle est donc
 * repliée tant que l'utilisateur ne la demande pas. La longueur de la trace n'y
 * figure pas : elle ne se rapporte à aucun passage multiple, et la carte porte
 * déjà l'étendue parcourue.
 */
import { computed } from 'vue'
import { formatKm } from './multirideFormat'
import type { MultiridePassage, MultirideParams } from '../../stores/multiride'

const props = defineProps<{
  /** Emprunts détectés, tous segments confondus, triés. */
  passages: MultiridePassage[]
  /** Longueur cumulée des portions répétées, faux positifs exclus (km). */
  repeatedKm: number
  /** Durée de la dernière détection jouée (ms). */
  analysisDurationMs: number
  /** Paramètres ayant produit la détection (`null` : aucune détection). */
  params: MultirideParams | null
}>()

/** Segments distincts détectés. */
const segmentCount = computed(
  () => new Set(props.passages.map((p) => p.segment)).size,
)

/** Segments écartés par l'utilisateur — un faux positif se marque par segment. */
const falsePositiveCount = computed(
  () =>
    new Set(props.passages.filter((p) => p.fauxPositif).map((p) => p.segment)).size,
)

/** Segments réunis à leur précédent par une fusion manuelle. */
const mergedCount = computed(
  () => new Set(props.passages.filter((p) => p.fusionne).map((p) => p.segment)).size,
)

/** Un paramètre actif, tel qu'il se lit : un libellé et sa valeur avec unité. */
interface ActiveParam {
  label: string
  value: string
}

/** Paramètres ayant produit la détection, un par ligne. */
const activeParams = computed<ActiveParam[]>(() => {
  const params = props.params
  if (!params) return []
  return [
    { label: 'Tolérance de superposition', value: `${params.toleranceM} m` },
    { label: 'Longueur minimale', value: `${params.longueurMinM} m` },
    { label: "Pas d'échantillonnage", value: `${params.pasEchantillonnageM} m` },
    { label: 'Fusion des références', value: `${params.fusionReferencesM} m` },
  ]
})
</script>
