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
  type Keyframe,
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
import { useKeyframesStore } from './keyframes'

/** Vitesse par défaut : 4000 ms/km (spec §7). Exposé pour l'affichage UI. */
export const DEFAULT_SPEED_MS_PER_KM = 4000

/** Tolérance (m) pour considérer le curseur « sur » un point de RdV. */
export const ON_KEYFRAME_EPSILON_M = 1

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

  /**
   * Algorithme de génération des keyframes : `'frustum'` (placement récursif
   * par visibilité, spec §3.3) ou `'simple'` (échantillonnage régulier MVP).
   * Défaut : frustum. Un changement force la régénération du jeu.
   */
  const keyframeAlgorithm = ref<'frustum' | 'simple'>('frustum')

  /**
   * Distance minimale entre keyframes (m) pour l'algorithme frustum
   * (anti-surabondance). Bornes 200–5000, pas de 50. Défaut : 1000.
   */
  const minKeyframeGapM = ref(1000)

  /**
   * Distance (m) du keyframe sélectionné pour l'édition (Composant B), ou
   * `null` si aucun keyframe n'est en cours d'édition. Alimenté par le clic
   * sur un tick RdV de la timeline ou par les boutons RdV précédent/suivant.
   */
  const selectedKeyframeDistance = ref<number | null>(null)

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

  /**
   * Fonction retournant l'altitude (m) à une distance donnée le long de la
   * polyligne réelle, ou `null` si non disponible.
   *
   * Exposée comme computed (closure) plutôt que getter paramétré, car Pinia
   * ne supporte pas les getters à arguments. La closure capture la
   * tracePolyline réactive : le résultat se met à jour automatiquement quand
   * la polyligne change (nouvelle trace chargée).
   */
  const altitudeAtDistance = computed<(m: number) => number | null>(() => {
    const poly = tracePolyline.value
    return (distanceM: number): number | null => {
      if (poly.length === 0) return null
      const p = samplePolylineAt(poly, distanceM)
      return p?.altitude ?? null
    }
  })

  /**
   * Keyframe sélectionné pour l'édition (Composant B), ou `null`.
   * Résolu par `selectedKeyframeDistance` contre le jeu de keyframes courant.
   */
  const selectedKeyframe = computed(() => {
    if (selectedKeyframeDistance.value === null) return null
    const kfs = keyframeSet.value?.keyframes ?? []
    return (
      kfs.find(k => k.distance_from_start_m === selectedKeyframeDistance.value) ?? null
    )
  })

  /**
   * Keyframe situé à la position courante du curseur (dans une tolérance),
   * ou `null` si le curseur n'est pas sur un point de RdV.
   *
   * C'est ce getter qui pilote le mode du CameraEditor : sur un RdV on
   * affiche les widgets d'édition ; hors RdV on affiche le bouton « Ajouter
   * un point de RdV ».
   */
  const currentKeyframe = computed(() => {
    const kfs = keyframeSet.value?.keyframes ?? []
    const dist = currentDistanceM.value
    return (
      kfs.find(k => Math.abs(k.distance_from_start_m - dist) <= ON_KEYFRAME_EPSILON_M) ??
      null
    )
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
    // Sélectionner le premier keyframe pour rendre l'éditeur (Composant B)
    // immédiatement utilisable au chargement.
    selectedKeyframeDistance.value = set?.keyframes[0]?.distance_from_start_m ?? null
  }

  // --- Actions : édition des keyframes (Composant B) ---

  /** Timer de debounce de la sauvegarde automatique. */
  let saveTimer: ReturnType<typeof setTimeout> | null = null

  /** Sauvegarde le jeu de keyframes courant sur disque (best-effort, debounce). */
  function scheduleSave() {
    const set = keyframeSet.value
    if (!set) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      void useKeyframesStore().saveKeyframes(set).catch(() => {
        // Échec best-effort : silencieux (la sauvegarde initiale l'est aussi).
      })
    }, 300)
  }

  /** Sélectionne un keyframe (par sa distance) pour l'édition. */
  function selectKeyframe(distanceM: number) {
    const exists = keyframeSet.value?.keyframes.some(
      k => k.distance_from_start_m === distanceM,
    )
    selectedKeyframeDistance.value = exists ? distanceM : null
  }

  /**
   * Met à jour les champs `cam` d'un keyframe identifié par sa distance.
   * Les champs fournis sont écrasés (Partial), les autres conservés.
   *
   * La sauvegarde est **explicite** (bouton Sauvegarder du CameraEditor) :
   * cette action ne persiste pas elle-même, elle marque seulement l'état en
   * mémoire (le bouton Sauvegarder est dégrisé dès qu'une modification
   * intervient).
   */
  function updateKeyframe(distanceM: number, updates: Partial<CamState>) {
    const kf = keyframeSet.value?.keyframes.find(
      k => k.distance_from_start_m === distanceM,
    )
    if (!kf) return
    Object.assign(kf.cam, updates)
  }

  /**
   * Sauvegarde **immédiate** du jeu de keyframes courant sur disque
   * (best-effort). Appelée par le bouton Sauvegarder du CameraEditor.
   */
  function saveKeyframes() {
    const set = keyframeSet.value
    if (!set) return
    void useKeyframesStore().saveKeyframes(set).catch(() => {
      // Échec best-effort : silencieux.
    })
  }

  /**
   * Insère un keyframe à la distance donnée.
   *
   * Si `cam`/`traceur` ne sont pas fournis, ils sont interpolés depuis les
   * keyframes voisins (l'insertion ne modifie donc pas la trajectoire).
   * Si un keyframe existe déjà à cette distance, la demande est ignorée.
   */
  function addKeyframe(
    distanceM: number,
    cam?: CamState,
    traceur?: TraceurPoint,
  ) {
    const set = keyframeSet.value
    if (!set) return
    const kfs = set.keyframes
    if (kfs.some(k => k.distance_from_start_m === distanceM)) return

    // Interpolation depuis les voisins si valeurs non fournies.
    let resolvedCam = cam
    let resolvedTraceur = traceur
    if (!resolvedCam || !resolvedTraceur) {
      const time = distanceM * MS_PER_METER
      const seg = findSegment(kfs, time)
      if (!resolvedCam) resolvedCam = interpolateCam(seg)
      if (!resolvedTraceur) {
        const prev = seg.prev.traceur
        const next = seg.next.traceur
        resolvedTraceur = {
          lng: prev.lng + (next.lng - prev.lng) * seg.ratio,
          lat: prev.lat + (next.lat - prev.lat) * seg.ratio,
          altitude:
            prev.altitude !== null && next.altitude !== null
              ? prev.altitude + (next.altitude - prev.altitude) * seg.ratio
              : null,
        }
      }
    }

    const kf = {
      time_ms: distanceM * MS_PER_METER,
      distance_from_start_m: distanceM,
      cam: resolvedCam,
      traceur: resolvedTraceur!,
    }
    // Insérer en conservant le tri par distance croissante.
    const index = kfs.findIndex(k => k.distance_from_start_m > distanceM)
    if (index === -1) kfs.push(kf)
    else kfs.splice(index, 0, kf)

    // Bornes mises à jour si le keyframe est en extrémité.
    set.total_distance_m = Math.max(set.total_distance_m, distanceM)
    set.total_duration_ms = Math.max(set.total_duration_ms, kf.time_ms)

    selectKeyframe(distanceM)
    scheduleSave()
  }

  /**
   * Supprime un keyframe identifié par sa distance.
   * Le point de départ (km 0, premier keyframe) ne peut pas être supprimé.
   * Au minimum 2 keyframes sont conservés (début + fin de trace).
   * Si le keyframe supprimé était sélectionné, la sélection est effacée.
   */
  function removeKeyframe(distanceM: number) {
    const set = keyframeSet.value
    if (!set || set.keyframes.length <= 2) return
    // Le point de départ (km 0) est intouchable.
    if (distanceM === set.keyframes[0].distance_from_start_m) return
    const index = set.keyframes.findIndex(k => k.distance_from_start_m === distanceM)
    if (index === -1) return
    set.keyframes.splice(index, 1)
    if (selectedKeyframeDistance.value === distanceM) {
      selectedKeyframeDistance.value = null
    }
    scheduleSave()
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

  /**
   * Change l'algorithme de génération des keyframes et force la régénération
   * (EditionMap écoute ce changement). La sélection du keyframe est remise à
   * zéro car le jeu de keyframes va être reconstruit.
   */
  function setKeyframeAlgorithm(algo: 'frustum' | 'simple') {
    if (keyframeAlgorithm.value === algo) return
    keyframeAlgorithm.value = algo
    selectedKeyframeDistance.value = null
  }

  /**
   * Change la distance minimale entre keyframes (m) pour l'algorithme frustum.
   * Clampée à [200, 5000] et arrondie au pas de 50. Force la régénération.
   */
  function setMinKeyframeGapM(m: number) {
    const clamped = Math.min(5000, Math.max(200, m))
    const stepped = Math.round(clamped / 50) * 50
    if (minKeyframeGapM.value === stepped) return
    minKeyframeGapM.value = stepped
    selectedKeyframeDistance.value = null
  }

  /** Se positionne à une distance cumulée donnée (m), clampée à la trace. */
  function seekToDistance(distanceM: number) {
    const t = distanceM * MS_PER_METER
    currentTimeMs.value = Math.min(
      totalDurationMs.value,
      Math.max(0, t),
    )
  }

  // --- Navigation entre points de RdV ---

  /**
   * Distance en dessous de laquelle on considère qu'on est déjà sur un RdV
   * (pour passer au RdV précédent/suivant **de ce point**).
   */
  const RDV_EPSILON_M = 0.5

  /** Point de RdV suivant (strictement après la position courante), ou null. */
  const nextRdv = computed(() => {
    const kfs = keyframeSet.value?.keyframes ?? []
    const cur = currentDistanceM.value
    return kfs.find(k => k.distance_from_start_m > cur + RDV_EPSILON_M) ?? null
  })

  /** Point de RdV précédent (strictement avant la position courante), ou null. */
  const prevRdv = computed(() => {
    const kfs = keyframeSet.value?.keyframes ?? []
    const cur = currentDistanceM.value
    let prev: Keyframe | undefined
    for (const k of kfs) {
      if (k.distance_from_start_m < cur - RDV_EPSILON_M) prev = k
      else break
    }
    return prev ?? null
  })

  /** `true` si un point de RdV suivant existe (grise le bouton sinon). */
  const canGoNextRdv = computed(() => nextRdv.value !== null)

  /** `true` si un point de RdV précédent existe (grise le bouton sinon). */
  const canGoPrevRdv = computed(() => prevRdv.value !== null)

  /**
   * Va au point de RdV suivant et le sélectionne pour l'édition.
   * **Met en pause** si la lecture est active.
   */
  function goToNextRdv() {
    const next = nextRdv.value
    if (!next) return
    if (isPlaying.value) pause()
    seekToDistance(next.distance_from_start_m)
    selectKeyframe(next.distance_from_start_m)
  }

  /**
   * Va au point de RdV précédent et le sélectionne pour l'édition.
   * **Met en pause** si la lecture est active.
   */
  function goToPrevRdv() {
    const prev = prevRdv.value
    if (!prev) return
    if (isPlaying.value) pause()
    seekToDistance(prev.distance_from_start_m)
    selectKeyframe(prev.distance_from_start_m)
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
    altitudeAtDistance,
    selectedKeyframe,
    currentKeyframe,
    // État : édition keyframes
    selectedKeyframeDistance,
    // État : algorithme de génération
    keyframeAlgorithm,
    minKeyframeGapM,
    // Actions : sélection / cadre
    selectTrace,
    clearSelection,
    toggleViewportFrame,
    // Actions : édition keyframes
    selectKeyframe,
    updateKeyframe,
    saveKeyframes,
    addKeyframe,
    removeKeyframe,
    // Actions : lecture
    setKeyframeSet,
    play,
    pause,
    togglePlay,
    setSpeed,
    setKeyframeAlgorithm,
    setMinKeyframeGapM,
    seekToDistance,
    tick,
    // Navigation entre points de RdV
    canGoNextRdv,
    canGoPrevRdv,
    goToNextRdv,
    goToPrevRdv,
  }
})
