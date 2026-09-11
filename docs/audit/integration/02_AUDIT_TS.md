# Livrable 2 — Contrat TypeScript `audit.ts`

**Fichier cible** : `src/stores/audit.ts`

**Rôle** : store Pinia du module Audit GPX (setup store, conforme à
`docs/CONVENTIONS.md`). Miroir exact des structs Rust de `types.rs`
(Livrable 1) et point d'entrée unique pour les commandes Tauri.

**Pattern** : setup store (fonction + refs + computed + actions), pas de
syntaxe options. Imports relatifs, TypeScript strict.

**Point de contrat** : le store ne fait **aucun calcul métier**. Il délègue
tout le travail lourd aux commandes Tauri (Livrable 4) et se contente de :
- conserver l'état (points, findings, sélection)
- exposer des getters dérivés
- orchestrer les appels Tauri

---

## Contenu du fichier

```typescript
// src/stores/audit.ts
//
// Store Pinia du module Audit GPX.
// Miroir exact des structs Rust définies dans src-tauri/src/gpx_audit/types.rs.
// Pattern Setup Store (conforme à docs/CONVENTIONS.md).

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// ─── Types primaires ──────────────────────────────────────────────────

export type FindingKind = 'ar' | 'rp'
export type FindingStatus = 'pending' | 'corrected' | 'fp'
export type CorrectionType = 'delete' | 'route-car' | 'route-bike'
export type PartRole = 'warn' | 'info'

// ─── Structures ───────────────────────────────────────────────────────

export interface AuditPoint {
  id: number
  lat: number
  lon: number
  ele?: number | null
}

export interface LatLon {
  lat: number
  lon: number
}

export interface FindingPair {
  aid: number
  bid: number
  a: number
  b: number
  d: number
}

export interface FindingPart {
  s: number
  e: number
  role: PartRole
  text: string
}

export interface FindingContext {
  up: number | null
  dn: number | null
}

export interface FindingContextIds {
  up: number | null
  dn: number | null
}

export interface UndoDelete {
  type: 'delete'
  origPts: AuditPoint[]
  anchorLeftId: number | null
  anchorRightId: number | null
  firstNo: number
  absorbedFp: Finding[]
}

export interface UndoRoute {
  type: 'route'
  origPts: AuditPoint[]
  insertedIds: number[]
  routePts: LatLon[]
  startPt: LatLon
  endPt: LatLon
  firstNo: number
  absorbedFp: Finding[]
}

export type UndoRecord = UndoDelete | UndoRoute

export interface Finding {
  id: string
  kind: FindingKind
  label: string
  summary: string
  peak: number
  peakId: number
  pairs: FindingPair[]
  pairIdx: number[]

  // AR uniquement
  ecart: number | null
  d1: number | null
  d2: number | null

  // RP uniquement
  totalAngle: number | null
  turnText: string | null
  coreIds: number[]

  zoneIds: number[]
  ctxIds: FindingContextIds
  ctx: FindingContext
  parts: FindingPart[]

  status: FindingStatus
  correction: CorrectionType | null
  undo: UndoRecord | null

  // Champ technique côté front (non sérialisé par Rust)
  // Utilisé pour le centrage de la carte
  _bounds?: unknown // LngLatBounds Mapbox
}

export interface AuditParams {
  consolM: number
  tolDeg: number
  pairM: number
  maxpairs: number
  segM: number
  closeM: number
  angleDeg: number
}

export interface AuditDetectionResult {
  traceId: string
  points: AuditPoint[]
  totalDistanceM: number
  findings: Finding[]
  params: AuditParams
  durationMs: number
}

export interface AuditState {
  points: AuditPoint[]
  findings: Finding[]
}

// ─── Store ────────────────────────────────────────────────────────────

export const useAuditStore = defineStore('audit', () => {
  // ─── State ─────────────────────────────────────────────────────
  const currentTraceId = ref<string | null>(null)
  const working = ref<AuditPoint[]>([])
  const findings = ref<Finding[]>([])
  const params = ref<AuditParams | null>(null)
  const selectedFindingId = ref<string | null>(null)
  const nextPointId = ref<number>(0)
  const analysisDurationMs = ref<number>(0)

  // ─── Getters ───────────────────────────────────────────────────
  const pendingCount = computed(
    () => findings.value.filter((f) => f.status === 'pending').length,
  )
  const correctedCount = computed(
    () => findings.value.filter((f) => f.status === 'corrected').length,
  )
  const fpCount = computed(
    () => findings.value.filter((f) => f.status === 'fp').length,
  )
  const allProcessed = computed(() => pendingCount.value === 0)
  const hasWorkInProgress = computed(() =>
    findings.value.some((f) => f.status !== 'pending'),
  )
  const selectedFinding = computed(
    () => findings.value.find((f) => f.id === selectedFindingId.value) ?? null,
  )
  const canApply = computed(
    () => hasWorkInProgress.value && allProcessed.value,
  )
  const totalFindings = computed(() => findings.value.length)

  // ─── Actions ───────────────────────────────────────────────────

  /**
   * Lance la détection (AR + RP) sur la trace donnée.
   * Remplace intégralement l'état courant.
   */
  async function runAudit(traceId: string, p: AuditParams): Promise<void> {
    const result = await invoke<AuditDetectionResult>('audit_run_detection', {
      traceId,
      params: p,
    })
    currentTraceId.value = traceId
    working.value = result.points
    findings.value = result.findings
    params.value = result.params
    analysisDurationMs.value = result.durationMs
    selectedFindingId.value = null
    // nextPointId initialisé au max + 1 pour les futures insertions
    nextPointId.value =
      working.value.length > 0
        ? Math.max(...working.value.map((p) => p.id)) + 1
        : 0
  }

  /**
   * Supprime une plage de points et met à jour le finding associé.
   * Rust valide la garde d'imbrication et la longueur restante.
   */
  async function applyDelete(
    findingId: string,
    ds: number,
    de: number,
  ): Promise<void> {
    if (!currentTraceId.value) {
      throw new Error('Aucune trace en cours d\'audit.')
    }
    const state = await invoke<AuditState>('audit_apply_delete', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
      ds,
      de,
      nextPointId: nextPointId.value,
    })
    working.value = state.points
    findings.value = state.findings
    nextPointId.value =
      working.value.length > 0
        ? Math.max(...working.value.map((p) => p.id)) + 1
        : nextPointId.value
  }

  /**
   * Insère un tracé ORS et met à jour le finding associé.
   * `coords` contient le tracé complet renvoyé par ORS (les ancres
   * sont exclues côté Rust pour éviter les doublons).
   */
  async function applyRoute(
    findingId: string,
    start: number,
    end: number,
    coords: LatLon[],
    profile: 'driving-car' | 'cycling-road',
  ): Promise<void> {
    if (!currentTraceId.value) {
      throw new Error('Aucune trace en cours d\'audit.')
    }
    const state = await invoke<AuditState>('audit_apply_route', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
      start,
      end,
      coords,
      profile,
      nextPointId: nextPointId.value,
    })
    working.value = state.points
    findings.value = state.findings
    nextPointId.value =
      working.value.length > 0
        ? Math.max(...working.value.map((p) => p.id)) + 1
        : nextPointId.value
  }

  /** Marque un finding comme faux positif (trace inchangée). */
  async function markFp(findingId: string): Promise<void> {
    const result = await invoke<Finding[]>('audit_mark_fp', {
      findings: findings.value,
      findingId,
    })
    findings.value = result
  }

  /** Retire le marqueur faux positif (retour à pending). */
  async function unmarkFp(findingId: string): Promise<void> {
    const result = await invoke<Finding[]>('audit_unmark_fp', {
      findings: findings.value,
      findingId,
    })
    findings.value = result
  }

  /**
   * Annule la correction d'un finding (undo).
   * Rust refuse si la zone a été réutilisée par une correction ultérieure
   * ou si les ancres ont disparu.
   */
  async function undoCorrection(findingId: string): Promise<void> {
    if (!currentTraceId.value) {
      throw new Error('Aucune trace en cours d\'audit.')
    }
    const state = await invoke<AuditState>('audit_undo_correction', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
    })
    working.value = state.points
    findings.value = state.findings
  }

  /**
   * Valide l'audit : réécrit le GPX, pose audit_status = "clean".
   * Point de non-retour. La vue doit fermer après succès.
   */
  async function validateAndRewrite(): Promise<unknown> {
    if (!currentTraceId.value) {
      throw new Error('Aucune trace en cours d\'audit.')
    }
    return invoke('audit_validate', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
    })
  }

  /**
   * Réinitialise intégralement le store.
   * Appelé à la sortie de la vue /audit (décision C.2).
   */
  function reset(): void {
    currentTraceId.value = null
    working.value = []
    findings.value = []
    params.value = null
    selectedFindingId.value = null
    nextPointId.value = 0
    analysisDurationMs.value = 0
  }

  /** Sélectionne ou désélectionne un finding. */
  function selectFinding(findingId: string | null): void {
    selectedFindingId.value = findingId
  }

  return {
    // State
    currentTraceId,
    working,
    findings,
    params,
    selectedFindingId,
    nextPointId,
    analysisDurationMs,
    // Getters
    pendingCount,
    correctedCount,
    fpCount,
    allProcessed,
    hasWorkInProgress,
    selectedFinding,
    canApply,
    totalFindings,
    // Actions
    runAudit,
    applyDelete,
    applyRoute,
    markFp,
    unmarkFp,
    undoCorrection,
    validateAndRewrite,
    reset,
    selectFinding,
  }
})
```

