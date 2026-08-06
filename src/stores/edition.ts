/**
 * Store Pinia de la vue d'édition caméra.
 *
 * Pattern Setup Store (cf. app.ts, traces.ts).
 *
 * Porte l'état de la phase d'édition (Phase 2 de la spec) :
 *   - trace sélectionnée et visibilité du cadre ViewPort 16:9 (MVP initial) ;
 *   - lecture (playback) : jeu de keyframes, lecture/pause, vitesse, temps
 *     courant, et getters d'interpolation caméra/marqueur consommés par la
 *     boucle d'animation (`EditionMap`) et le HUD (`TelemetryHud`).
 *
 * Aucune persistance : l'état est perdu au rechargement de la page (la vue
 * redirige alors vers l'accueil si aucune trace n'est sélectionnée).
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  type KeyframeSet,
  type CamState,
  type TraceurPoint,
  type PolyVertex,
  findSegment,
  interpolateCam,
  buildTracePolyline,
  buildTracePolylineFromPoints,
  samplePolylineAt,
  MS_PER_METER,
} from '../algorithms/keyframeGenerator'
import { bearing, bearingDelta, haversineMeters } from '../utils/geo'

/** Vitesse par défaut : 4000 ms/km (spec §7). Exposé pour l'affichage UI. */
export const DEFAULT_SPEED_MS_PER_KM = 4000

