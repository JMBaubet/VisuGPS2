/**
 * Store Pinia de la vue d'édition caméra.
 *
 * Pattern Setup Store (cf. app.ts, traces.ts).
 *
 * Porte l'état de la phase d'édition (Phase 2 de la spec) :
 *   - trace sélectionnée, ratio d'écran de destination (16:9 / 4:3) et
 *     visibilité du cadre ViewPort ;
 *   - lecture (playback) : jeu de keyframes, lecture/pause, vitesse, temps
 *     courant, et getters d'interpolation caméra/marqueur consommés par la
 *     boucle d'animation (`EditionMap`) et le HUD (`TelemetryHud`).
 *
 * Aucune persistance : l'état est perdu au rechargement de la page (la vue
 * redirige alors vers l'accueil si aucune trace n'est sélectionnée).
 */

import { defineStore } from 'pinia'
import { ref, computed, watch, reactive } from 'vue'
import {
  type KeyframeSet,
  type Keyframe,
  type CamState,
  type TraceurPoint,
  type PolyVertex,
  type ViewportAspect,
  findSegment,
  interpolateCam,
  buildTracePolyline,
  buildTracePolylineFromPoints,
  samplePolylineAt,
  MS_PER_METER,
} from '../algorithms/keyframeGenerator'
import {
  computeHeadingChanges,
  type HeadingChange,
} from '../algorithms/headingChanges'
import { bearing, bearingDelta, haversineMeters } from '../utils/geo'
import { useKeyframesStore } from './keyframes'
import { useSettingsStore } from './settings'
import { isValidHexAlpha } from '../utils/materialColors'

/** Vitesse par défaut : 4000 ms/km (spec §7). Exposé pour l'affichage UI. */
export const DEFAULT_SPEED_MS_PER_KM = 4000

/** Tolérance (m) pour considérer le curseur « sur » un point de RdV. */
export const ON_KEYFRAME_EPSILON_M = 1

// --- Chemins des paramètres d'édition (panneau Paramètres, settings.default.toml) ---

const SETTING_ALGORITHM = 'Edition.Generation.algorithme'
const SETTING_GAP_MIN = 'Edition.Generation.gapMin'
const SETTING_ZOOM_DEFAULT = 'Edition.Camera.zoomDefaut'
const SETTING_PITCH_DEFAULT = 'Edition.Camera.pitchDefaut'
const SETTING_VIEWPORT_DEFAULT = 'Edition.Camera.viewportDefaut'
const SETTING_FLY_TO_DURATION = 'Edition.Camera.dureeFlyTo'
const SETTING_SHOW_TELEMETRY = 'Edition.Camera.afficherTelemetrie'
const SETTING_SHOW_HEADING_CHANGES = 'Edition.Camera.afficherCapBrutaux'
const SETTING_PLAYBACK_ACCELERATION = 'Edition.Playback.acceleration'
const SETTING_TRACE_COLOR = 'Edition.Couleurs.trace'
const SETTING_CURSOR_COLOR = 'Edition.Couleurs.curseur'
const SETTING_TRACE_WIDTH = 'Edition.Couleurs.epaisseurTrace'

