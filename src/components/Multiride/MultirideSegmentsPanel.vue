<template>
  <v-navigation-drawer permanent width="360" class="mrl-panel">
    <div class="mrl-panel-body">
      <div class="mrl-panel-list">
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
                 L'état du segment est porté à droite par ses badges : un
                 verdict — écarté, ou approuvé —, et la marque d'une fusion, qui
                 est d'un autre ordre et peut donc l'accompagner. -->
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
              <template v-else>
                <v-chip
                  v-if="isMerged(segment)"
                  size="x-small"
                  variant="tonal"
                  color="info"
                >
                  fusionné
                </v-chip>
                <v-chip
                  v-if="isValidated(segment)"
                  size="x-small"
                  variant="tonal"
                  color="success"
                >
                  validé
                </v-chip>
              </template>
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

      <v-divider />

      <!-- La synthèse ferme le panneau, sous la liste : une lecture de
           contexte, repliée tant qu'on ne la demande pas. -->
      <MultirideSynthesis
        :passages="passages"
        :repeated-km="repeatedKm"
        :analysis-duration-ms="analysisDurationMs"
        :params="params"
      />
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Panneau latéral des passages multiples : un bloc par segment — en-tête,
 * **ruban multi-rails** et emprunts répétés —, fermé par la synthèse.
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
import type { MultirideParams, MultiridePassage } from '../../stores/multiride'
import MultirideSynthesis from './MultirideSynthesis.vue'
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

/** `true` si le segment a été approuvé tel quel. */
function isValidated(segment: number): boolean {
  return segmentPassages(segment).some((p) => p.valide)
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
