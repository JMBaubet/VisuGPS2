<template>
  <v-card
    min-height="80px"
    width="500"
    class="mx-auto circuit-card"
    :class="{ 'circuit-hover': isHovering }"
    @mouseenter="isHovering = true"
    @mouseleave="onInfoLeave"
  >

    <!-- Bande couleur gauche -->
    <div
      :class="computedBackgroundColor"
      class="left-strip"
    ></div>

    <v-card-title
      style="max-height: 40px"
      :class="['d-flex', 'align-center']"
    >
      <span
        style="display: block; "
        class="text-headline-small text-truncate"
      >
        {{ trace.name }}
      </span>
      <v-spacer></v-spacer> <!-- Pousse les icônes à droite (alignées sur la ligne Distance/Dénivelé) -->
      <div class="d-flex align-center">
        <!--
          Éditer : icône et couleur selon l'état de la trace.
           - audit requis → mdi-map-marker-path orange (barrière 1) ;
           - sinon → mdi-pencil, couleur = avancement du verrouillage
             (vert / ambre / orange / rouge — seuils identiques à la toolbar
             d'édition, qui emploie le jaune pour le palier > 50 %).
          La barrière 2 (passages multiples à valider) ne figure plus ici :
          elle dispose de sa propre icône mdi-repeat sur la seconde rangée
          (affichée en permanence tant qu'elle reste à lever). **Forcée
          visible** quand l'édition est incomplète (verrouillage partiel ou
          audit restant), même sans survol ; masquée uniquement si verte et
          non survolée ; visible au survol quelle que soit la couleur.
        -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(isEditionIncomplete || isHovering) }"
          :icon="needsAudit ? 'mdi-map-marker-path' : 'mdi-pencil'"
          :color="editColor"
          variant="text"
          density="comfortable"
          size="small"
          :title="pencilTitle"
          @click="editerCircuit"
        />
        <!-- Gérer les groupes : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-account-group"
          variant="text"
          density="comfortable"
          size="small"
          title="Gérer les groupes"
          @click=""
        />
        <!-- Gérer la météo : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-sun-thermometer-outline"
          variant="text"
          density="comfortable"
          size="small"
          title="Gérer la météo"
          @click=""
        />
        <!-- Visualiser : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-video-image"
          variant="text"
          density="comfortable"
          size="small"
          color="green"
          title="Visualiser"
          @click="visualiserCircuit"
        />
        <!-- Favoris : visible si actif, sinon au survol (masqué par opacité) -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(isSelected || isHovering) }"
          :icon="isSelected ? 'mdi-star' : 'mdi-star-outline'"
          :color="isSelected ? 'orange' : ''"
          variant="text"
          density="comfortable"
          size="small"
          :title="isSelected ? 'Retirer des favoris' : 'Ajouter aux favoris'"
          @click="emitToggleFavorite"
        />
      </div>
    </v-card-title>

    <!-- Affichage des données principales (distance, dénivelé + actions rapides) -->
    <v-card-text
      style="display: flex; align-items: center"
      class="pt-2"
    >
      <span>
        Dist. : {{ formattedDistance }}
        | d+ : {{ formattedElevation }}
      </span>
      <v-spacer></v-spacer>
      <div class="d-flex align-center">
        <!--
          Anomalies de la source : icône mdi-map-marker-path affichée sur
          la seconde rangée.
           - **jamais rendue quand `needsAudit` est vrai** : dans ce cas
             l'icône figure déjà sur la première rangée (bouton Éditer en
             mode audit), la redondance est évitée ;
           - **rendue seulement quand un audit est archivé**
             (`hasAuditArchive`) : sans archive, la vue `/audit` n'aurait rien
             à restituer. Principe général : un bouton est affiché et actif,
             ou absent — plus d'état grisé désactivé ;
           - visible **au survol uniquement**.
        -->
        <v-btn
          v-if="!needsAudit && hasAuditArchive"
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-map-marker-path"
          color="green"
          variant="text"
          density="comfortable"
          size="small"
          title="Voir les anomalies de la source"
          @click="voirAnomaliesSource"
        />
        <!--
          Passages multiples : icône mdi-repeat affichée ici uniquement
          (jamais en première rangée ni dans la section info).
           - tant que `needsMultiride` est vrai (détection jouée, passages
             restant à valider) : visible en permanence, couleur ambre —
             c'est une barrière à lever avant l'édition caméra ;
           - passages validés : visible au survol, couleur verte
             (consultation) ;
           - **aucun passage détecté** (`multiride_status === "none"`) :
             visible au survol, **blanche**, et le clic affiche le message
             bleu « Aucun passage multiple détecté sur cette trace. » ;
           - **trace jamais détectée** (`multiride_status` nul) : bouton
             **absent** — il n'y a rien à consulter.
        -->
        <v-btn
          v-if="!isMultirideHidden"
          class="action-btn"
          :class="{ 'action-btn--hidden': !(needsMultiride || isHovering) }"
          icon="mdi-repeat"
          :color="multirideColor"
          variant="text"
          density="comfortable"
          size="small"
          :title="multirideTitle"
          @click="voirMultiride"
        />
        <!--
          Terrain : icône mdi-terrain, affichée sur la seconde rangée.
          Ouvre la vue Altimétrie d'une trace nettoyée.
           - trace non nettoyée : grise, le clic affiche le message
             « L'altimétrie n'est disponible qu'après le nettoyage de la
             trace. » ;
           - trace propre, analyse jamais sauvegardée par l'opérateur :
             **jaune**, et **forcée visible** — une validation reste à mener ;
           - analyse validée (sauvegarde utilisateur) : **verte**, visible au
             survol seulement.
        -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(isTerrainForced || isHovering) }"
          icon="mdi-terrain"
          :color="terrainColor"
          variant="text"
          density="comfortable"
          size="small"
          :title="terrainTitle"
          @click="ouvrirAltimetrie"
        />
        <!--
          Communes traversées (mdi-city) : état de la détection, seconde rangée.
           - **gris** : circuit non nettoyé — la détection ne porte que sur une
             trace propre ;
           - **jaune** : ce circuit est en cours de traitement (import d'une
             trace propre, fin d'audit, ou tâche de fond) ;
           - **orange** : détection en attente — circuit en file, ou tâche de
             fond occupée sur un autre circuit ;
           - **vert** : communes détectées — le clic ouvre la consultation ;
           - **rouge** : circuit propre sans détection — le clic la lance, et le
             bouton reste **forcé visible** hors survol, puisqu'une action reste
             à mener.

          L'icône n'est jamais `disabled` : un bouton désactivé masque son
          infobulle et écrase sa couleur. C'est le garde du clic
          (`onCommunesClick`) qui rend gris, jaune et orange inertes.
        -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(isCommunesForced || isHovering) }"
          icon="mdi-city"
          :color="communesColor"
          variant="text"
          density="comfortable"
          size="small"
          :title="communesTitle"
          @click="onCommunesClick"
        />
        <!-- Supprimer : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-delete"
          color="red-darken-3"
          variant="text"
          density="comfortable"
          size="small"
          title="Supprimer"
          @click="emitDelete"
        />
        <!-- Exporter (non câblé) : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-export"
          variant="text"
          density="comfortable"
          size="small"
          title="Exporter"
        />
        <!-- Info : visible au survol uniquement -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-information-outline"
          variant="text"
          density="comfortable"
          size="small"
          :title="infoExpanded ? 'Masquer les informations' : 'Informations'"
          @click="onInfoClick"
        />
        <!-- Affichage : visible si actif, sinon au survol (masqué par opacité) -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(isDisplayed || isHovering) }"
          :icon="isDisplayed ? 'mdi-map-check' : 'mdi-map-check-outline'"
          :color="isDisplayed ? 'blue' : ''"
          variant="text"
          density="comfortable"
          size="small"
          :title="isDisplayed ? 'Masquer' : 'Afficher'"
          @click="emitToggleDisplay"
        />
      </div>
    </v-card-text>

    <!-- Section info déroulante (ouverte au clic sur Info, refermée au survol sortant) -->
    <v-expand-transition>
      <div
        v-if="infoExpanded"
      >
        <v-divider></v-divider>
        <v-card-text class="py-2 px-4">
          <!-- Détails du circuit : date d'import, source, lien. Les actions
               de consultation (anomalies de la source, passages multiples)
               ont été déplacées sur la seconde rangée d'icônes. -->
          <div class="text-body-2 mb-1">
            Importé le {{ formattedImportDate }}
          </div>
          <div class="text-body-2 mb-1">
            Source : {{ trace.source }}
          </div>
          <div v-if="trace.source_url" class="text-body-2 d-flex align-center">
            <v-icon size="small" icon="mdi-link-variant" class="mr-1" />
            <a href="#" @click.prevent="ouvrirSource" class="text-blue">
              {{ trace.source_url }}
            </a>
          </div>

          <v-divider class="my-2" />

          <div v-if="infoLoading" class="text-caption">
            Chargement des informations…
          </div>
          <div v-else-if="infoError" class="text-caption text-warning">
            {{ infoError }}
          </div>
          <template v-else-if="infoSummary">
            <!-- Audit : aller-retours puis rond-points. La date de l'archive
                 est portée par la seconde ligne, les deux catégories la
                 partageant. -->
            <template v-if="infoSummary.audit">
              <div class="circuit-info__row">
                <span class="circuit-info__left">
                  <v-icon
                    size="x-small"
                    :color="progressColor(
                      infoSummary.audit.arTreated,
                      infoSummary.audit.arTotal,
                    )"
                    icon="mdi-map-marker-path"
                    class="mr-1"
                  />
                  <span
                    :style="{
                      color: progressColor(
                        infoSummary.audit.arTreated,
                        infoSummary.audit.arTotal,
                      ),
                    }"
                  >
                    Aller-retours
                  </span>
                  <template v-if="infoSummary.audit.arTotal > 0">
                    :
                    <span
                      v-if="infoSummary.audit.arTreated > 0"
                      class="text-green"
                    >
                      {{ infoSummary.audit.arTreated }}
                      traité{{ plural(infoSummary.audit.arTreated) }}
                    </span>
                    <span
                      v-if="infoSummary.audit.arTreated > 0 && infoSummary.audit.arPending > 0"
                    >
                      /
                    </span>
                    <span
                      v-if="infoSummary.audit.arPending > 0"
                      class="text-red"
                    >
                      {{ infoSummary.audit.arPending }}
                      non traité{{ plural(infoSummary.audit.arPending) }}
                    </span>
                  </template>
                  <template v-else> : Aucun</template>
                </span>
              </div>

              <div class="circuit-info__row">
                <span class="circuit-info__left">
                  <v-icon
                    size="x-small"
                    :color="progressColor(
                      infoSummary.audit.rpTreated,
                      infoSummary.audit.rpTotal,
                    )"
                    icon="mdi-map-marker-path"
                    class="mr-1"
                  />
                  <span
                    :style="{
                      color: progressColor(
                        infoSummary.audit.rpTreated,
                        infoSummary.audit.rpTotal,
                      ),
                    }"
                  >
                    Rond-points
                  </span>
                  <template v-if="infoSummary.audit.rpTotal > 0">
                    :
                    <span
                      v-if="infoSummary.audit.rpTreated > 0"
                      class="text-green"
                    >
                      {{ infoSummary.audit.rpTreated }}
                      traité{{ plural(infoSummary.audit.rpTreated) }}
                    </span>
                    <span
                      v-if="infoSummary.audit.rpTreated > 0 && infoSummary.audit.rpPending > 0"
                    >
                      /
                    </span>
                    <span
                      v-if="infoSummary.audit.rpPending > 0"
                      class="text-red"
                    >
                      {{ infoSummary.audit.rpPending }}
                      non traité{{ plural(infoSummary.audit.rpPending) }}
                    </span>
                  </template>
                  <template v-else> : Aucun</template>
                </span>
                <span class="circuit-info__right">
                  Traitement :
                  {{ formatDateTime(infoSummary.audit.updatedAt) }}
                </span>
              </div>
            </template>
            <!-- Le repli ne concerne que les traces encore à auditer : une
                 trace propre d'origine n'a jamais eu d'archive, il n'y a rien
                 à annoncer. -->
            <div v-else-if="needsAudit" class="circuit-info__row">
              <span class="circuit-info__left">
                <v-icon size="x-small" icon="mdi-map-marker-path" class="mr-1" />
                Audit :
                <span class="text-medium-emphasis">Aucune archive</span>
              </span>
            </div>

            <!-- Tant que la trace n'est pas nettoyée, seuls l'audit et ses
                 deux familles d'anomalies ont un sens : passages multiples,
                 montées, profil et communes ne s'appliquent qu'à une trace
                 propre. -->
            <template v-if="!needsAudit">
            <!-- Passages multiples : la ligne n'existe que si une détection a
                 laissé une description. La couleur du libellé suit l'icône
                 (verte seulement si la validation est globale et complète). -->
            <div v-if="infoSummary.multiride" class="circuit-info__row">
              <span class="circuit-info__left">
                <v-icon
                  size="x-small"
                  :color="multirideLabelColor"
                  icon="mdi-repeat"
                  class="mr-1"
                />
                <span :style="{ color: multirideLabelColor }">
                  Passages multiples
                </span>
                <template
                  v-if="infoSummary.multiride && infoSummary.multiride.segmentsTotal > 0"
                >
                  :
                  <span
                    v-if="infoSummary.multiride.segmentsTreated > 0"
                    class="text-green"
                  >
                    {{ infoSummary.multiride.segmentsTreated }}
                    traité{{ plural(infoSummary.multiride.segmentsTreated) }}
                  </span>
                  <span
                    v-if="infoSummary.multiride.segmentsTreated > 0 && infoSummary.multiride.segmentsPending > 0"
                  >
                    /
                  </span>
                  <span
                    v-if="infoSummary.multiride.segmentsPending > 0"
                    class="text-red"
                  >
                    {{ infoSummary.multiride.segmentsPending }}
                    non traité{{ plural(infoSummary.multiride.segmentsPending) }}
                  </span>
                </template>
                <template v-else> : Aucune détection</template>
              </span>
              <span
                v-if="infoSummary.multiride && infoSummary.multiride.segmentsTotal > 0"
                class="circuit-info__right"
              >
                Traitement :
                {{ formatDateTime(infoSummary.multiride.updatedAt) }}
              </span>
            </div>

            <!-- Montées : total global et, s'il y en a, nombre de fusionnées.
                 En attente de validation, toute la ligne passe en jaune. -->
            <div
              class="circuit-info__row"
              :class="{ 'circuit-info__row--pending': isAltimetriePending }"
            >
              <span class="circuit-info__left">
                <v-icon
                  size="x-small"
                  :color="infoTerrainColor"
                  icon="mdi-terrain"
                  class="mr-1"
                />
                Montées :
                <template v-if="infoSummary.altimetrie">
                  <template v-if="climbTotal > 0">
                    <b>{{ climbTotal }}</b>
                    <template v-if="infoSummary.altimetrie.mergedClimbs > 0">
                      dont <b>{{ infoSummary.altimetrie.mergedClimbs }}</b>
                      fusionnée{{ plural(infoSummary.altimetrie.mergedClimbs) }}
                    </template>
                  </template>
                  <template v-else>Aucune</template>
                </template>
                <span v-else class="text-medium-emphasis">
                  Analyse indisponible
                </span>
              </span>
              <span v-if="infoSummary.altimetrie" class="circuit-info__right">
                <template v-if="infoSummary.altimetrie.validated">
                  Traitement :
                  {{ formatDateTime(infoSummary.altimetrie.updatedAt) }}
                </template>
                <span v-else>Attente de validation</span>
              </span>
            </div>

            <!-- Profil : uniquement si des montées existent. -->
            <div
              v-if="infoSummary.altimetrie && climbTotal > 0"
              class="circuit-info__profile"
            >
              <div class="circuit-info__label mb-1">
                <v-icon
                  size="x-small"
                  icon="mdi-chart-areaspline"
                  class="mr-1"
                />
                Profil d'altitude
              </div>
              <CircuitAltitudeProfile
                :profile="infoSummary.altimetrie.profile"
                :climbs="infoSummary.altimetrie.climbs"
                :alt-min-m="infoSummary.altimetrie.altMinM"
                :alt-max-m="infoSummary.altimetrie.altMaxM"
                :total-distance-m="infoSummary.altimetrie.totalDistanceM"
              />
            </div>

            <!-- Communes : sous le profil. -->
            <div class="circuit-info__row">
              <span class="circuit-info__left">
                <v-icon
                  size="x-small"
                  :color="communesColor"
                  icon="mdi-city"
                  class="mr-1"
                />
                Communes traversées :
                <template v-if="infoSummary.communes">
                  <b>{{ infoSummary.communes.communeCount }}</b>
                  commune{{ plural(infoSummary.communes.communeCount) }}
                </template>
                <span v-else class="text-medium-emphasis">
                  Détection indisponible
                </span>
              </span>
            </div>
            </template>
          </template>

          <!-- Diagnostic : l'identifiant n'apparaît qu'en mode debug, avec
               une icône de copie dans le presse-papiers du système. -->
          <div v-if="appStore.isDebug" class="circuit-info__id">
            <v-btn
              icon="mdi-content-copy"
              size="x-small"
              variant="text"
              density="comfortable"
              title="Copier l'identifiant de la trace"
              @click.stop="copierIdentifiant"
            />
            <span>ID : {{ trace.id }}</span>
          </div>
        </v-card-text>
      </div>
    </v-expand-transition>

  </v-card>
  <v-divider></v-divider>