---

## Notes pour Zcode

1. **Pattern setup store strict** : `defineStore('audit', () => { ... })`.
   Pas de `defineStore({ state, getters, actions })`.

2. **Ordre interne** : State → Getters → Actions → Return. Conforme à
   `docs/CONVENTIONS.md`.

3. **Conversion camelCase ↔ snake_case** : Tauri 2.x convertit automatiquement
   les **arguments** d'appel (`traceId` → `trace_id`), mais **pas** les
   **retours** de commande. Les interfaces TypeScript portent donc les noms
   **camelCase** utilisés côté front, et le Rust sérialise en snake_case
   que `invoke` mappe automatiquement sur les propriétés camelCase... **non**.
   En réalité, Tauri ne fait **aucune** conversion sur les retours : le
   front doit s'attendre aux noms **snake_case** renvoyés par Rust.

   **Correction du contrat** : les interfaces ci-dessus **doivent** être en
   camelCase, mais chaque interface utilisée comme **retour de commande**
   doit porter `#[serde(rename_all = "camelCase")]` côté Rust. Cette
   convention est déjà appliquée dans `settings.rs` (`#[serde(rename_all =
   "camelCase")]` sur `SettingDefinition`).

   **Recommandation** : ajouter `#[serde(rename_all = "camelCase")]` sur
   toutes les structs de `types.rs` (Livrable 1) à l'exception des enums
   (déjà couvertes par `rename_all = "lowercase"` / `"kebab-case"`).

