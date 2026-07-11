/**
 * Store Pinia des traces GPX importées.
 *
 * Pattern Setup Store (cf. app.ts, settings.ts).
 * Toute communication avec le disque passe par les commandes Tauri `get_traces`
 * et `import_gpx_file`, car seul le backend connaît le mode d'exécution actif
 * et donc le dossier de stockage approprié.
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { haversineMeters } from '../utils/geo'

// --- Types (miroir exact des structs Rust import_gpx.rs, snake_case) ---

/** Coordonnées géographiques d'un point (latitude, longitude, altitude). */
export interface Point3D {
  lat: number
  lon: number
  alt: number | null
}

/** Statistiques calculées à partir des points de la trace. */
export interface TraceStats {
  start_point: Point3D
  end_point: Point3D
  distance_m: number
  positive_elevation_m: number
  negative_elevation_m: number
  alt_min_m: number | null
  alt_max_m: number | null
  points_count: number
  duration_s: number | null
}

/** Métadonnées complètes d'une trace importée. */
export interface TraceMetadata {
  /** Identifiant unique (UUID v4). */
  id: string
  /** Nom de la trace. */
  name: string
  /** Éditeur / plateforme source (Strava, Garmin Connect, etc.). */
  source: string
  /** URL source si détectée. */
  source_url: string | null
  /** Type d'activité (running, cycling, hiking…). */
  activity_type: string | null
  /** Nom du fichier stocké dans le dossier gpx/. */
  filename: string
  /** Date et heure d'import (ISO 8601 UTC). */
  import_date: string
  /** Statistiques calculées. */
  stats: TraceStats
  /** Empreinte SHA256 du fichier original, préfixée "sha256:". */
  hash: string
  /** Trace marquée comme favorite. */
  favorite: boolean
  /** Trace affichée sur la carte. */
  is_displayed: boolean
}

// --- Store ---

export const useTracesStore = defineStore('traces', () => {
  // État réactif
  const traces = ref<TraceMetadata[]>([])
  const loading = ref(false)
  /** Centre courant de la carte (mis à jour par Map.vue sur moveend). */
  const mapCenter = ref<{ lat: number; lon: number }>({ lat: 43.7, lon: 2.0 })

  // Getters
  const traceCount = computed(() => traces.value.length)

  /** Traces triées par distance croissante au centre courant de la carte. */
  const sortedTracesByDistance = computed(() =>
    [...traces.value]
      .map(t => ({
        trace: t,
        distance: haversineMeters(
          mapCenter.value.lat,
          mapCenter.value.lon,
          t.stats.start_point.lat,
          t.stats.start_point.lon,
        ),
      }))
      .sort((a, b) => a.distance - b.distance)
      .map(entry => entry.trace),
  )

  /**
   * Charge la liste des traces depuis le backend.
   * Le backend lit le registre traces.json du mode d'exécution actif.
   */
  async function loadTraces() {
    loading.value = true
    try {
      traces.value = await invoke<TraceMetadata[]>('get_traces')
    } catch (error) {
      console.error('Impossible de charger les traces :', error)
      traces.value = []
    } finally {
      loading.value = false
    }
  }

  /**
   * Importe un fichier GPX via la commande Tauri.
   * Le backend ouvre un sélecteur natif, parse le fichier, copie le GPX dans
   * le dossier du mode actif, et met à jour le registre.
   *
   * Lève une erreur (string) en cas de doublon, fichier invalide, ou annulation.
   */
  async function importerGpx(): Promise<TraceMetadata> {
    loading.value = true
    try {
      const result = await invoke<TraceMetadata>('import_gpx_file')
      // Recharger la liste depuis le backend (source de vérité)
      await loadTraces()
      return result
    } finally {
      loading.value = false
    }
  }

  /**
   * Supprime une trace par son identifiant.
   * Le backend supprime le fichier GPX et l'entrée du registre.
   * Après suppression, recharge la liste depuis le backend.
   */
  async function supprimerTrace(traceId: string): Promise<void> {
    loading.value = true
    try {
      await invoke('delete_trace', { traceId })   // camelCase TS → trace_id Rust
      await loadTraces()                           // recharger (source de vérité = backend)
    } finally {
      loading.value = false
    }
  }

  /**
   * Met à jour partiellement une trace (favorite, affichage).
   * Seuls les champs fournis dans le patch sont modifiés.
   * Après mise à jour, recharge la liste depuis le backend.
   */
  async function updateTrace(
    traceId: string,
    patch: { favorite?: boolean; is_displayed?: boolean },
  ): Promise<void> {
    loading.value = true
    try {
      await invoke('update_trace', {
        traceId,                                   // camelCase TS → trace_id Rust
        favorite: patch.favorite ?? null,           // Option<bool> Rust → null si absent
        isDisplayed: patch.is_displayed ?? null,    // camelCase TS → is_displayed Rust
      })
      await loadTraces()
    } finally {
      loading.value = false
    }
  }

  /**
   * Met à jour le centre de la carte.
   * Appelé par Map.vue sur moveend pour synchroniser le tri par distance.
   */
  function updateMapCenter(lat: number, lon: number) {
    mapCenter.value = { lat, lon }
  }

  // Exposition publique
  return {
    // État
    traces,
    loading,
    mapCenter,
    // Getters
    traceCount,
    sortedTracesByDistance,
    // Actions
    loadTraces,
    importerGpx,
    supprimerTrace,
    updateTrace,
    updateMapCenter,
  }
})