</template>

<script setup lang="ts">
/**
 * Composant affichant une carte de circuit (trace GPX importée).
 *
 * Deux lignes d'icônes d'action (masquées par opacité hors survol) :
 *  - ligne de titre : Éditer, Gérer les groupes, Gérer la météo, Visualiser ;
 *  - ligne Distance / Dénivelé : Anomalies de la source (mdi-map-marker-path),
 *    Passages multiples (mdi-repeat), Terrain, Communes (mdi-city), Supprimer,
 *    Exporter, Info, Affichage, Favoris.
 * La section info déroulante (ouverte via Info) ne contient plus que les
 * détails du circuit (date d'import, source, lien).
 *
 * L'icône Éditer (ligne de titre) : mdi-map-marker-path orange tant qu'un
 * audit reste à faire, mdi-pencil sinon — dont la couleur traduit l'avancement
 * du verrouillage (mêmes seuils que la toolbar d'édition, qui emploie le jaune
 * pour le palier > 50 %).
 *
 * Principe général des actions de la seconde rangée : un bouton est **affiché
 * et actif**, ou **absent** — plus d'état grisé désactivé. Les deux exceptions
 * sont des explications au clic, pas des boutons inertes.
 *
 * L'icône mdi-map-marker-path (seconde rangée) matérialise la consultation
 * des anomalies de la source : elle n'est rendue que lorsque `needsAudit` est
 * faux (sinon l'icône figure déjà sur la première rangée) **et** qu'un audit
 * est archivé (`hasAuditArchive`). Sans archive, la vue `/audit` n'aurait rien
 * à restituer : le bouton est absent. Elle mène à `/audit`.
 *
 * L'icône mdi-repeat (seconde rangée) matérialise la barrière des passages
 * multiples : **blanche** et accompagnée du message bleu « Aucun passage
 * multiple détecté sur cette trace. » quand la détection n'a rien trouvé
 * (`multiride_status === "none"`), visible **en permanence** et ambre tant que
 * `needsMultiride` est vrai, verte une fois les passages validés. Une trace
 * jamais détectée (`multiride_status` nul) n'affiche **pas** le bouton.
 *
 * L'icône mdi-city (seconde rangée) matérialise la **détection des communes
 * traversées** (module Communes), par sa couleur : **grise** tant que la trace
 * n'est pas nettoyée (le clic explique la condition d'accès), **jaune** pendant
 * que ce circuit est effectivement traité, **orange** en attente de traitement
 * (en file, ou tâche de fond occupée ailleurs), **verte** une fois la détection
 * aboutie et **rouge** dans les autres cas. Deux états seulement sont
 * actionnables : le **rouge** (le clic lance la détection de ce circuit) et le
 * **vert** (le clic ouvre la fenêtre des communes). L'icône n'est jamais
 * désactivée — un bouton `disabled` masquerait son infobulle et éteindrait la
 * couleur de l'état : c'est le garde du clic qui rend les autres états inertes.
 *
 * L'icône mdi-terrain (seconde rangée) ouvre la vue Altimétrie d'une trace
 * propre : **jaune** et forcée visible tant que l'opérateur n'a pas validé
 * l'analyse (aucune sauvegarde utilisateur), **verte** une fois validée, grise
 * et explicative tant que la trace n'est pas nettoyée.
 *
 * Le bouton Info déploie une section (v-expand-transition) contenant les
 * détails du circuit (date d'import, source, lien) puis une **synthèse** lue à
 * la demande par `get_trace_info` : une ligne par catégorie, date à droite —
 * **aller-retours** puis **rond-points** (libellé rouge / jaune / vert selon
 * l'avancement, compteurs traités en vert et non traités en rouge), **passages
 * multiples** (libellé de la couleur de l'icône), **montées** (total global et
 * nombre de fusionnées, avec « Attente de validation » ou la date), puis le
 * **profil d'altitude seulement si des montées existent** et le nombre de
 * **communes** en dessous. En **mode debug** (`appStore.isDebug`), l'identifiant
 * de la trace est ajouté en pied avec une icône de copie. La section se réduit
 * dès que le curseur quitte la carte. Le clic Info déclenche en outre un focus
 * carte (état `focusedTraceId` du store traces) : Map.vue isole la trace et la
 * cadre.
 */
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { useAppStore } from '../../stores/app'
import { useTracesStore } from '../../stores/traces'
import type { TraceMetadata } from '../../stores/traces'
import { useCommunesStore } from '../../stores/communes'
import { useEditionStore } from '../../stores/edition'
import { useSettingsStore } from '../../stores/settings'
import { useKeyframesStore } from '../../stores/keyframes'
import { useUiStore } from '../../stores/ui'
import type { ViewportAspect } from '../../algorithms/keyframeGenerator'
import { useCircuitInfo } from '../../composables/useCircuitInfo'
import CircuitAltitudeProfile from './CircuitAltitudeProfile.vue'
import {
  formatDateTime,
  formatDistance,
  formatElevation,
} from '../../utils/format'

