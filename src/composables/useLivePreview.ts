/**
 * Composable de live preview : synchronise la map 3D avec le curseur de
 * timeline et les overrides de montage (Mode 2, §10.1).
 *
 * Responsabilités :
 *  - Recalculer les keyframes finaux (fusion brute + overrides) dès qu'un
 *    override change → quasi instantané (la doc l'impose).
 *  - Appliquer `map.jumpTo` avec l'état caméra interpolé (LERP position,
 *    SLERP bearing) à chaque déplacement du curseur de temps.
 *
 * Le composable expose `finalKeyframes` (réactif) pour que la timeline et le
 * profile d'altitude puissent afficher les repères d'override sur les données
 * effectivement rendues.
 */

import { computed, watch, shallowRef, type ShallowRef, type Ref } from 'vue'
import type { Map as MbMap } from 'mapbox-gl'
import { useKeyframesStore } from '../stores/keyframes'
import { blendKeyframes, computeCamAt } from './useKeyframeEngine'
import type { RawKeyframesFile } from '../utils/keyframes'

/**
 * Branche le live preview sur une map Mapbox (réactive : la map peut être nulle
 * pendant l'init et devenir disponible plus tard).
 *
 * @param mapRef - ref (shallow) vers l'instance Mapbox, ou null tant que la map
 *                 n'est pas initialisée.
 * @returns `{ finalKeyframes }` — keyframes fusionnés, réactifs.
 */
export function useLivePreview(mapRef: ShallowRef<MbMap | null> | Ref<MbMap | null>) {
  const keyframesStore = useKeyframesStore()

  /** Keyframes finaux (bruts fusionnés avec les overrides actifs). Réactif. */
  const finalKeyframes = computed<RawKeyframesFile | null>(() => {
    const raw = keyframesStore.rawKeyframes
    if (!raw) return null
    return blendKeyframes(raw, keyframesStore.overrides.overrides)
  })

  /**
   * Applique l'état caméra interpolé à la map, à l'instant courant du curseur.
   * Sans effet si la map ou les keyframes finaux ne sont pas disponibles.
   */
  function applyCurrentFrame() {
    const map = mapRef.value
    const final = finalKeyframes.value
    if (!map || !final) return
    const state = computeCamAt(final, keyframesStore.currentTime)
    map.jumpTo({
      center: [state.cam.lng, state.cam.lat],
      zoom: state.cam.zoom,
      bearing: state.cam.bearing,
      pitch: state.cam.pitch,
    })
  }

  // Refusion + mise à jour de la map dès qu'un override change (deep) ou que
  // les keyframes bruts sont (re)chargés. `flush: 'post'` pour laisser le DOM
  // se stabiliser avant le jumpTo (cohérent avec le pattern Map.vue).
  watch(
    () => [keyframesStore.rawKeyframes, keyframesStore.overrides],
    () => applyCurrentFrame(),
    { deep: true, flush: 'post' },
  )

  // Mise à jour de la map quand le curseur de temps bouge (seek manuel ou lecture).
  watch(
    () => keyframesStore.currentTime,
    () => applyCurrentFrame(),
    { flush: 'post' },
  )

  return { finalKeyframes, applyCurrentFrame }
}

/** Factory pour exposer un shallowRef typé null initial (utile au consommateur). */
export function useMapRef(): ShallowRef<MbMap | null> {
  return shallowRef<MbMap | null>(null)
}