export const useEditionStore = defineStore('edition', () => {
  /**
   * Identifiant de la trace en cours d'édition, ou null.
   * Positionné par le bouton « Éditer » de Circuit.vue avant la
   * navigation vers la route `editionCamera`.
   */
  const selectedTraceId = ref<string | null>(null)

  /**
   * Visibilité du cadre ViewPort (overlay CSS pur).
   * Le cadre représente la zone de rendu finale (export vidéo) ; il est
   * purement informatif et ne modifie pas le rendu MapBox. Affiché par défaut
   * (16:9).
   */
  const showViewportFrame = ref(true)

  /**
   * `true` quand la vue d'édition est prête à être **révélée** : la carte est
   * positionnée au point de départ (km 0) et les tuiles visibles sont
   * chargées. Tant que c'est `false`, la carte reste masquée (écran vierge)
   * et la vue affiche le curseur d'attente système.
   */
  const editionViewReady = ref(false)

  /**
   * Ratio d'écran de destination : détermine le fichier keyframes exploité
   * (`{trace_id}_169.json` / `{trace_id}_43.json`) **et** le cadre ViewPort
   * affiché. Défaut : 16:9. Un changement charge/génère le jeu du ratio choisi.
   */
  const viewportAspect = ref<ViewportAspect>('16:9')

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
   * Facteur d'accélération de la lecture (paramètre
   * `Edition.Playback.acceleration`), appliqué quand le mode accéléré est
   * actif. Bornes 1.5–8. Défaut : 2.
   */
  const acceleration = ref(2)

  /** `true` si la lecture accélérée (bouton double chevron) est active. */
  const accelerated = ref(false)

  /**
   * Vitesse de lecture effective : 1× en lecture normale, sinon le facteur
   * d'accélération (`acceleration`). La durée « réelle » du parcours est
   * multipliée par `1 / speed` pendant la lecture.
   */
  const speed = computed(() => (accelerated.value ? acceleration.value : 1))

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
   * Seuil de détection des changements de cap « brutaux » (taux de rotation en
   * °/km). Bornes 10–1000, pas de 5. Défaut : 45. Simple **filtre d'affichage**
   * (timeline + tableau) : ne régénère pas les keyframes.
   */
  const headingChangeThresholdDegPerKm = ref(45)

  // --- Paramètres d'édition (panneau Paramètres de la vue) ---

  /**
   * Zoom par défaut de la caméra (paramètre `Edition.Camera.zoomDefaut`).
   * Appliqué aux nouveaux keyframes générés et valeur de référence du reset des
   * sliders du CameraEditor. Défaut : 16.0.
   */
  const defaultZoom = ref(16)

  /**
   * Pitch par défaut de la caméra (paramètre `Edition.Camera.pitchDefaut`).
   * Même usage que `defaultZoom`. Défaut : 60.
   */
  const defaultPitch = ref(60)

  /**
   * Couleur de la LineString de la trace (paramètre `Edition.Couleurs.trace`),
   * au format #RRGGBBAA. Défaut : rouge Material 500.
   */
  const traceColor = ref('#F44336FF')

  /**
   * Couleur du curseur d'avancement (paramètre `Edition.Couleurs.curseur`) —
   * marqueur carte + curseur timeline. Format #RRGGBBAA. Défaut : jaune A400.
   */
  const cursorColor = ref('#FFD600FF')

  /**
   * Épaisseur de la LineString de la trace (paramètre
   * `Edition.Couleurs.epaisseurTrace`), en pixels. Bornes 1–20. Défaut : 4.
   */
  const traceWidth = ref(4)

  /**
   * Durée de l'animation caméra quand la position du curseur change
   * (paramètre `Edition.Camera.dureeFlyTo`), en ms. Bornes 100–1000.
   * Défaut : 250.
   */
  const flyToDurationMs = ref(250)

  /**
   * Visibilité du HUD de télémétrie (paramètre
   * `Edition.Camera.afficherTelemetrie`). Masqué par défaut.
   */
  const showTelemetryHud = ref(false)

  /** Visibilité du panneau « Changements de cap brutaux » (tableau à la demande). */
  const showHeadingChangesPanel = ref(false)

  /**
   * Mode **validation** : pendant la lecture, un clic sur la carte signale un
   * « problème » sur le segment courant (il reste déverrouillé) ; les segments
   * traversés sans clic sont **verrouillés** automatiquement. Bascule via le
   * bouton `mdi-camera-lock` de la toolbar.
   */
  const validationMode = ref(false)

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

  // --- Changements de cap (analyse des virages entre keyframes consécutifs) ---

  /** Tous les changements de cap (un par segment entre keyframes consécutifs). */
  const headingChanges = computed<HeadingChange[]>(() =>
    computeHeadingChanges(keyframeSet.value?.keyframes ?? []),
  )

  /** Sous-ensemble « brutal » : taux de rotation ≥ seuil réglable. */
  const brutalHeadingChanges = computed(() =>
    headingChanges.value.filter(
      h => h.rateDegPerKm >= headingChangeThresholdDegPerKm.value,
    ),
  )

  // --- Verrous de segments (mode validation) ---

  /**
   * Distances (m) du keyframe de départ des segments **verrouillés**, dérivées
   * des flags `locked` portés par chaque keyframe (verrou du segment qui en
   * part). Set construit pour les recherches O(1) des consommateurs.
   */
  const lockedSegmentFromDistances = computed<Set<number>>(() => {
    const set = new Set<number>()
    for (const kf of keyframeSet.value?.keyframes ?? []) {
      if (kf.locked) set.add(kf.distance_from_start_m)
    }
    return set
  })

  /** Distance (m) du keyframe de départ du segment courant (ou null). */
  const currentSegmentFromDistance = computed<number | null>(() => {
    const seg = currentSegment.value
    return seg ? seg.prev.distance_from_start_m : null
  })

  /** `true` si le segment partant de `fromDistanceM` est verrouillé. */
  function isSegmentLocked(fromDistanceM: number): boolean {
    return lockedSegmentFromDistances.value.has(fromDistanceM)
  }

  /**
   * `true` si le keyframe à `distanceM` **borde** un segment verrouillé (il est
   * le `from` d'un segment verrouillé, ou le `to` du segment précédent). Un tel
   * keyframe n'est pas modifiable.
   */
  function isKeyframeLocked(distanceM: number): boolean {
    const kfs = keyframeSet.value?.keyframes ?? []
    const idx = kfs.findIndex(k => k.distance_from_start_m === distanceM)
    if (idx === -1) return false
    return kfs[idx].locked === true || (idx > 0 && kfs[idx - 1].locked === true)
  }

  // --- Actions : sélection / cadre ViewPort ---

  /**
   * Sélectionne la trace à éditer (appelé par le bouton Éditer de l'accueil
   * avant la navigation). À chaque **ouverture** de la vue d'édition :
   *   - le mode validation repart **désactivé** (même s'il était actif lors
   *     d'une session précédente) ;
   *   - la vue repart en « écran vierge » (`editionViewReady` false) — l'icône
   *     ViewPort est alors neutre jusqu'au chargement de la trace.
   * Les verrous persistés des segments ne sont pas touchés.
   */
  function selectTrace(traceId: string) {
    selectedTraceId.value = traceId
    validationMode.value = false
    validationClicks.clear()
    previousSegmentFromDistance = null
    suppressAutoLock = false
    editionViewReady.value = false
  }

  function clearSelection() {
    selectedTraceId.value = null
  }

  function toggleViewportFrame() {
    showViewportFrame.value = !showViewportFrame.value
  }

  /**
   * Change le ratio d'écran de destination et **affiche le cadre ViewPort**
   * (choisir un ratio, c'est vouloir le voir). La sélection du keyframe en
   * cours est remise à zéro car le jeu de keyframes va être rechargé/généré
   * pour le nouveau ratio (EditionMap écoute ce changement).
   */
  function setViewportAspect(aspect: ViewportAspect) {
    if (viewportAspect.value === aspect) return
    viewportAspect.value = aspect
    showViewportFrame.value = true
    selectedKeyframeDistance.value = null
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
    // Les marques transitoires du mode validation référencent les segments de
    // l'ancien jeu (régénération, changement de ratio) : on les réinitialise.
    validationClicks.clear()
    if (tracePoints && tracePoints.length > 0) {
      tracePolyline.value = buildTracePolylineFromPoints(tracePoints)
    } else {
      tracePolyline.value = feature ? buildTracePolyline(feature) : []
    }
    currentTimeMs.value = 0
    isPlaying.value = false
    // Nouveau jeu de keyframes : état de validation reparti à zéro (le segment
    // « précédent » de l'ancien jeu n'existe plus).
    previousSegmentFromDistance = null
    suppressAutoLock = false
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
    // Verrouillage : un keyframe bordant un segment verrouillé n'est pas modifiable.
    if (isKeyframeLocked(distanceM)) return
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
    // Verrouillage : on ne peut pas insérer un keyframe dans un segment verrouillé.
    if (distanceM < kfs[0].distance_from_start_m) return
    const insertSeg = findSegment(kfs, distanceM * MS_PER_METER)
    if (isSegmentLocked(insertSeg.prev.distance_from_start_m)) return

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
    // Verrouillage : un keyframe bordant un segment verrouillé n'est pas supprimable.
    if (isKeyframeLocked(distanceM)) return
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
    // Si la lecture est terminée, repartir du début (repositionnement neutre).
    if (currentTimeMs.value >= totalDurationMs.value) {
      currentTimeMs.value = 0
      suppressAutoLock = true
    }
    isPlaying.value = true
  }

  function pause() {
    isPlaying.value = false
  }

  function togglePlay() {
    if (isPlaying.value) pause()
    else play()
  }

  /** Bascule la lecture accélérée (bouton double chevron). */
  function toggleAccelerated() {
    accelerated.value = !accelerated.value
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

  /**
   * Applique les paramètres d'édition du panneau Paramètres au store.
   *
   * Lit les valeurs par dotted path dans `settingsStore.settings` (avec repli
   * sur les défauts) et les répercute sur l'état. `includeViewport` ne vaut
   * `true` qu'à **l'ouverture** de la vue : le « viewport par défaut » fixe
   * alors le ratio initial. En cours de session (`false`), le viewport n'est
   * **pas** touché ici — un watch dédié sur la valeur persistée du paramètre
   * (EditionCamera.vue) bascule le ratio actif quand **c'est ce paramètre** qui
   * change, sans écraser le flip-flop ViewPort à la sauvegarde d'un autre
   * paramètre.
   *
   * Algorithme / gap min : les watchers d'EditionMap déclenchent la
   * régénération. Couleurs / zoom / pitch : consommés réactivement.
   */
  function applySettings(includeViewport: boolean) {
    const settings = useSettingsStore()
    const read = (path: string, fallback: unknown) =>
      settings.settings.find(d => d.path === path)?.value ?? fallback

    const algo = read(SETTING_ALGORITHM, 'frustum')
    if (algo === 'frustum' || algo === 'simple') keyframeAlgorithm.value = algo

    const gap = Number(read(SETTING_GAP_MIN, 1000))
    if (!Number.isNaN(gap)) minKeyframeGapM.value = Math.min(5000, Math.max(200, gap))

    const zoom = Number(read(SETTING_ZOOM_DEFAULT, 16))
    if (!Number.isNaN(zoom)) defaultZoom.value = Math.min(22, Math.max(1, zoom))

    const pitch = Number(read(SETTING_PITCH_DEFAULT, 60))
    if (!Number.isNaN(pitch)) defaultPitch.value = Math.min(85, Math.max(0, pitch))

    const trace = read(SETTING_TRACE_COLOR, '#F44336FF')
    if (typeof trace === 'string' && isValidHexAlpha(trace)) traceColor.value = trace

    const cursor = read(SETTING_CURSOR_COLOR, '#FFD600FF')
    if (typeof cursor === 'string' && isValidHexAlpha(cursor)) cursorColor.value = cursor

    const width = Number(read(SETTING_TRACE_WIDTH, 4))
    if (!Number.isNaN(width)) traceWidth.value = Math.min(20, Math.max(1, width))

    const fly = Number(read(SETTING_FLY_TO_DURATION, 250))
    if (!Number.isNaN(fly)) flyToDurationMs.value = Math.min(1000, Math.max(100, fly))

    const accel = Number(read(SETTING_PLAYBACK_ACCELERATION, 2))
    if (!Number.isNaN(accel)) acceleration.value = Math.min(8, Math.max(1.5, accel))

    // Viewport / affichages (télémétrie, caps) : appliqués à l'ouverture
    // (`includeViewport`) — la valeur initiale. En cours de session, les mises à
    // jour live sont pilotées par des watchers dédiés sur les valeurs persistées
    // (cf. EditionCamera.vue), pour ne pas écraser les bascules manuelles
    // (flip-flop ViewPort, boutons Fermer) quand un autre paramètre change.
    if (includeViewport) {
      const vp = read(SETTING_VIEWPORT_DEFAULT, '16:9')
      if (vp === '16:9' || vp === '4:3') viewportAspect.value = vp
      const telemetry = read(SETTING_SHOW_TELEMETRY, false)
      showTelemetryHud.value = telemetry === true
      const hcp = read(SETTING_SHOW_HEADING_CHANGES, false)
      showHeadingChangesPanel.value = hcp === true
    }
  }

  /**
   * Change le seuil de détection des changements de cap « brutaux » (°/km).
   * Clampé à [10, 1000] et arrondi au pas de 5. Simple filtre d'affichage :
   * aucune régénération des keyframes.
   */
  function setHeadingChangeThreshold(rate: number) {
    const clamped = Math.min(1000, Math.max(10, rate))
    const stepped = Math.round(clamped / 5) * 5
    if (headingChangeThresholdDegPerKm.value === stepped) return
    headingChangeThresholdDegPerKm.value = stepped
  }

  /** Affiche / masque le panneau « Changements de cap brutaux ». */
  function toggleHeadingChangesPanel() {
    showHeadingChangesPanel.value = !showHeadingChangesPanel.value
  }

  /** Fixe l'affichage du panneau « Changements de cap brutaux » (paramètre). */
  function setHeadingChangesPanel(visible: boolean) {
    showHeadingChangesPanel.value = visible
  }

  /** Fixe l'affichage du HUD de télémétrie (fermeture via son bouton). */
  function setTelemetryHud(visible: boolean) {
    showTelemetryHud.value = visible
  }

  // --- Mode validation & verrous de segments ---

  /**
   * Segments « signalés » pendant le **parcours courant** en mode validation
   * (touche Entrée ou clic carte). Set transitoire : sert uniquement de garde
   * anti re-verrouillage automatique du watcher. Les traits bleus, eux, sont
   * **persistés** dans `Keyframe.marks` (fichier keyframes).
   */
  const validationClicks = reactive(new Set<number>())
  /** Dernier segment parcouru (pour verrouiller à la sortie). */
  let previousSegmentFromDistance: number | null = null
  /**
   * `true` pendant un **seek manuel** (clic timeline, navigation RdV, km0,
   * animation fly-to) : l'auto-verrouillage est alors **neutre** — aucun
   * verrouillage ni consommation de marque. Levé à la prochaine avancée
   * naturelle `tick()`.
   */
  let suppressAutoLock = false

  /** Bascule le mode validation (réinitialise l'état transitoire). */
  function toggleValidationMode() {
    validationMode.value = !validationMode.value
    validationClicks.clear()
    previousSegmentFromDistance = currentSegmentFromDistance.value
    suppressAutoLock = false
  }

  /**
   * En mode validation : signale un « problème » sur le segment courant — il
   * reste **déverrouillé dans tous les cas** : on mémorise le segment (il ne
   * sera pas re-verrouillé à sa sortie pendant **ce parcours**) **et** on le
   * déverrouille explicitement s'il était déjà verrouillé. Un trait bleu
   * **persisté** est posé à la distance du curseur d'avancement (plusieurs
   * possibles par segment). Appelé par un clic sur la carte ou la touche Entrée.
   */
  function markValidationClick() {
    if (!validationMode.value) return
    const d = currentSegmentFromDistance.value
    if (d === null) return
    const set = keyframeSet.value
    const kf = set?.keyframes.find(k => k.distance_from_start_m === d)
    if (!set || !kf) return
    if (!kf.marks) kf.marks = []
    kf.marks.push(currentDistanceM.value)
    validationClicks.add(d)
    if (isSegmentLocked(d)) unlockSegment(d)
    saveKeyframes()
  }

  /** Verrouille le segment partant de `fromDistanceM` et persiste. */
  function lockSegment(fromDistanceM: number) {
    const set = keyframeSet.value
    if (!set) return
    const kf = set.keyframes.find(k => k.distance_from_start_m === fromDistanceM)
    if (!kf || kf.locked) return
    kf.locked = true
    // Un segment verrouillé n'est plus « signalé » : ses traits bleus
    // persistés sont supprimés et sa protection de parcours levée.
    delete kf.marks
    validationClicks.delete(fromDistanceM)
    saveKeyframes()
  }

  /**
   * Déverrouille le segment partant de `fromDistanceM` et persiste.
   *
   * Le déverrouillage manuel protège le segment du **re-verrouillage
   * automatique** — uniquement pendant **ce parcours** (la garde est consommée
   * à la sortie par le watcher) : sans cela, naviguer vers un autre segment
   * (le seek du double-clic) ferait « quitter » ce segment au watcher, qui le
   * reverrouillerait aussitôt. Aucune marque persistée n'est posée ici
   * (contrairement à l'Entrée / clic carte).
   */
  function unlockSegment(fromDistanceM: number) {
    validationClicks.add(fromDistanceM)
    const set = keyframeSet.value
    if (!set) return
    const kf = set.keyframes.find(k => k.distance_from_start_m === fromDistanceM)
    if (!kf?.locked) return
    kf.locked = false
    saveKeyframes()
  }

  /** Bascule l'état du verrou d'un segment (appui long sur la timeline RdV). */
  function toggleSegmentLock(fromDistanceM: number) {
    if (isSegmentLocked(fromDistanceM)) unlockSegment(fromDistanceM)
    else lockSegment(fromDistanceM)
  }

  // En validation : quand le curseur quitte un segment —
  //   - sans marque posée pendant **ce parcours** → le segment est verrouillé
  //     et ses traits bleus persistés sont supprimés ;
  //   - avec une marque posée pendant ce parcours → il reste déverrouillé, et
  //     la marque est **consommée** (elle ne protège plus les parcours
  //     suivants : un re-parcours sans Entrée le reverrouillera).
  // Un **seek manuel** (clic timeline, navigation RdV, km0, animation fly-to —
  // signal `suppressAutoLock`) est totalement **neutre** : ni verrouillage,
  // ni consommation de marque. Seule la progression naturelle de la lecture
  // (`tick`) traverse et verrouille.
  watch(currentSegmentFromDistance, (newFrom) => {
    if (!validationMode.value) return
    const prev = previousSegmentFromDistance
    previousSegmentFromDistance = newFrom
    if (prev === null || prev === newFrom) return
    if (suppressAutoLock) return
    if (!validationClicks.has(prev)) {
      lockSegment(prev)
    }
    validationClicks.delete(prev)
  })

  /** Se positionne à une distance cumulée donnée (m), clampée à la trace. */
  function seekToDistance(distanceM: number) {
    // Seek manuel : **neutre** pour le mode validation — les segments
    // traversés ne sont pas verrouillés. Le signal est levé jusqu'à la
    // prochaine avancée naturelle `tick()` (et couvre aussi les frames de
    // l'animation fly-to qui suivent).
    suppressAutoLock = true
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
    // La progression naturelle de la lecture réactive l'auto-verrouillage
    // après un seek manuel : les traversées suivantes verrouillent à nouveau.
    suppressAutoLock = false
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
    editionViewReady,
    viewportAspect,
    // État : lecture
    keyframeSet,
    isPlaying,
    speed,
    acceleration,
    accelerated,
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
    // État : analyse des changements de cap
    headingChanges,
    brutalHeadingChanges,
    headingChangeThresholdDegPerKm,
    showHeadingChangesPanel,
    // État : verrous de segments (mode validation)
    validationMode,
    lockedSegmentFromDistances,
    currentSegmentFromDistance,
    // État : édition keyframes
    selectedKeyframeDistance,
    // État : algorithme de génération
    keyframeAlgorithm,
    minKeyframeGapM,
    // État : paramètres d'édition (panneau Paramètres)
    defaultZoom,
    defaultPitch,
    traceColor,
    cursorColor,
    traceWidth,
    flyToDurationMs,
    showTelemetryHud,
    // Actions : sélection / cadre
    selectTrace,
    clearSelection,
    toggleViewportFrame,
    setViewportAspect,
    // Actions : paramètres d'édition
    applySettings,
    // Actions : analyse des changements de cap
    setHeadingChangeThreshold,
    toggleHeadingChangesPanel,
    setHeadingChangesPanel,
    setTelemetryHud,
    // Actions : mode validation & verrous
    toggleValidationMode,
    markValidationClick,
    toggleSegmentLock,
    isSegmentLocked,
    isKeyframeLocked,
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
    toggleAccelerated,
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