const emit = defineEmits<{
  /** Émis quand l'utilisateur demande la suppression de la trace. */
  (e: 'delete', id: string): void
  /** Émis quand l'utilisateur bascule le favori. */
  (e: 'toggle-favorite', id: string): void
  /** Émis quand l'utilisateur bascule l'affichage. */
  (e: 'toggle-display', id: string): void
}>()

const props = defineProps<{
  /** Métadonnées de la trace importée. */
  trace: TraceMetadata
  /** Classe CSS de la bande couleur gauche (optionnel). */
  backgroundColor?: string
}>()

const appStore = useAppStore()
const router = useRouter()
const tracesStore = useTracesStore()
const communesStore = useCommunesStore()
const editionStore = useEditionStore()
const settingsStore = useSettingsStore()
const keyframesStore = useKeyframesStore()
const ui = useUiStore()

// Synthèse du volet Info, chargée à la première ouverture de la section.
const {
  summary: infoSummary,
  loading: infoLoading,
  error: infoError,
  load: loadInfo,
} = useCircuitInfo()

/** État d'ouverture de la section info déroulante. */
const infoExpanded = ref(false)
/** true tant que le curseur survole la carte (régit l'affichage des actions). */
const isHovering = ref(false)

/** Nombre total de montées, fusionnées et séparées comprises. */
const climbTotal = computed(() => {
  const altimetrie = infoSummary.value?.altimetrie
  if (!altimetrie) return 0
  return (
    altimetrie.simpleClimbs +
    altimetrie.mergedClimbs +
    altimetrie.separatedClimbs
  )
})

