<template>
  <div v-if="editionStore.showTelemetryHud && editionStore.hasKeyframes" class="telemetry-hud">
    <div class="hud-title">Télémétrie</div>

    <!-- Paramètres caméra -->
    <div class="hud-section">
      <div class="hud-section-title">Caméra</div>
      <div class="hud-row"><span>Zoom</span><span>{{ fmt(cam?.zoom, 1) }}</span></div>
      <div class="hud-row"><span>Pitch</span><span>{{ fmt(cam?.pitch, 0) }}°</span></div>
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
  pointer-events: none;
  user-select: none;
}

.hud-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  opacity: 0.7;
  margin-bottom: 6px;
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