4. **`nextPointId` recalculé après chaque action** : Rust ne renvoie pas le
   compteur, donc le front le recalcule à partir du max d'ids de `working`.
   Cela garantit l'invariant « ids jamais recyclés » (CORRECTIONS C5).

5. **Aucune logique métier dans le store** : pas de calcul d'angle, pas de
   détection, pas de tri. Tout passe par les commandes Tauri.

6. **Gestion d'erreurs** : les actions `invoke` peuvent lever. Le store ne
   fait pas de try/catch — c'est à l'appelant (composant) de gérer, avec
   un toast via `useUiStore` (ou équivalent du projet).

7. **`_bounds` est optionnel** : rempli par le composant carte après
   calcul des bornes `LngLatBounds`. Le store ne le calcule jamais.

---

## Tests de validation (à créer dans `src/stores/__tests__/audit.spec.ts`)

Ces tests mockent `invoke` pour vérifier la bonne mise à jour de l'état :

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useAuditStore } from '../audit'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'

describe('audit store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('runAudit initialise l\'état', async () => {
    ;(invoke as any).mockResolvedValueOnce({
      traceId: 't1',
      points: [
        { id: 1, lat: 45, lon: 2 },
        { id: 2, lat: 45.001, lon: 2 },
      ],
      totalDistanceM: 100,
      findings: [],
      params: {
        consolM: 0.5,
        tolDeg: 20,
        pairM: 50,
        maxpairs: 5,
        segM: 200,
        closeM: 15,
        angleDeg: 270,
      },
      durationMs: 42,
    })

    const store = useAuditStore()
    await store.runAudit('t1', {
      consolM: 0.5,
      tolDeg: 20,
      pairM: 50,
      maxpairs: 5,
      segM: 200,
      closeM: 15,
      angleDeg: 270,
    })

    expect(store.currentTraceId).toBe('t1')
    expect(store.working).toHaveLength(2)
    expect(store.nextPointId).toBe(3)
    expect(store.analysisDurationMs).toBe(42)
  })

  it('canApply est faux si pending > 0', () => {
    const store = useAuditStore()
    store.findings = [
      {
        id: 'ar-1',
        kind: 'ar',
        label: 'Aller-retour : 1',
        summary: '',
        peak: 5,
        peakId: 6,
        pairs: [],
        pairIdx: [],
        ecart: 0,
        d1: 10,
        d2: 10,
        totalAngle: null,
        turnText: null,
        coreIds: [],
        zoneIds: [],
        ctxIds: { up: null, dn: null },
        ctx: { up: null, dn: null },
        parts: [],
        status: 'pending',
        correction: null,
        undo: null,
      },
    ]
    expect(store.canApply).toBe(false)
    expect(store.pendingCount).toBe(1)
  })

  it('canApply est vrai si pending === 0 et au moins un traité', () => {
    const store = useAuditStore()
    store.findings = [
      {
        id: 'ar-1',
        kind: 'ar',
        label: 'Aller-retour : 1',
        summary: '',
        peak: 5,
        peakId: 6,
        pairs: [],
        pairIdx: [],
        ecart: 0,
        d1: 10,
        d2: 10,
        totalAngle: null,
        turnText: null,
        coreIds: [],
        zoneIds: [],
        ctxIds: { up: null, dn: null },
        ctx: { up: null, dn: null },
        parts: [],
        status: 'fp',
        correction: null,
        undo: null,
      },
    ]
    expect(store.canApply).toBe(true)
    expect(store.fpCount).toBe(1)
  })

  it('reset vide tout l\'état', () => {
    const store = useAuditStore()
    store.currentTraceId = 't1'
    store.working = [{ id: 1, lat: 0, lon: 0 }]
    store.findings = []
    store.reset()
    expect(store.currentTraceId).toBeNull()
    expect(store.working).toHaveLength(0)
    expect(store.nextPointId).toBe(0)
  })
})
```

---

## Critères de validation

- `npx vue-tsc --noEmit` passe sans erreur
- Les 4 tests Vitest passent
- Aucune dépendance à un composant Vue (store pur)
- Aucune logique métier (que de l'orchestration Tauri)