/**
 * Couleur de l'icône Terrain du volet Info, lue sur l'analyse affichée :
 * jaune tant qu'elle n'est pas validée, blanche si validée sans montée, verte
 * sinon. Neutre quand l'analyse est indisponible.
 */
const infoTerrainColor = computed(() => {
  const altimetrie = infoSummary.value?.altimetrie
  if (!altimetrie) return ''
  if (!altimetrie.validated) return '#FBC02D'
  return climbTotal.value === 0 ? 'white' : '#4CAF50'
})

/**
 * L'analyse existe mais n'a pas été validée : toute la ligne « Montées »
 * passe en jaune, pour attirer l'attention sur la validation à mener.
 */
const isAltimetriePending = computed(
  () => infoSummary.value?.altimetrie?.validated === false,
)

// --- Avancement de l'édition (icône Éditer) ---

/**
 * Ratio (0..1) de segments **verrouillés** pour le **viewport paramétré**
 * (`Edition.Camera.viewportDefaut`). 0 si aucun jeu de keyframes ou indisponible
 * (édition jamais commencée). Calculé au montage (les cartes sont remontées à
 * chaque retour sur l'accueil, l'état est donc rafraîchi).
 */
const lockRatio = ref(0)

/** Ratio viewport paramétré (`Edition.Camera.viewportDefaut`) utilisé pour l'édition. */
const viewportAspect = ref<ViewportAspect>('16:9')

