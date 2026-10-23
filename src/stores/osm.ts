/**
 * Store Pinia de la gestion des données OSM.
 *
 * Il expose l'état du support (inventaire, catalogue, espace disque) et les
 * actions utilisées par l'icône de la toolbar et la fenêtre de gestion. Les
 * commandes `osm_*` réalisent toutes les E/S disque ; le store ne fait que
 * porter les contrats IPC et la progression.
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type OsmState =
  | 'not_configured'
  | 'usb_missing'
  | 'empty'
  | 'green'
  | 'yellow'
  | 'orange'

export type OsmFileState = 'green' | 'yellow' | 'orange'
export type OsmDateSource = 'pbf' | 'created' | 'modified' | 'unknown'

export interface OsmFileStatus {
  filename: string
  regionId: string | null
  regionName: string | null
  countryCode: string | null
  countryName: string | null
  continent: string | null
  continentName: string | null
  sizeBytes: number
  createdAt: string | null
  dateSource: OsmDateSource
  ageDays: number
  renewalDays: number
  daysRemaining: number
  validityDaysRemaining: number | null
  state: OsmFileState
}

export interface OsmSupportOverview {
  configured: boolean
  readable: boolean
  usbMissing: boolean
  folder: string | null
  regionsPresent: boolean
  regionsVersion: string | null
  freeBytes: number | null
  totalBytes: number | null
  maxPbfMb: number
  maxValidityMonths: number
  renewalWeeks: number
  state: OsmState
  files: OsmFileStatus[]
  totalSizeBytes: number
  message: string | null
}

export interface OsmRegionOption {
  id: string
  name: string
  countryCode: string | null
  countryName: string | null
  continent: string | null
  continentName: string | null
  pbfFilename: string
  sizeMb: number | null
  present: boolean
  localSizeBytes: number | null
  localCreatedAt: string | null
}

export interface OsmRegionCatalog {
  present: boolean
  version: string | null
  regions: OsmRegionOption[]
}

export interface OsmRegionProbe {
  bytes: number | null
  availableBytes: number | null
}

export interface OsmRegionGeometry {
  regionId: string
  bbox: [number, number, number, number]
  geometry: GeoJSON.Geometry
  approximate: boolean
}

export interface OsmPresentRegionGeometry {
  regionId: string
  state: OsmFileState
  bbox: [number, number, number, number]
  geometry: GeoJSON.Geometry
  approximate: boolean
}

export interface OsmDownloadResult {
  filename: string
  sizeBytes: number
}

export interface OsmProgress {
  stage: 'catalog' | 'download'
  regionId: string | null
  regionName: string | null
  filename: string | null
  bytesDone: number
  bytesTotal: number
  percent: number
}

export interface OsmProbeState {
  regionId: string
  bytes: number | null
  availableBytes: number | null
  loading: boolean
  error: string | null
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export const useOsmStore = defineStore('osm', () => {
  // ─── State ──────────────────────────────────────────────────────────
  const overview = ref<OsmSupportOverview | null>(null)
  const catalog = ref<OsmRegionCatalog | null>(null)
  const loading = ref(false)
  const catalogLoading = ref(false)
  const probe = ref<OsmProbeState | null>(null)
  const downloading = ref(false)
  const downloadingRegionId = ref<string | null>(null)
  const progress = ref<OsmProgress | null>(null)
  const lastError = ref<string | null>(null)
  const isDialogOpen = ref(false)
  const previewGeometry = ref<GeoJSON.Geometry | null>(null)
  const previewBounds = ref<[number, number, number, number] | null>(null)
  const previewApproximate = ref(false)
  const previewRegionId = ref<string | null>(null)
  const presentRegions = ref<OsmPresentRegionGeometry[]>([])

  let progressBound = false
  let probeToken = 0
  let previewToken = 0

  // ─── Getters ────────────────────────────────────────────────────────
  const isBusy = computed(() => downloading.value || catalogLoading.value)

  // ─── Actions ────────────────────────────────────────────────────────
  async function refreshOverview(): Promise<void> {
    loading.value = true
    lastError.value = null
    try {
      overview.value = await invoke<OsmSupportOverview>('osm_support_overview')
      lastError.value = null
    } catch (error) {
      lastError.value = errorMessage(error)
      console.error('[osm] État du support illisible :', error)
    } finally {
      loading.value = false
    }
  }

  async function loadCatalog(): Promise<void> {
    catalogLoading.value = true
    try {
      catalog.value = await invoke<OsmRegionCatalog>('osm_region_catalog')
    } catch (error) {
      lastError.value = errorMessage(error)
      console.error('[osm] Catalogue OSM illisible :', error)
    } finally {
      catalogLoading.value = false
    }
  }

  async function refreshCatalog(): Promise<void> {
    catalogLoading.value = true
    progress.value = null
    lastError.value = null
    try {
      catalog.value = await invoke<OsmRegionCatalog>('osm_catalog_refresh')
      await refreshOverview()
    } catch (error) {
      lastError.value = errorMessage(error)
      throw error
    } finally {
      catalogLoading.value = false
      progress.value = null
    }
  }

  async function probeRegion(regionId: string): Promise<void> {
    const token = ++probeToken
    probe.value = {
      regionId,
      bytes: null,
      availableBytes: null,
      loading: true,
      error: null,
    }
    try {
      const result = await invoke<OsmRegionProbe>('osm_region_size', {
        regionId,
      })
      if (token !== probeToken) return
      probe.value = {
        regionId,
        bytes: result.bytes,
        availableBytes: result.availableBytes,
        loading: false,
        error: null,
      }
      if (result.availableBytes != null && overview.value) {
        overview.value = {
          ...overview.value,
          freeBytes: result.availableBytes,
        }
      }
    } catch (error) {
      if (token !== probeToken) return
      probe.value = {
        regionId,
        bytes: null,
        availableBytes: null,
        loading: false,
        error: errorMessage(error),
      }
    }
  }

  async function downloadRegion(
    regionId: string,
    renew: boolean,
    ignoreSizeLimit = false,
  ): Promise<OsmDownloadResult> {
    downloading.value = true
    downloadingRegionId.value = regionId
    progress.value = null
    lastError.value = null
    try {
      const result = await invoke<OsmDownloadResult>('osm_download', {
        regionId,
        renew,
        ignoreSizeLimit,
      })
      await Promise.all([refreshOverview(), loadCatalog()])
      return result
    } catch (error) {
      lastError.value = errorMessage(error)
      throw error
    } finally {
      downloading.value = false
      downloadingRegionId.value = null
      progress.value = null
    }
  }

  async function deleteFile(filename: string): Promise<void> {
    lastError.value = null
    try {
      await invoke('osm_delete', { filename })
      await Promise.all([refreshOverview(), loadCatalog()])
    } catch (error) {
      lastError.value = errorMessage(error)
      throw error
    }
  }

  async function bindProgress(): Promise<void> {
    if (progressBound) return
    progressBound = true
    try {
      await listen<OsmProgress>('osm-progress', event => {
        progress.value = event.payload
      })
    } catch (error) {
      progressBound = false
      console.error("[osm] Écoute de la progression impossible :", error)
    }
  }

  function openDialog(): void {
    isDialogOpen.value = true
  }

  function closeDialog(): void {
    isDialogOpen.value = false
    clearPreview()
    clearPresentRegions()
  }

  async function loadPresentRegions(): Promise<void> {
    try {
      presentRegions.value = await invoke<OsmPresentRegionGeometry[]>(
        'osm_present_region_geometries',
      )
    } catch (error) {
      presentRegions.value = []
      console.error(
        '[osm] Géométries des régions présentes illisibles :',
        error,
      )
    }
  }

  function clearPresentRegions(): void {
    presentRegions.value = []
  }

  async function loadRegionGeometry(regionId: string): Promise<void> {
    const token = ++previewToken
    try {
      const result = await invoke<OsmRegionGeometry | null>(
        'osm_region_geometry',
        { regionId },
      )
      if (token !== previewToken) return
      if (!result) {
        previewGeometry.value = null
        previewBounds.value = null
        previewApproximate.value = false
        previewRegionId.value = null
        return
      }
      previewBounds.value = result.bbox
      previewGeometry.value = result.geometry
      previewApproximate.value = result.approximate
      previewRegionId.value = regionId
    } catch (error) {
      if (token !== previewToken) return
      previewGeometry.value = null
      previewBounds.value = null
      previewApproximate.value = false
      previewRegionId.value = null
      console.error('[osm] Géométrie de région illisible :', error)
    }
  }

  function clearPreview(): void {
    previewToken += 1
    previewGeometry.value = null
    previewBounds.value = null
    previewApproximate.value = false
    previewRegionId.value = null
  }

  return {
    // State
    overview,
    catalog,
    loading,
    catalogLoading,
    probe,
    downloading,
    downloadingRegionId,
    progress,
    lastError,
    isDialogOpen,
    previewGeometry,
    previewBounds,
    previewApproximate,
    previewRegionId,
    presentRegions,
    // Getters
    isBusy,
    // Actions
    refreshOverview,
    loadCatalog,
    refreshCatalog,
    probeRegion,
    loadRegionGeometry,
    clearPreview,
    loadPresentRegions,
    clearPresentRegions,
    downloadRegion,
    deleteFile,
    bindProgress,
    openDialog,
    closeDialog,
  }
})
