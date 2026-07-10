<template>
  <v-card
    min-height="80px"
    width="500"
    class="mx-auto"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >

    <!-- Bande couleur gauche -->
    <div
      :class="computedBackgroundColor"
      class="left-strip"
    ></div>

    <v-card-title
      style="max-height: 40px"
      :class="['d-flex', 'align-center', 'pr-0']"
    >
      <span
        style="display: block; "
        class="text-headline-small text-truncate"
      >
        {{ trace.name }}
      </span>
      <v-spacer></v-spacer> <!-- A garder pour avoir les menus à droite -->
      <v-menu open-on-hover location="right top">
        <template v-slot:activator="{ props }">
          <v-btn icon="mdi-dots-vertical" variant="text" v-bind="props"></v-btn>
        </template>

        <v-list density="compact">
          <v-list-item @click="">
            <v-icon left small>mdi-pencil</v-icon>
            <span class="ml-2">Éditer</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small>mdi-account-group</v-icon>
            <span class="ml-2">Gérer les groupes...</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small>mdi-sun-thermometer-outline</v-icon>
            <span class="ml-2">Gérer la météo...</span>
          </v-list-item>

          <v-list-item @click="visualiserCircuit">
            <v-icon left small color="green">mdi-video-image</v-icon>
            <span class="text-green-darken-3 ml-2">Visualiser</span>
          </v-list-item>
        </v-list>
      </v-menu>
    </v-card-title>

    <!-- Affichage des données principales (distance, dénivelé + badges d'état) -->
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
        <v-icon
          v-if="isSelected"
          color="orange"
          icon="mdi-star"
          size="small"
          @click="emitToggleFavorite"
          class="mr-1"
          title="Retirer des favoris"
        />
        <v-icon
          v-if="isDisplayed"
          color="blue"
          icon="mdi-map-check"
          size="small"
          @click="emitToggleDisplay"
          title="Masquer"
        />
      </div>
    </v-card-text>

    <!-- Section info déroulante (ouverte au hover, basculable au clic) -->
    <v-expand-transition>
      <div v-if="info" @click="info = false" style="cursor: pointer;" :class="computedBackgroundColor">
        <v-divider></v-divider>
        <v-card-text class="py-2 px-4">
          <!-- Première ligne : activité (gauche) + icônes d'action (droite) -->
          <div class="d-flex align-center mb-1">
            <span class="text-caption font-weight-bold">{{ trace.activity_type || '' }}</span>
            <v-spacer></v-spacer>
            <div class="d-flex align-center">
              <!-- Lien source -->
              <v-icon
                v-if="trace.source_url"
                icon="mdi-link-variant"
                size="small"
                class="mr-1"
                @click.stop="ouvrirSource"
                title="Ouvrir la source"
              />
              <!-- Favoris -->
              <v-icon
                :icon="isSelected ? 'mdi-star' : 'mdi-star-outline'"
                :color="isSelected ? 'orange' : ''"
                size="small"
                class="mr-1"
                @click.stop="emitToggleFavorite"
                :title="isSelected ? 'Retirer des favoris' : 'Ajouter aux favoris'"
              />
              <!-- Affichage -->
              <v-icon
                :icon="isDisplayed ? 'mdi-map-check' : 'mdi-map-check-outline'"
                :color="isDisplayed ? 'blue' : ''"
                size="small"
                class="mr-1"
                @click.stop="emitToggleDisplay"
                :title="isDisplayed ? 'Masquer' : 'Afficher'"
              />
              <!-- Exporter (non câblé) -->
              <v-icon
                icon="mdi-export"
                size="small"
                class="mr-1"
                title="Exporter"
              />
              <!-- Supprimer -->
              <v-icon
                icon="mdi-delete"
                color="red-darken-3"
                size="small"
                @click.stop="emitDelete"
                title="Supprimer"
              />
            </div>
          </div>

          <!-- Deuxième ligne : date d'import -->
          <div class="text-caption text-disabled mb-1">
            Importé le {{ formattedImportDate }}
          </div>

          <!-- Contenu existant conservé -->
          <b>{{ trace.name }}</b>
          <br />
          <span class="text-caption">Source : {{ trace.source }}</span>
          <br v-if="trace.source_url" />
          <span v-if="trace.source_url" class="text-caption text-blue">
            <v-icon size="x-small" icon="mdi-link-variant" /> {{ trace.source_url }}
          </span>
          <br v-if="trace.activity_type" />
          <span v-if="trace.activity_type" class="text-caption">
            Activité : {{ trace.activity_type }}
          </span>
          <br />
          <span class="text-caption">
            {{ trace.stats.points_count }} points
            | Dénivelé − : {{ formatElevation(trace.stats.negative_elevation_m) }}
            | Durée : {{ formattedDuration }}
          </span>
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
 * Refonte : section info ouvrable au hover (délai 300 ms), actions rapides
 * (favoris, affichage, export, suppression) déplacées dans la section info,
 * menu réduit aux actions de gestion avancée.
 */
import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { useAppStore } from '../../stores/app'
import type { TraceMetadata } from '../../stores/traces'
import { formatDistance, formatElevation, formatDuration } from '../../utils/format'

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

const info = ref(false)
let infoTimer: ReturnType<typeof setTimeout> | null = null

function toggleInfo() {
  info.value = !info.value

  if (infoTimer) clearTimeout(infoTimer)

  if (info.value) {
    infoTimer = setTimeout(() => {
      info.value = false
    }, 10000) // Fermeture automatique après 10 secondes
  }
}

// --- Gestion du hover ---

const isHovering = ref(false)
let hoverTimer: ReturnType<typeof setTimeout> | null = null

function onMouseEnter() {
  isHovering.value = true
  if (!info.value) {
    toggleInfo()
  }
}

function onMouseLeave() {
  isHovering.value = false
  hoverTimer = setTimeout(() => {
    if (!isHovering.value && info.value) {
      toggleInfo()
    }
  }, 300)
}

/** Nettoyer les timers au démontage du composant. */
onUnmounted(() => {
  if (infoTimer) clearTimeout(infoTimer)
  if (hoverTimer) clearTimeout(hoverTimer)
})

// --- État persisté (lié aux props via le backend) ---

const isSelected = computed(() => props.trace.favorite)
const isDisplayed = computed(() => props.trace.is_displayed)

// --- Formatted stats ---

const formattedDistance = computed(() => formatDistance(props.trace.stats.distance_m))
const formattedElevation = computed(() => formatElevation(props.trace.stats.positive_elevation_m))
const formattedDuration = computed(() => formatDuration(props.trace.stats.duration_s))

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

/** Classe CSS de la bande couleur, avec fallback sur la source. */
const computedBackgroundColor = computed(() => {
  if (props.backgroundColor) return props.backgroundColor
  // Attribution par défaut selon la source
  switch (props.trace.source) {
    case 'Strava': return 'bg-orange'
    case 'Garmin Connect': return 'bg-blue'
    case 'OpenRunner': return 'bg-green'
    case 'RideWithGPS': return 'bg-purple'
    default: return 'bg-grey'
  }
})

// sourceIcon conservé en cas de réutilisation future (tooltip source dans la section info)
const _sourceIcon = computed(() => {
  switch (props.trace.source) {
    case 'Strava': return 'mdi-run-fast'
    case 'Garmin Connect': return 'mdi-watch'
    case 'OpenRunner': return 'mdi-map-marker-path'
    case 'RideWithGPS': return 'mdi-bicycle'
    default: return null
  }
})
void _sourceIcon

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
</style>