/**
 * Charge le fichier keyframes du viewport paramétré et calcule le ratio de
 * verrouillage des segments (segments verrouillés / segments totaux).
 */
onMounted(async () => {
  try {
    const raw = await settingsStore.getSettingValue('Edition.Camera.viewportDefaut')
    viewportAspect.value = raw === '4:3' ? '4:3' : '16:9'
    const kf = await keyframesStore.loadKeyframes(props.trace.id, viewportAspect.value)
    if (kf && kf.keyframes.length > 1) {
      const locked = kf.keyframes.filter(k => k.locked).length
      lockRatio.value = locked / (kf.keyframes.length - 1)
    }
  } catch (e) {
    console.warn(`[Circuit] État d'édition indisponible pour ${props.trace.id} :`, e)
    lockRatio.value = 0
  }
})

/**
 * Couleur de l'icône Éditer.
 * La barrière d'audit prime sur l'avancement du verrouillage, car elle ferme
 * l'accès à l'édition : une trace non « clean » est orange. Sinon :
 * vert (tout verrouillé) → ambre (> 50 %) → orange (≥ 10 %) → rouge.
 *
 * La barrière des passages multiples n'entre pas dans cette couleur : elle
 * dispose de sa propre icône mdi-repeat sur la seconde rangée.
 */
const editColor = computed(() => {
  if (needsAudit.value) return '#FF9800' // orange — à auditer
  const r = lockRatio.value
  if (r >= 1) return '#4CAF50' // vert
  if (r > 0.5) return '#FFC107' // ambre
  if (r >= 0.1) return '#FF9800' // orange
  return '#F44336' // rouge
})

/**
 * `true` si la trace doit être auditée avant toute édition (anomalies
 * détectées à l'import, ou audit non encore appliqué).
 */
const needsAudit = computed(() => props.trace.audit_status !== 'clean')

/**
 * `true` si la trace porte des passages multiples détectés mais non validés :
 * l'édition caméra reste inaccessible tant qu'ils n'ont pas été validés dans la
 * vue `/multiride`. Une trace jamais détectée (`null`) passe librement — la
 * barrière ne concerne que les traces dont la détection a été jouée.
 *
 * Régit la visibilité **permanente** de l'icône mdi-repeat sur la seconde
 * rangée.
 */
const needsMultiride = computed(() => props.trace.multiride_status === 'pending')

/**
 * `true` si l'édition de la trace est **incomplète** pour le viewport
 * paramétré (ratio < 1 → icône non verte), ou si un audit reste à mener.
 * Dans ce cas l'icône Éditer (1ʳᵉ rangée) est **toujours visible** (même sans
 * survol) pour signaler qu'une action reste à mener.
 *
 * La barrière des passages multiples n'entre pas dans ce calcul : c'est
 * l'icône mdi-repeat de la seconde rangée qui en porte le signal.
 */
const isEditionIncomplete = computed(
  () => lockRatio.value < 1 || needsAudit.value,
)

/** Tooltip de l'icône Éditer (barrière d'audit, ou avancement du verrouillage). */
const pencilTitle = computed(() => {
  if (needsAudit.value) return 'Auditer la trace (anomalies détectées)'
  const pct = Math.round(lockRatio.value * 100)
  const vp = viewportAspect.value
  return lockRatio.value >= 1
    ? `Éditer — édition complète (${vp})`
    : `Éditer (${vp}) — édition incomplète (${pct} % des segments verrouillés)`
})

/**
 * `true` si un audit archivé est disponible pour cette trace : c'est la
 * condition pour que la consultation des anomalies de la source ait du
 * contenu à restituer. Sinon l'icône `mdi-map-marker-path` de la seconde
 * rangée est absente.
 */
const hasAuditArchive = computed(() => !!props.trace.audit_archived)

// --- Icône Passages multiples (seconde rangée) ---

/**
 * État de l'icône Passages multiples :
 * - `hidden`    : détection jamais jouée — rien à consulter, bouton absent ;
 * - `none`      : détection jouée, aucun passage — icône blanche, message au clic ;
 * - `pending`   : passages à valider — barrière 2, icône jaune forcée visible ;
 * - `validated` : validation globale posée — icône verte seulement si tous les
 *   segments ont un verdict (`multiride_complete`), jaune sinon.
 */
type MultirideState = 'hidden' | 'none' | 'pending' | 'validated'

const multirideState = computed<MultirideState>(() => {
  const status = props.trace.multiride_status
  if (status == null) return 'hidden'
  if (status === 'pending') return 'pending'
  if (status === 'validated') return 'validated'
  return 'none'
})

