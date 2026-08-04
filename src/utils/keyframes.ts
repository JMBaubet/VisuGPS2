/**
 * Types des fichiers de keyframes et d'overrides de montage.
 *
 * Miroir exact (snake_case) des structs Rust du module `src-tauri/src/edition.rs`,
 * conformément à la convention appliquée pour `TraceMetadata` / `TraceStats` /
 * `Point3D` (cf. `src/stores/traces.ts`). Tauri effectue automatiquement la
 * conversion camelCase TS ↔ snake_case Rust.
 */

// --- Types partagés ---

/**
 * État complet de la caméra à un instant donné (image clé).
 *
 * `lng`/`lat` est le centre de la caméra, issu de l'algorithme de zone morte
 * et **jamais** modifié par les overrides de montage (règle §6.2 de la spec) :
 * seule la position reste pilotée automatiquement pour garder le traceur dans
 * le cadre. Les overrides agissent sur `zoom`/`pitch`/`bearing`.
 */
export interface CamState {
  lng: number
  lat: number
  zoom: number
  bearing: number
  pitch: number
}

/**
 * Position du traceur (sujet suivi) à un instant donné.
 * `altitude` est issue de `queryTerrainElevation` (Mapbox, source primaire)
 * avec repli sur l'altitude GPX si la requête a renvoyé `null`.
 */
export interface TraceurState {
  lng: number
  lat: number
  altitude: number | null
}

/**
 * Image clé : enregistrement de l'état de la caméra et du traceur à un
 * instant `time` (en millisecondes depuis le départ).
 */
export interface Keyframe {
  time: number
  cam: CamState
  traceur: TraceurState
}

/**
 * Dimensions du viewport de référence utilisé lors du pré-calcul.
 *
 * Le pré-calcul force une résolution canonique (défaut 1920×1080) via
 * `map.resize()` afin que les calculs de zone morte (qui utilisent
 * `map.project()` → pixels) soient indépendants de l'écran réel. À la
 * lecture, `jumpTo` reproduit le même cadrage géographique sur n'importe
 * quel écran (vidéoprojecteur inclus) : un écran plus large révèle
 * simplement plus de contexte périphérique sans décaler le traceur.
 */
export interface ReferenceViewport {
  width: number
  height: number
}

/**
 * Contenu du fichier `{traceId}_raw_keyframes.json`.
 *
 * Produit par le pré-calcul (Mode 1), lu par le moteur de fusion puis par
 * l'atelier d'édition. `total_duration` et `total_distance` permettent de
 * graduer la timeline sans reparcourir les keyframes.
 */
export interface RawKeyframesFile {
  trace_id: string
  total_duration: number
  total_distance: number
  reference_viewport: ReferenceViewport
  sample_rate: number
  keyframes: Keyframe[]
}

// --- Overrides de montage ---

/**
 * Fonction de lissage d'une transition, appliquée aux bords de la plage d'un
 * override pour éviter les cassures brutales entre la valeur brute et la
 * valeur surchargée.
 */
export type EasingType = 'linear' | 'smoothstep' | 'easeInOut'

/**
 * Paramètres d'un override de montage caméra.
 *
 * Toutes les valeurs sont optionnelles : seules celles présentes sont
 * appliquées. Les valeurs absolues (`zoom`, `pitch`, `bearing`) écrasent la
 * valeur brute ; les offsets relatifs (`*_offset`) s'y ajoutent. Un override
 * peut combiner les deux ou n'utiliser que l'un des deux.
 */
export interface OverrideParams {
  zoom?: number
  pitch?: number
  bearing?: number
  zoom_offset?: number
  bearing_offset?: number
  pitch_offset?: number
  speed_multiplier?: number
}

/**
 * Override de montage : modification artistique ciblée sur une plage
 * temporelle `[start_time, end_time]` (en millisecondes).
 *
 * `easing` lisse la transition entre la valeur brute et la valeur override
 * aux bords de la plage. `damping` (entre 0 et 1) ajoute un effet d'inertie.
 * `disabled` permet de comparer le rendu en désactivant temporairement un
 * override sans le supprimer (§10.4).
 */
export interface Override {
  id: string
  name: string
  start_time: number
  end_time: number
  params: OverrideParams
  easing: EasingType
  damping?: number
  disabled: boolean
}

/**
 * Contenu du fichier `{traceId}_montage_overrides.json`.
 *
 * Les tableaux `messages` et `pois` sont anticipés vides : la phase 1 ne
 * couvre que les overrides caméra. Ils sont typés en `unknown[]` pour rester
 * agnostiques du schéma futur (pas d'interface à maintenir tant que les
 * fonctionnalités ne sont pas implémentées).
 */
export interface MontageOverridesFile {
  overrides: Override[]
  messages: unknown[]
  pois: unknown[]
}

// --- Helpers de factory ---

/**
 * Crée un override vide (valeurs par défaut), prêt à être rempli par
 * l'UI d'édition puis sauvegardé.
 */
export function createEmptyOverride(
  startTime: number,
  endTime: number,
  name = 'Nouvelle séquence',
): Override {
  return {
    id: generateOverrideId(),
    name,
    start_time: startTime,
    end_time: endTime,
    params: {},
    easing: 'smoothstep',
    disabled: false,
  }
}

/**
 * Génère un identifiant stable et unique pour un override.
 * Format `ovr_<timestamp>_<random>` (stable au sein d'une session).
 */
export function generateOverrideId(): string {
  return `ovr_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}
