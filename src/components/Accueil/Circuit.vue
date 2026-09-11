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
          Éditer : couleur = avancement du verrouillage (vert / jaune / orange /
          rouge, mêmes seuils que la toolbar d'édition). **Forcée visible** quand
          l'édition est incomplète (non verte), même sans survol ; masquée
          uniquement si verte et non survolée ; visible au survol quelle que
          soit la couleur (pour pouvoir lancer l'édition).
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
      </div>
    </v-card-title>

    <!-- Affichage des données principales (distance, dénivelé + actions rapides) -->
    <v-card-text
      style="display: flex; align-items: center"
      class="pt-2"
    >
      <span>
        Distance : {{ formattedDistance }}
        | Dénivelé : {{ formattedElevation }}
      </span>
      <v-spacer></v-spacer>
      <div class="d-flex align-center">
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
    </v-card-text>

    <!-- Section info déroulante (ouverte au clic sur Info, refermée au survol sortant) -->
    <v-expand-transition>
      <div
        v-if="infoExpanded"
      >
        <v-divider></v-divider>
        <v-card-text class="py-2 px-4">
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
 *  - ligne de titre : Éditer, Gérer les groupes, Gérer la météo, Visualiser
 *  - ligne Distance / Dénivelé : Supprimer, Exporter, Info, Affichage, Favoris
 * (les Favoris et l'Affichage restent visibles s'ils sont actifs).
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
 * Couleur de l'icône Éditer selon l'avancement de l'édition (mêmes seuils que
 * le bouton ViewPort de la toolbar d'édition) : vert si **tous** les segments
 * sont verrouillés, jaune si **> 50 %**, orange si **≥ 10 %**, rouge sinon.
 *
 * Une trace non « clean » (anomalies à auditer) est toujours affichée en
 * orange : elle n'est pas candidate à l'édition caméra.
 */
const editColor = computed(() => {
  if (needsAudit.value) return '#FF9800' // orange — à auditer
  const r = lockRatio.value
  if (r >= 1) return '#4CAF50' // vert
  if (r > 0.5) return '#FFEB3B' // jaune
  if (r >= 0.1) return '#FF9800' // orange
  return '#F44336' // rouge
})

/**
 * `true` si la trace doit être auditée avant toute édition (anomalies
 * détectées à l'import, ou audit non encore appliqué).
 */
const needsAudit = computed(() => props.trace.audit_status !== 'clean')

/**
 * `true` si l'édition de la trace est **incomplète** pour le viewport paramétré
 * (ratio < 1 → icône non verte). Dans ce cas l'icône Éditer est **toujours
 * visible** (même sans survol) pour signaler qu'une édition reste à faire.
 */
const isEditionIncomplete = computed(() => lockRatio.value < 1)

/** Tooltip de l'icône Éditer (ratio viewport + % de segments verrouillés). */
const pencilTitle = computed(() => {
  if (needsAudit.value) return 'Auditer la trace (anomalies détectées)'
  const pct = Math.round(lockRatio.value * 100)
  const vp = viewportAspect.value
  return lockRatio.value >= 1
    ? `Éditer — édition complète (${vp})`
    : `Éditer (${vp}) — édition incomplète (${pct} % des segments verrouillés)`
})

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
 * - sinon → édition caméra (`/edition-camera`).
 *
 * La trace auditée est désignée par la **query** de la route : plus de store
 * intermédiaire (Livrable 5 §7.1).
 */
function editerCircuit() {
  if (needsAudit.value) {
    router.push({ name: 'audit', query: { traceId: props.trace.id } })
    return
  }
  editionStore.selectTrace(props.trace.id)
  router.push({ name: 'editionCamera' })
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
 */
.action-btn {
  opacity: 1;
  transition: opacity 0.15s ease;
}

.action-btn--hidden {
  opacity: 0;
  pointer-events: none;
}
</style>