/** Trace jamais détectée : le bouton est masqué, pas grisé. */
const isMultirideHidden = computed(() => multirideState.value === 'hidden')

/**
 * `true` quand la détection est **globalement validée** et que **tous les
 * segments** ont reçu un verdict. La couleur verte de l'icône est réservée à
 * ce cas : une validation globale qui laisse des segments à examiner reste
 * jaune.
 */
const isMultirideComplete = computed(
  () =>
    props.trace.multiride_status === 'validated' &&
    props.trace.multiride_complete === true,
)

/**
 * Couleur de l'icône mdi-repeat :
 *  - **verte** si la validation est globale et complète ;
 *  - **jaune** tant que des segments restent à examiner, même si la
 *    validation globale a été posée ;
 *  - blanc quand la détection n'a trouvé aucun passage.
 */
const multirideColor = computed(() => {
  if (multirideState.value === 'none') return 'white'
  return isMultirideComplete.value ? 'green' : '#FFC107'
})

/**
 * Couleur du libellé « Passages multiples » du volet Info : la même que
 * l'icône, pour que les deux lisent le même état. Neutre quand la détection
 * n'a jamais été jouée (icône absente).
 */
const multirideLabelColor = computed(() => {
  if (multirideState.value === 'hidden') return ''
  if (multirideState.value === 'none') return 'white'
  return isMultirideComplete.value ? '#4CAF50' : '#FFC107'
})

/** Tooltip de l'icône Passages multiples, selon l'état de la trace. */
const multirideTitle = computed(() => {
  switch (multirideState.value) {
    case 'pending':
      return 'Valider les passages multiples'
    case 'validated':
      return isMultirideComplete.value
        ? 'Voir les passages multiples (validés)'
        : 'Passages multiples — validation globale, segments encore à examiner'
    case 'none':
      return 'Aucun passage multiple détecté sur cette trace'
    default:
      return ''
  }
})

/**
 * Couleur d'une catégorie d'anomalies selon son avancement : rouge sans
 * traitement, jaune partiel, vert complet. Chaîne vide si la catégorie est
 * absente (total nul), pour laisser le libellé dans sa couleur neutre.
 */
function progressColor(treated: number, total: number): string {
  if (total === 0) return ''
  if (treated === 0) return '#F44336'
  if (treated >= total) return '#4CAF50'
  return '#FBC02D'
}

/** Suffixe de pluriel des libellés de comptage (« 1 traité », « 2 traités »). */
function plural(count: number): string {
  return count > 1 ? 's' : ''
}

// --- Icône Terrain (seconde rangée) ---

/**
 * État de l'icône Terrain :
 * - `locked`    : trace non nettoyée — le clic explique la condition d'accès ;
 * - `pending`   : analyse disponible mais jamais sauvegardée par l'opérateur ;
 * - `validated` : une sauvegarde utilisateur a validé l'analyse.
 */
type TerrainState = 'locked' | 'pending' | 'validated'

const terrainState = computed<TerrainState>(() => {
  if (needsAudit.value) return 'locked'
  return props.trace.altimetrie_validated ? 'validated' : 'pending'
})

/**
 * Couleur de l'icône Terrain : grise tant que la trace n'est pas nettoyée,
 * **jaune** tant que l'analyse reste à valider, **verte** une fois validée —
 * sauf si l'analyse validée ne contient **aucune montée**, où elle est
 * **blanche**.
 */
const terrainColor = computed(() => {
  if (terrainState.value === 'locked') return 'grey'
  if (terrainState.value === 'pending') return 'yellow-darken-2'
  return props.trace.altimetrie_climb_count === 0 ? 'white' : 'green'
})

/**
 * Une analyse non validée est une action en attente : l'icône jaune reste
 * visible hors survol, comme la barrière des passages multiples.
 */
const isTerrainForced = computed(() => terrainState.value === 'pending')

/** Tooltip de l'icône Terrain, selon l'état de la trace. */
const terrainTitle = computed(() => {
  if (terrainState.value === 'locked') {
    return "L'altimétrie n'est disponible qu'après le nettoyage de la trace."
  }
  if (terrainState.value !== 'validated') {
    return "Analyser l'altimétrie — validation requise"
  }
  return props.trace.altimetrie_climb_count === 0
    ? 'Altimétrie validée — aucune montée détectée'
    : "Altimétrie validée — consulter l'analyse"
})

// --- Icône Communes (seconde rangée) ---------------------------------

/**
 * État de la détection des communes, du point de vue de cette carte :
 * - `grey`   : circuit non nettoyé — la détection ne porte que sur une trace propre ;
 * - `yellow` : **ce circuit** est effectivement en cours de traitement, quelle que
 *              soit l'origine de la demande (import, fin d'audit, tâche de fond) ;
 * - `orange` : détection en attente — circuit dans la file, ou tâche de fond
 *              occupée sur un autre circuit ;
 * - `green`  : communes détectées — la carte propose la consultation ;
 * - `red`    : tout autre cas — circuit propre sans détection, **seul état où le
 *              clic lance** la détection.
 */
type CommunesState = 'grey' | 'yellow' | 'orange' | 'green' | 'red'

/**
 * État de la détection des communes traversées (module Communes) pour cette
 * trace.
 *
 * L'ordre d'évaluation compte : un circuit **déjà détecté reste vert** même
 * pendant qu'une tâche de fond traite d'autres circuits, et seul le circuit
 * réellement en cours de traitement passe au jaune — les deux états chauds
 * signalent ce qui reste à faire, jamais ce qui est fait.
 */
const communesState = computed<CommunesState>(() => {
  if (needsAudit.value) return 'grey'        // trace non nettoyée : hors sujet
  if (props.trace.communes_completed) return 'green'
  if (communesStore.currentTraceId === props.trace.id) return 'yellow'
  if (communesStore.isRunning || communesStore.isPending(props.trace.id)) return 'orange'
  return 'red'
})

/**
 * Couleur de l'icône : le jaune marque le traitement en cours, le rouge la seule
 * situation où le bouton lance quelque chose.
 *
 * Le jaune est volontairement assombri (`yellow-darken-2`) : le jaune de base
 * est illisible sur une carte claire.
 */
