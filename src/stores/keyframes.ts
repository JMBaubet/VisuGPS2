/**
 * Store Pinia pour la persistance des keyframes (vue d'édition caméra).
 *
 * Pattern Setup Store (cf. app.ts, traces.ts).
 *
 * Wrappe les 3 commandes Tauri de persistance :
 *   - `loadKeyframes` : charge les keyframes depuis le disque (ou null) ;
 *   - `saveKeyframes` : sauvegarde un jeu de keyframes sur disque ;
 *   - `clearKeyframes` : supprime le fichier keyframes d'une trace.
 *
 * Le stockage est côté Rust : **un fichier par trace et par ratio d'écran** —
 * `{mode_dir}/keyframes/{trace_id}_169.json` (16:9) et
 * `{mode_dir}/keyframes/{trace_id}_43.json` (4:3) — écriture atomique (tmp +
 * rename). Le ratio d'un jeu sauvegardé est **dérivé de son champ `viewport`**
 * (largeur 1440 → 4:3, sinon 16:9) : le bon fichier est ainsi toujours écrit,
 * sans risque de désynchronisation avec l'état courant.
 */

import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { VIEWPORTS_BY_ASPECT, type ViewportAspect } from '../algorithms/keyframeGenerator'
import type { KeyframeSet } from '../algorithms/keyframeGenerator'

/** Déduit le ratio d'écran d'un jeu keyframes à partir de son viewport. */
function aspectOfSet(set: KeyframeSet): ViewportAspect {
  return set.viewport.width === VIEWPORTS_BY_ASPECT['4:3'].width ? '4:3' : '16:9'
}

export const useKeyframesStore = defineStore('keyframes', () => {
  /**
   * Charge les keyframes persistés d'une trace pour un ratio d'écran donné
   * depuis le disque.
   *
   * Valide la forme minimale du JSON (`trace_id` + `keyframes` non vide).
   * En cas de forme invalide, log et renvoie null (le frontend régénèrera).
   *
   * @returns Le KeyframeSet désérialisé, ou null si absent / invalide.
   */
  async function loadKeyframes(
    traceId: string,
    viewportAspect: ViewportAspect,
  ): Promise<KeyframeSet | null> {
    try {
      const result = await invoke<KeyframeSet | null>('get_keyframes', {
        traceId,
        viewportAspect,
      })

      if (result === null || result === undefined) return null

      // Validation minimale de la forme
      if (!result.trace_id || !Array.isArray(result.keyframes) || result.keyframes.length === 0) {
        console.warn(
          `[keyframes] JSON invalide pour la trace ${traceId} (${viewportAspect}) (trace_id ou keyframes manquants/vides).`,
        )
        return null
      }

      return result
    } catch (error) {
      console.error(`[keyframes] Erreur de chargement pour la trace ${traceId} :`, error)
      return null
    }
  }

  /**
   * Sauvegarde un jeu de keyframes sur disque.
   *
   * Le ratio d'écran (et donc le fichier cible) est déduit du champ `viewport`
   * du jeu. Le frontend envoie le KeyframeSet complet ; Rust le sérialise en
   * JSON et l'écrit avec une écriture atomique (tmp + rename).
   */
  async function saveKeyframes(set: KeyframeSet): Promise<void> {
    await invoke('save_keyframes', {
      traceId: set.trace_id,
      viewportAspect: aspectOfSet(set),
      keyframesJson: set, // sérialisé automatiquement par Tauri
    })
  }

  /**
   * Supprime le fichier keyframes d'une trace pour un ratio donné sur disque.
   * Tolérant si le fichier n'existe pas.
   */
  async function clearKeyframes(traceId: string, viewportAspect: ViewportAspect): Promise<void> {
    await invoke('delete_keyframes', { traceId, viewportAspect })
  }

  return {
    loadKeyframes,
    saveKeyframes,
    clearKeyframes,
  }
})
