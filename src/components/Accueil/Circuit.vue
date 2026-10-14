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
           - visible **au survol uniquement** ;
           - **désactivée et grisée** quand aucun audit n'est archivé
             (`hasAuditArchive` faux) : la vue `/audit` n'aurait rien à
             restituer.
        -->
        <v-btn
          v-if="!needsAudit"
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-map-marker-path"
          :color="hasAuditArchive ? 'green' : 'grey'"
          variant="text"
          density="comfortable"
          size="small"
          :disabled="!hasAuditArchive"
          :title="hasAuditArchive ? 'Voir les anomalies de la source' : 'Aucune anomalie archivée'"
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
           - trace jamais détectée : visible au survol, neutre et
             **désactivée** — la vue n'aurait rien à restituer.
        -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !(needsMultiride || isHovering) }"
          icon="mdi-repeat"
          :color="multirideColor"
          variant="text"
          density="comfortable"
          size="small"
          :disabled="isMultirideDisabled"
          :title="multirideTitle"
          @click="voirMultiride"
        />
        <!--
          Terrain : icône mdi-terrain, affichée sur la seconde rangée.
          Fonction à câbler ultérieurement. Visible au survol uniquement,
          en blanc (couleur neutre du thème).
        -->
        <v-btn
          class="action-btn"
          :class="{ 'action-btn--hidden': !isHovering }"
          icon="mdi-terrain"
          variant="text"
          density="comfortable"
          size="small"
          title="Terrain"
          @click=""
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
 * L'icône mdi-map-marker-path (seconde rangée) matérialise la consultation
 * des anomalies de la source : elle n'est rendue que lorsque `needsAudit` est
 * faux (sinon l'icône figure déjà sur la première rangée), n'apparaît qu'au
 * survol, et reste **désactivée et grisée** tant qu'aucun audit n'est archivé
 * (`hasAuditArchive` faux). Elle mène à `/audit`.
 *
 * L'icône mdi-repeat (seconde rangée) matérialise la barrière des passages
 * multiples : visible **en permanence** tant que `needsMultiride` est vrai
 * (ambre), sinon visible au survol seulement. Elle est désactivée pour une
 * trace jamais détectée (aucune action à ouvrir). Elle mène à `/multiride`.
 *
 * L'icône mdi-city (seconde rangée) matérialise la **détection des communes
 * traversées** (module Communes), par sa couleur : **grise** tant que la trace
 * n'est pas nettoyée, **jaune** pendant que ce circuit est effectivement traité,
 * **orange** en attente de traitement (en file, ou tâche de fond occupée
 * ailleurs), **verte** une fois la détection aboutie et **rouge** dans les
 * autres cas. Deux états seulement sont actionnables : le **rouge** (le clic
 * lance la détection de ce circuit) et le **vert** (le clic ouvre la fenêtre des
 * communes). L'icône n'est jamais désactivée — un bouton `disabled` masquerait
 * son infobulle et éteindrait la couleur de l'état : c'est le garde du clic qui
 * rend les autres états inertes.
 *
 * Le bouton Info déploie une section (v-expand-transition) contenant les
 * détails du circuit (date d'import, source, lien) ; elle se réduit dès que le
 * curseur quitte la carte. Le clic Info déclenche en outre un focus carte
 * (état `focusedTraceId` du store traces) : Map.vue isole la trace et la cadre.
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
import type { ViewportAspect } from '../../algorithms/keyframeGenerator'
import { formatDistance, formatElevation } from '../../utils/format'

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

/** État d'ouverture de la section info déroulante. */
const infoExpanded = ref(false)
/** true tant que le curseur survole la carte (régit l'affichage des actions). */
const isHovering = ref(false)

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
 * rangée est désactivée (grisée par Vuetify).
 */
const hasAuditArchive = computed(() => !!props.trace.audit_archived)

// --- Icône Passages multiples (seconde rangée) ---

/**
 * Couleur de l'icône mdi-repeat de la seconde rangée :
 *  - ambre tant que les passages restent à valider (barrière 2) ;
 *  - vert pour une trace dont les passages sont validés (consultation) ;
 *  - neutre sinon (trace jamais détectée : aucun état à représenter).
 */
const multirideColor = computed(() => {
  if (needsMultiride.value) return '#FFC107'
  if (props.trace.multiride_status === 'validated') return 'green'
  return ''
})

/**
 * `true` quand l'icône Passages multiples est « neutre » : ni barrière à
 * lever (`pending`) ni passages validés. C'est le cas d'une trace dont la
 * détection n'a jamais été jouée — l'action n'a alors rien à ouvrir, le
 * bouton est désactivé.
 */
const isMultirideDisabled = computed(() => multirideColor.value === '')

/** Tooltip de l'icône Passages multiples, selon l'état de la trace. */
const multirideTitle = computed(() => {
  if (needsMultiride.value) return 'Valider les passages multiples'
  if (props.trace.multiride_status === 'validated') return 'Voir les passages multiples (validés)'
  return 'Passages multiples (détection non jouée)'
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
    grey: 'Communes — circuit à nettoyer au préalable',
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
 * (consulter les communes détectées). Gris, jaune et orange n'ont aucune action,
 * et n'ouvrent pas non plus de fenêtre.
 *
 * L'icône n'étant jamais `disabled` — un bouton désactivé masquerait son
 * infobulle et éteindrait la couleur de l'état —, c'est ce garde qui rend ces
 * trois états inertes.
 */
function onCommunesClick() {
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
 */
function voirMultiride() {
  router.push({ name: 'multiride', query: { traceId: props.trace.id } })
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
</style>