const communesColor = computed(() => {
  const colors: Record<CommunesState, string> = {
    grey: 'grey',
    yellow: 'yellow-darken-2',
    orange: 'orange',
    green: 'green',
    red: 'red',
  }
  return colors[communesState.value]
})

/** Le bouton est **forcé visible** quand une action reste à mener. */
const isCommunesForced = computed(() => communesState.value === 'red')

/** Tooltip de l'icône Communes : l'état réel, ou l'action quand il y en a une. */
const communesTitle = computed(() => {
  const titles: Record<CommunesState, string> = {
    grey: "Les communes traversées ne sont disponibles qu'après le nettoyage de la trace.",
    yellow: 'Communes — détection en cours pour ce circuit…',
    orange: 'Communes — détection en attente…',
    green: 'Voir les communes traversées',
    red: 'Détecter les communes traversées',
  }
  return titles[communesState.value]
})

/**
 * Action de l'icône Communes : elle n'existe que dans les deux états
 * actionnables — **rouge** (lancer la détection de ce circuit) et **verte**
 * (consulter les communes détectées). Jaune et orange n'ont aucune action,
 * et n'ouvrent pas non plus de fenêtre.
 *
 * L'icône n'étant jamais `disabled` — un bouton désactivé masquerait son
 * infobulle et éteindrait la couleur de l'état —, c'est ce garde qui rend ces
 * états inertes. Le gris (trace non nettoyée) explique la condition d'accès
 * par un message, au lieu de rester silencieux.
 */
function onCommunesClick() {
  if (communesState.value === 'grey') {
    ui.showWarning(
      "Les communes traversées ne sont disponibles qu'après le nettoyage de la trace.",
    )
    return
  }
  if (communesState.value === 'red') {
    // La trace passe en tête de la file du store : une analyse déjà en cours se
    // termine d'abord, puis Overpass et le backend enchaînent.
    communesStore.request(props.trace.id)
    return
  }
  if (communesState.value === 'green') {
    appStore.openCommunes(props.trace.id)
  }
}

/**
 * Ouvre/ferme la section info et pilote le focus carte correspondant.
 * Une seule carte peut avoir le focus à la fois : la prise d'un nouveau focus
 * ferme automatiquement les autres sections ouvertes (via le watcher ci-dessous).
 */
function onInfoClick() {
  if (infoExpanded.value) {
    infoExpanded.value = false
    if (tracesStore.focusedTraceId === props.trace.id) tracesStore.focusedTraceId = null
  } else {
    infoExpanded.value = true
    tracesStore.focusedTraceId = props.trace.id
    // Lecture disque à la première ouverture seulement : le résultat reste
    // en cache tant que la carte est montée.
    void loadInfo(props.trace.id)
  }
}

/** Ferme la section info et libère le focus quand le curseur quitte la carte. */
function onInfoLeave() {
  isHovering.value = false
  if (!infoExpanded.value) return
  infoExpanded.value = false
  if (tracesStore.focusedTraceId === props.trace.id) tracesStore.focusedTraceId = null
}

// Synchronisation : si une autre carte prend le focus, refermer celle-ci.
watch(() => tracesStore.focusedTraceId, (newId) => {
  if (newId !== props.trace.id && infoExpanded.value) {
    infoExpanded.value = false
  }
})

// --- État persisté (lié aux props via le backend) ---

const isSelected = computed(() => props.trace.favorite)
const isDisplayed = computed(() => props.trace.is_displayed)

// --- Formatted stats ---

const formattedDistance = computed(() => formatDistance(props.trace.stats.distance_m))
const formattedElevation = computed(() => formatElevation(props.trace.stats.positive_elevation_m))

/** Date d'import formatée en français (JJ/MM/AAAA à HH:MM). */
const formattedImportDate = computed(() => {
  try {
    const date = new Date(props.trace.import_date)
    return date.toLocaleDateString('fr-FR', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return props.trace.import_date
  }
})

/**
 * Classe CSS de la bande couleur, neutre par défaut.
 * Conservée pour un usage futur (mise à jour des données de visualisation).
 */
const computedBackgroundColor = computed(() =>
  props.backgroundColor ? props.backgroundColor : 'bg-grey'
)

// --- Actions ---

/** Émet l'événement de suppression vers le composant parent. */
function emitDelete() {
  emit('delete', props.trace.id)
}

/** Émet un événement de bascule du favori vers le composant parent. */
function emitToggleFavorite() {
  emit('toggle-favorite', props.trace.id)
}

/** Émet un événement de bascule de l'affichage vers le composant parent. */
function emitToggleDisplay() {
  emit('toggle-display', props.trace.id)
}

/** Ouvre l'URL source dans le navigateur par défaut. */
async function ouvrirSource() {
  if (props.trace.source_url) {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener')
      await openUrl(props.trace.source_url)
    } catch (e) {
      console.error("Erreur lors de l'ouverture de l'URL source :", e)
    }
  }
}

/**
 * Copie l'identifiant de la trace dans le presse-papiers du système.
 *
 * L'API Clipboard du webview est tentée d'abord ; le repli `execCommand`
 * couvre les webviews où elle n'est pas exposée. L'action n'est proposée
 * qu'en mode debug, où l'identifiant est affiché.
 */
async function copierIdentifiant(): Promise<void> {
  const value = props.trace.id

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value)
      ui.showSuccess('Identifiant copié.')
      return
    }
  } catch {
    // Repli ci-dessous : certains webviews refusent l'API Clipboard.
  }

  try {
    copierViaExecCommand(value)
    ui.showSuccess('Identifiant copié.')
  } catch {
    ui.showError("Copie de l'identifiant impossible.")
  }
}

/** Repli de copie pour les webviews sans API Clipboard. */
function copierViaExecCommand(value: string): void {
  const textarea = document.createElement('textarea')
  textarea.value = value
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.appendChild(textarea)
  textarea.focus()
  textarea.select()
  const copied = document.execCommand('copy')
  document.body.removeChild(textarea)
  if (!copied) throw new Error('Copie refusée par le webview.')
}