export const useEditionStore = defineStore('edition', () => {
  /**
   * Identifiant de la trace en cours d'édition, ou null.
   * Positionné par le bouton « Éditer » de Circuit.vue avant la
   * navigation vers la route `editionCamera`.
   */
  const selectedTraceId = ref<string | null>(null)

  /**
   * Visibilité du cadre ViewPort 16:9 (overlay CSS pur).
   * Le cadre représente la zone de rendu finale (export vidéo) ; il est
   * purement informatif et ne modifie pas le rendu MapBox.
   */
  const showViewportFrame = ref(false)

  // --- Lecture (playback) ---

  /** Jeu de keyframes courant, ou null tant qu'aucune trace n'est chargée. */
  const keyframeSet = ref<KeyframeSet | null>(null)

  /**
   * Polyligne de la trace indexée par distance cumulée (tous les points GPX).
   * Sert au déplacement du marker : celui-ci avance **le long de la trace
   * réelle**, pas entre les keyframes échantillonnés — sinon il tracerait
   * des cordes droites à travers les virages.
   */
  const tracePolyline = ref<PolyVertex[]>([])

  /** `true` si la lecture est en cours (boucle rAF active côté EditionMap). */
  const isPlaying = ref(false)

  /**
   * Multiplicateur de vitesse (0.5, 1, 2, 4). La durée « réelle » du
   * parcours est multipliée par `1 / speed` pendant la lecture.
   */
  const speed = ref(1)

  /** Temps absolu courant depuis le départ (ms), dans [0, total_duration_ms]. */
  const currentTimeMs = ref(0)

  // --- Getters ---

  const hasKeyframes = computed(() => !!keyframeSet.value)
  const totalDistanceM = computed(() => keyframeSet.value?.total_distance_m ?? 0)
  const totalDistanceKm = computed(() => totalDistanceM.value / 1000)
  const totalDurationMs = computed(() => keyframeSet.value?.total_duration_ms ?? 0)

  /** Distance cumulée courante le long de la trace (m). */
  const currentDistanceM = computed(() => currentTimeMs.value / MS_PER_METER)
  const currentDistanceKm = computed(() => currentDistanceM.value / 1000)

  /** Progression normalisée sur [0, 1]. */
  const progressRatio = computed(() =>
    totalDurationMs.value > 0
      ? Math.min(1, Math.max(0, currentTimeMs.value / totalDurationMs.value))
      : 0,
  )

  /** Segment encadrant le temps courant (ou null si pas de keyframes). */
  const currentSegment = computed(() => {
    const kf = keyframeSet.value?.keyframes
    if (!kf || kf.length === 0) return null
    return findSegment(kf, currentTimeMs.value)
  })

  /** État interpolé de la caméra à l'instant courant (ou null). */
  const interpolatedCam = computed<CamState | null>(() => {
    const seg = currentSegment.value
    return seg ? interpolateCam(seg) : null
  })

  /**
   * Position interpolée du traceur à l'instant courant (ou null).
   *
   * Échantillonnée **le long de la polyligne réelle** à la distance cumulée
   * courante (et non interpolée entre les keyframes), de sorte que le
   * marker suive fidèlement les virages de la trace au lieu de tirer des
   * cordes droites entre keyframes échantillonnés.
   */
  const interpolatedTraceur = computed<TraceurPoint | null>(() => {
    if (tracePolyline.value.length === 0) return null
    const p = samplePolylineAt(tracePolyline.value, currentDistanceM.value)
    if (!p) return null
    return { lng: p.lng, lat: p.lat, altitude: p.altitude }
  })

  /**
   * Distance (m) entre le point de vue caméra et le marqueur. Aux keyframes
   * la caméra est sur le traceur → ~0 ; entre deux keyframes le marqueur peut
   * dériver latéralement.
   */
  const markerDistanceM = computed(() => {
    const cam = interpolatedCam.value
    const tr = interpolatedTraceur.value
    if (!cam || !tr) return 0
    return haversineMeters(cam.lat, cam.lng, tr.lat, tr.lng)
  })

  /**
   * Cap (°) entre la direction de la caméra et la ligne caméra→marqueur,
   * normalisé sur [-180, 180]. Proche de 0 quand le marqueur est dans l'axe
   * de visée ; positif à droite, négatif à gauche.
   */
  const markerRelativeBearing = computed(() => {
    const cam = interpolatedCam.value
    const tr = interpolatedTraceur.value
    if (!cam || !tr) return 0
    const capToMarker = bearing(cam.lat, cam.lng, tr.lat, tr.lng)
    return bearingDelta(cam.bearing, capToMarker)
  })

  // --- Actions : sélection / cadre ViewPort ---

  function selectTrace(traceId: string) {
    selectedTraceId.value = traceId
  }

  function clearSelection() {
    selectedTraceId.value = null
  }

  function toggleViewportFrame() {
    showViewportFrame.value = !showViewportFrame.value
  }

  // --- Actions : lecture ---

  /**
   * Définit le jeu de keyframes et la polyligne de la trace associée.
   *
   * La polyligne sert au déplacement du marker le long de la trace réelle.
   * Deux sources possibles :
   *   - `tracePoints` (priorité) : points riches du backend avec altitude.
   *     Les distances cumulées sont recalculées en 2D (Haversine) pour
   *     rester cohérentes avec les keyframes (distances 2D). L'apport
   *     est l'altitude réelle par point pour le HUD.
   *   - `feature` (fallback) : GeoJSON LineString 2D → polyligne Haversine 2D,
   *     sans altitude.
   * Réinitialise le temps courant à 0 et met la lecture en pause.
   */
  function setKeyframeSet(
    set: KeyframeSet | null,
    feature: GeoJSON.Feature | null = null,
    tracePoints: { lat: number; lon: number; alt: number | null; distance_m: number }[] | null = null,
  ) {
    keyframeSet.value = set
    if (tracePoints && tracePoints.length > 0) {
      tracePolyline.value = buildTracePolylineFromPoints(tracePoints)
    } else {
      tracePolyline.value = feature ? buildTracePolyline(feature) : []
    }
    currentTimeMs.value = 0
    isPlaying.value = false
  }

  function play() {
    if (!keyframeSet.value) return
    // Si la lecture est terminée, repartir du début.
    if (currentTimeMs.value >= totalDurationMs.value) currentTimeMs.value = 0
    isPlaying.value = true
  }

  function pause() {
    isPlaying.value = false
  }

  function togglePlay() {
    if (isPlaying.value) pause()
    else play()
  }

  function setSpeed(s: number) {
    speed.value = s
  }

  /** Se positionne à une distance cumulée donnée (m), clampée à la trace. */
  function seekToDistance(distanceM: number) {
    const t = distanceM * MS_PER_METER
    currentTimeMs.value = Math.min(
      totalDurationMs.value,
      Math.max(0, t),
    )
  }

  /**
   * Avance le temps courant du delta écoulé (multiplié par la vitesse).
   * À appeler à chaque frame par la boucle d'animation.
   *
   * @param deltaMs - Temps réel écoulé depuis la frame précédente (ms).
   */
  function tick(deltaMs: number) {
    if (!isPlaying.value || !keyframeSet.value) return
    const next = currentTimeMs.value + deltaMs * speed.value
    if (next >= totalDurationMs.value) {
      currentTimeMs.value = totalDurationMs.value
      isPlaying.value = false // pause automatique à la fin
    } else {
      currentTimeMs.value = next
    }
  }

  return {
    // État : sélection / cadre
    selectedTraceId,
    showViewportFrame,
    // État : lecture
    keyframeSet,
    isPlaying,
    speed,
    currentTimeMs,
    // Getters
    hasKeyframes,
    totalDistanceM,
    totalDistanceKm,
    totalDurationMs,
    currentDistanceM,
    currentDistanceKm,
    progressRatio,
    currentSegment,
    interpolatedCam,
    interpolatedTraceur,
    markerDistanceM,
    markerRelativeBearing,
    // Actions : sélection / cadre
    selectTrace,
    clearSelection,
    toggleViewportFrame,
    // Actions : lecture
    setKeyframeSet,
    play,
    pause,
    togglePlay,
    setSpeed,
    seekToDistance,
    tick,
  }
})
