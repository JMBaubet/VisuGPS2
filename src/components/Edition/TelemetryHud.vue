<template>
  <div v-if="editionStore.showTelemetryHud && editionStore.hasKeyframes" class="telemetry-hud">
    <!-- En-tête : titre + bouton fermer (comme le panneau des caps brutaux) -->
    <div class="hud-header">
      <span class="hud-title">Télémétrie</span>
      <v-spacer />
      <v-btn
        icon
        size="x-small"
        variant="text"
        title="Fermer"
        @click="editionStore.setTelemetryHud(false)"
      >
        <v-icon>mdi-close</v-icon>
      </v-btn>
    </div>

    <!-- Paramètres caméra -->
    <div class="hud-section">
      <div class="hud-section-title">Caméra</div>
      <div class="hud-row"><span>Zoom</span><span :style="{ color: zoomColor }">{{ fmt(cam?.zoom, 1) }}</span></div>
      <div class="hud-row"><span>Pitch</span><span :style="{ color: pitchColor }">{{ fmt(cam?.pitch, 0) }}°</span></div>
      <div class="hud-row"><span>Bearing</span><span>{{ fmt(cam?.bearing, 0) }}°</span></div>
      <div class="hud-row"><span>Lng</span><span>{{ cam ? cam.lng.toFixed(4) : '—' }}</span></div>
      <div class="hud-row"><span>Lat</span><span>{{ cam ? cam.lat.toFixed(4) : '—' }}</span></div>
    </div>

    <!-- Traceur -->
    <div class="hud-section">
      <div class="hud-section-title">Traceur</div>
      <div class="hud-row"><span>Alt</span><span>{{ fmtAlt(editionStore.interpolatedTraceur?.altitude) }}</span></div>
    </div>

    <!-- Relation caméra ↔ marqueur -->
    <div class="hud-section">
      <div class="hud-section-title">Caméra ↔ marqueur</div>
      <div class="hud-row"><span>Distance</span><span>{{ Math.round(editionStore.markerDistanceM) }} m</span></div>
      <div class="hud-row"><span>Cap</span><span>{{ fmt(editionStore.markerRelativeBearing, 0) }}°</span></div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Composant C — HUD de télémétrie (spec §4.5).
 *
 * Overlay coin supérieur droit, fond semi-transparent sombre (~80 %),
 * texte blanc. Affiche en temps réel les paramètres de la caméra
 * (Zoom/Pitch/Bearing/Lng/Lat) et la relation caméra ↔ marqueur
 * (distance, cap relatif), lus depuis `editionStore`.
 *
 * **Masqué par défaut** : la visibilité est pilotée par le paramètre
 * `Edition.Camera.afficherTelemetrie` (bool, défaut false). Affiché
 * uniquement si ce paramètre est actif **et** qu'un jeu de keyframes est chargé.
 */
import { computed } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

const cam = computed(() => editionStore.interpolatedCam)

/**
 * Couleurs des lignes Zoom / Pitch, alignées sur la timeline des points de RdV :
 * vert quand la valeur est celle par défaut (paramètres `zoomDefaut` /
 * `pitchDefaut`), orange (pitch ≠ défaut) / rouge (zoom ≠ défaut) sinon.
 * Les valeurs sont interpolées : on compare avec une tolérance (moitié du pas
 * de réglage) pour absorber la dérive flottante du lerp en cours de lecture.
 */
const DEFAULT_COLOR = '#4CAF50' // vert : valeur par défaut (pitch ET zoom)
const PITCH_COLOR = '#FF9800'   // orange : pitch ≠ défaut
const ZOOM_COLOR = '#F44336'    // rouge : zoom ≠ défaut

const zoomColor = computed(() =>
  cam.value && Math.abs(cam.value.zoom - editionStore.defaultZoom) < 0.05
    ? DEFAULT_COLOR
    : ZOOM_COLOR,
)

const pitchColor = computed(() =>
  cam.value && Math.abs(cam.value.pitch - editionStore.defaultPitch) < 0.5
    ? DEFAULT_COLOR
    : PITCH_COLOR,
)

/** Formate un nombre avec `n` décimales ; renvoie '—' si indéfini. */
function fmt(value: number | undefined, decimals: number): string {
  return value === undefined || value === null || Number.isNaN(value)
    ? '—'
    : value.toFixed(decimals)
}

/** Formate une altitude en mètres arrondis avec séparateur de milliers. */
function fmtAlt(alt: number | null | undefined): string {
  return alt === undefined || alt === null || Number.isNaN(alt)
    ? '—'
    : `${Math.round(alt).toLocaleString('fr-FR')} m`
}
</script>

<style scoped>
.telemetry-hud {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 6;
  min-width: 210px;
  padding: 10px 12px;
  background: rgba(0, 0, 0, 0.8);
  color: #ffffff;
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.5;
  font-family: monospace;
  /* Interactions nécessaires pour le bouton Fermer (comme le panneau caps). */
  pointer-events: auto;
  user-select: none;
}

.hud-header {
  display: flex;
  align-items: center;
}

.hud-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  opacity: 0.7;
  margin-bottom: 0;
}

.hud-section {
  margin-bottom: 8px;
}

.hud-section:last-child {
  margin-bottom: 0;
}

.hud-section-title {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  opacity: 0.55;
  margin-bottom: 2px;
}

.hud-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.hud-row span:first-child {
  opacity: 0.75;
}

.hud-row span:last-child {
  font-weight: 600;
}
</style>
