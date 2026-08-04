/**
 * Composable de lecture (Mode 2, preview interne à l'éditeur — distinct du
 * futur Mode 3 « Relecture finale »).
 *
 * Boucle `requestAnimationFrame` qui :
 *  - avance `currentTime` selon `playbackSpeed` et le `speed_multiplier` de
 *    l'override actif à l'instant courant (§12.4) ;
 *  - déplace le marqueur traceur sur la carte à chaque frame ;
 *  - met à jour la trace déjà parcourue (LineString dynamique).
 *
 * Le marqueur traceur et la couche trace sont ajoutés à la map par le composant
 * `Map3D.vue` (méthode `setupPlaybackLayers`), puis pilotés ici via les
 * callbacks `onMarkerMove` et `onTrailUpdate`.
 */

import { watch, onUnmounted } from 'vue'
import { useKeyframesStore } from '../stores/keyframes'
import { computeCamAt, blendKeyframes } from './useKeyframeEngine'

export interface PlaybackCallbacks {
  /** Déplace le marqueur traceur aux coords [lng, lat] données (alt en m). */
  onMarkerMove?: (lng: number, lat: number, altitude: number | null) => void
  /** Met à jour la trace parcourue (coords [lng, lat][] depuis le départ). */
  onTrailUpdate?: (coords: [number, number][]) => void
}

/**
 * Branche la lecture sur le store `keyframes` et appelle les callbacks à
 * chaque frame pour déplacer le marqueur traceur et mettre à jour la trace.
 *
 * La map n'est pas directement manipulée ici : le composant hôte (Map3D)
 * gère les couches et expose les mutations via les callbacks. Cela garde ce
 * composable indépendant d'une instance Mapbox.
 */
export function usePlayback(callbacks: PlaybackCallbacks = {}) {
  const keyframesStore = useKeyframesStore()

  /** Identifiant de frame courant (pour cancellation). */
  let rafId: number | null = null
  /** Timestamp du précédent tick (ms, performance.now). */
  let lastTs = 0

  /**
   * Vitesse effective à l'instant courant : `playbackSpeed` du store multiplié
   * par le `speed_multiplier` de l'override actif (s'il y en a un).
   */
  function effectiveSpeedAt(time: number): number {
    let mult = 1
    const active = keyframesStore.overrides.overrides.filter(
      o => !o.disabled && time >= o.start_time && time <= o.end_time,
    )
    for (const ov of active) {
      if (ov.params.speed_multiplier !== undefined) {
        mult *= ov.params.speed_multiplier
      }
    }
    return keyframesStore.playbackSpeed * mult
  }

  /** Boucle de lecture appelée à chaque frame. */
  function tick(ts: number) {
    if (!keyframesStore.isPlaying) {
      rafId = null
      return
    }
    if (lastTs === 0) lastTs = ts
    const dt = ts - lastTs
    lastTs = ts

    // Avancer le curseur selon la vitesse effective. La trace avance plus
    // lentement si un override ralentit (speed_multiplier < 1).
    const speed = effectiveSpeedAt(keyframesStore.currentTime)
    const next = keyframesStore.currentTime + dt * speed

    if (next >= keyframesStore.totalDuration) {
      // Fin de la trace : arrêter en fin de course.
      keyframesStore.seek(keyframesStore.totalDuration)
      keyframesStore.pause()
      updateMarkerAndTrail()
      rafId = null
      return
    }

    keyframesStore.seek(next)
    updateMarkerAndTrail()
    rafId = requestAnimationFrame(tick)
  }

  /** Met à jour le marqueur traceur et la trace parcourue à l'instant courant. */
  function updateMarkerAndTrail() {
    const raw = keyframesStore.rawKeyframes
    if (!raw) return
    const blended = blendKeyframes(raw, keyframesStore.overrides.overrides)
    const state = computeCamAt(blended, keyframesStore.currentTime)

    callbacks.onMarkerMove?.(
      state.traceur.lng,
      state.traceur.lat,
      state.traceur.altitude,
    )

    // Trace parcourue : tous les points des keyframes jusqu'au curseur.
    if (callbacks.onTrailUpdate) {
      const trail: [number, number][] = []
      for (const kf of blended.keyframes) {
        if (kf.time > keyframesStore.currentTime) break
        trail.push([kf.traceur.lng, kf.traceur.lat])
      }
      callbacks.onTrailUpdate?.(trail)
    }
  }

  /** Démarre la boucle de lecture (appelé quand isPlaying passe à true). */
  function start() {
    if (rafId !== null) return
    lastTs = 0
    rafId = requestAnimationFrame(tick)
  }

  /** Arrête la boucle (appelé quand isPlaying passe à false ou au démontage). */
  function stop() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
    lastTs = 0
  }

  // Démarrer/arrêter selon l'état de lecture du store.
  watch(
    () => keyframesStore.isPlaying,
    (playing) => {
      if (playing) start()
      else stop()
    },
  )

  // Nettoyage au démontage du composant hôte.
  onUnmounted(stop)

  return { start, stop }
}
