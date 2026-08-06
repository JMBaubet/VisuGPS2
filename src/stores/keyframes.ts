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
 * Le stockage est côté Rust : un fichier `{mode_dir}/keyframes/{trace_id}.json`
 * par trace, écriture atomique (tmp + rename).
 */

import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { KeyframeSet } from '../algorithms/keyframeGenerator'

export const useKeyframesStore = defineStore('keyframes', () => {
  /**
   * Charge les keyframes persistés d'une trace depuis le disque.
   *
   * Valide la forme minimale du JSON (`trace_id` + `keyframes` non vide).
   * En cas de forme invalide, log et renvoie null (le frontend régénèrera).
   *
   * @returns Le KeyframeSet désérialisé, ou null si absent / invalide.
   */
  async function loadKeyframes(traceId: string): Promise<KeyframeSet | null> {
    try {
      const result = await invoke<KeyframeSet | null>('get_keyframes', {
        traceId,
      })

      if (result === null || result === undefined) return null

      // Validation minimale de la forme
      if (!result.trace_id || !Array.isArray(result.keyframes) || result.keyframes.length === 0) {
        console.warn(
          `[keyframes] JSON invalide pour la trace ${traceId} (trace_id ou keyframes manquants/vides).`,
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
   * Le frontend envoie le KeyframeSet complet ; Rust le sérialise en JSON
   * et l'écrit avec une écriture atomique (tmp + rename).
   */
  async function saveKeyframes(set: KeyframeSet): Promise<void> {
    await invoke('save_keyframes', {
      traceId: set.trace_id,
      keyframesJson: set, // sérialisé automatiquement par Tauri
    })
  }

  /**
   * Supprime le fichier keyframes d'une trace sur disque.
   * Tolérant si le fichier n'existe pas.
   */
  async function clearKeyframes(traceId: string): Promise<void> {
    await invoke('delete_keyframes', { traceId })
  }

  return {
    loadKeyframes,
    saveKeyframes,
    clearKeyframes,
  }
})