/**
 * Sélectionne la trace puis navigue vers la vue adaptée :
 * - trace non « clean » → vue d'audit (`/audit?traceId=…`) : la trace contient
 *   des anomalies (aller-retours, boucles giratoires) et n'est **pas candidate**
 *   à l'édition caméra tant qu'elles ne sont pas traitées et validées ;
 * - trace dont les passages multiples ne sont pas validés → vue des passages
 *   multiples (`/multiride?traceId=…`), qui lève la seconde barrière ;
 * - sinon → édition caméra (`/edition-camera`).
 *
 * Les deux barrières s'enchaînent : l'audit d'abord, les passages multiples
 * ensuite. La trace est désignée par la **query** de la route : plus de store
 * intermédiaire (Livrable 5 §7.1).
 */
function editerCircuit() {
  if (needsAudit.value) {
    router.push({ name: 'audit', query: { traceId: props.trace.id } })
    return
  }
  if (needsMultiride.value) {
    router.push({ name: 'multiride', query: { traceId: props.trace.id } })
    return
  }
  editionStore.selectTrace(props.trace.id)
  router.push({ name: 'editionCamera' })
}

/**
 * Ouvre la consultation des anomalies de la source.
 *
 * Cible de l'icône mdi-map-marker-path de la seconde rangée, rendue seulement
 * pour une trace « clean » (une trace à auditer voit l'icône sur la première
 * rangée, via le bouton Éditer).
 */
function voirAnomaliesSource() {
  router.push({ name: 'audit', query: { traceId: props.trace.id } })
}

/**
 * Ouvre la vue des passages multiples (validation ou consultation).
 *
 * Cible unique de l'icône mdi-repeat de la seconde rangée : que la trace soit
 * en attente de validation (barrière à lever) ou déjà validée (consultation),
 * la vue est la même. Un ajustement y est **toujours** possible, y compris sur
 * des passages déjà validés : il réécrit le fichier de description sans faire
 * rebasculer le statut, un ajustement n'ayant pas d'incidence sur l'édition
 * caméra.
 *
 * Une détection sans passage (`none`) n'a rien à ouvrir : le clic se contente
 * du message bleu, l'icône restant blanche.
 */
function voirMultiride() {
  if (multirideState.value === 'none') {
    ui.showInfo('Aucun passage multiple détecté sur cette trace.')
    return
  }
  router.push({ name: 'multiride', query: { traceId: props.trace.id } })
}

/**
 * Ouvre la vue Altimétrie.
 *
 * L'analyse suit le nettoyage de la trace : une trace non « clean » est
 * refusée, avec un message qui renvoie vers l'audit. Le multiride n'est pas
 * une barrière : l'altimétrie est une analyse indépendante.
 */
function ouvrirAltimetrie() {
  if (needsAudit.value) {
    ui.showWarning(
      "L'altimétrie n'est disponible qu'après le nettoyage de la trace.",
    )
    return
  }
  router.push({ name: 'altimetrie', query: { traceId: props.trace.id } })
}

/** Lance la visualisation 3D du circuit. */
async function visualiserCircuit() {
  // Recharger les informations des écrans pour détecter en temps réel un branchement
  await appStore.loadDisplays()

  if (appStore.displays.length >= 2) {
    try {
      await invoke('open_second_window')
      appStore.isScreenBisOpen = true
    } catch (e) {
      console.error("Erreur lors de l'ouverture de la seconde fenêtre :", e)
    }
  } else {
    // Configuration mono-écran : afficher ScreenBis.vue dans la fenêtre actuelle
    appStore.isScreenBisOpen = true
    router.push({ name: 'screenBis' })
  }
}
</script>

<style scoped>
.left-strip {
  position: absolute;
  left: 0;
  top: 0;
  width: 10px;
  height: 100%;
  border-radius: 8px 0 0 8px;
}

/*
 * Survol du circuit : on teinte la surface avec une part de `on-surface`.
 * Cette variable est foncée en thème clair et claire en thème sombre, le
 * mélange assombrit donc la carte en mode clair et l'éclaircit en mode sombre,
 * sans aucune détection de thème.
 */
.circuit-card {
  transition: background-color 0.15s ease;
}

.circuit-hover {
  background-color: color-mix(in srgb, rgb(var(--v-theme-surface)) 88%, rgb(var(--v-theme-on-surface)));
}

/*
 * Boutons d'action : toujours présents dans le layout (ils gardent leur
 * emplacement), mais masqués visuellement par opacité tant que le curseur
 * n'est pas sur la carte. L'icône reste intangible quand elle est masquée.
 *
 * Aucune `opacity: 1` n'est déclarée ici : ce serait écraser l'opacité
 * réduite que Vuetify applique aux boutons désactivés (0.26 par défaut),
 * rendant indistinguables un bouton actif et un bouton disabled.
 */
.action-btn {
  transition: opacity 0.15s ease;
}

.action-btn--hidden {
  opacity: 0;
  pointer-events: none;
}

/* Synthèse du volet Info : une ligne par thème, la date en retrait. */
.circuit-info__row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  font-size: 0.8rem;
  line-height: 1.45;
}

.circuit-info__row + .circuit-info__row {
  margin-top: 4px;
}

.circuit-info__left {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0 4px;
  min-width: 0;
}

.circuit-info__right {
  flex: 0 0 auto;
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.55);
  white-space: nowrap;
}

.circuit-info__label {
  font-size: 0.72rem;
  font-weight: 500;
  color: rgba(var(--v-theme-on-surface), 0.62);
  white-space: nowrap;
}

/* Ligne « Montées » en attente de validation : toute la ligne en jaune. */
.circuit-info__row--pending,
.circuit-info__row--pending .circuit-info__right {
  color: #FBC02D;
}

.circuit-info__profile {
  margin-top: 10px;
}

.circuit-info__id {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 10px;
  font-family: monospace;
  font-size: 0.68rem;
  color: rgba(var(--v-theme-on-surface), 0.55);
  word-break: break-all;
}
</style>