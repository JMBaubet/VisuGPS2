<script setup lang="ts">
/**
 * Panneau d'action contextuel de l'anomalie sélectionnée (IHM §6).
 *
 * Six vues exclusives, déterminées par le statut de l'anomalie :
 * - `main` (à traiter) : routage, suppression de points, faux positif ;
 * - `routeSel` / `choice` : correction par routage OpenRouteService —
 *   intégrées à la sous-étape 4.4 (message d'attente en attendant) ;
 * - `delSel` : réglage de la plage à supprimer, avec **aperçu** sur la carte ;
 * - `fpMode` : retrait du marqueur faux positif ;
 * - `undoMode` : annulation de la correction.
 *
 * L'aperçu de suppression est **prospectif** : il ne modifie jamais la trace de
 * travail et est abandonné à toute sortie de vue (`delClose` de la référence).
 */
import { ref, computed, watch, onUnmounted } from 'vue'
import { useAuditStore, type Finding, type LatLon } from '../../stores/audit'
import { useUiStore } from '../../stores/ui'
import { useAuditOrs, type OrsProfile } from '../../composables/useAuditOrs'

const props = defineProps<{ finding: Finding }>()
const emit = defineEmits<{ close: [] }>()

const auditStore = useAuditStore()
const ui = useUiStore()
const ors = useAuditOrs()

type PanelView =
  | 'main'
  | 'routeSel'
  | 'choice'
  | 'delSel'
  | 'fpMode'
  | 'undoMode'

const currentView = ref<PanelView>('main')
/** Couleurs de l'aperçu (IHM §15) : conservé / à supprimer. */
const AUDIT_KEEP = '#8fc1ff'
const AUDIT_DROP_RED = '#dd3327'

/** Bornes du réglage de suppression, domaine de l'emprise de l'anomalie. */
const delRange = ref<[number, number]>([0, 0])
/** Bornes de la vue de routage : les ancres conservées. */
const routeRange = ref<[number, number]>([0, 0])
/** Domaine des curseurs de routage (IHM §7.1). */
const routeDomain = ref<{ startMin: number; startMax: number; endMin: number; endMax: number }>({
  startMin: 0,
  startMax: 0,
  endMin: 1,
  endMax: 1,
})
/** Profil retenu dans la vue de choix (défaut : voiture). */
const selectedProfile = ref<OrsProfile>('driving-car')

/** Domaine des curseurs : l'emprise de l'anomalie (IHM §8.1, §9.1). */
const zoneStart = computed(() => props.finding.parts[0]?.s ?? 0)
const zoneEnd = computed(() => props.finding.parts[0]?.e ?? 0)

/** Titre : « label · pts s → e » (à traiter) ou label seul (corrigée). */
const title = computed(() => {
  const f = props.finding
  const part = f.parts[0]
  if (f.status === 'corrected' || !part) return f.label
  return `${f.label} · pts ${part.s + 1} → ${part.e + 1}`
})

/** Aperçu courant, s'il porte sur l'anomalie affichée. */
const preview = computed(() =>
  auditStore.deletePreview && auditStore.selectedFindingId === props.finding.id
    ? auditStore.deletePreview
    : null,
)

/** Note d'aide de la vue de suppression, selon le modèle AR ou RP. */
const deleteNote = computed(() =>
  props.finding.kind === 'rp'
    ? "Aperçu : les curseurs épargnent — les points bleus seront conservés, les points gris seront supprimés, et le trait rouge montre la plage réellement supprimée. À l'ouverture, tout l'intérieur de l'emprise est supprimé."
    : 'Aperçu : les points grisés seront supprimés, les points bleus seront conservés ; la ligne bleue montre le raccord direct entre les points conservés. Un point isolé est possible en amenant Début sur Fin.',
)

/**
 * Libellés des curseurs : compteur de points pour l'AR, intervalles conservés
 * pour le RP (IHM §8.2, §9.2).
 */
const countStart = computed(() => preview.value?.countStart ?? `pt ${delRange.value[0] + 1}`)
const countEnd = computed(() => preview.value?.countEnd ?? `pt ${delRange.value[1] + 1}`)

/**
 * Pistes bicolores des curseurs (IHM §8.2, §9.2) : la partie avant le curseur
 * est peinte par `track-fill-color`, celle après par `track-color`. En AR la
 * zone neutre garde la couleur du thème.
 */
const startTrack = computed(() => {
  if (props.finding.kind === 'rp') {
    return { fill: AUDIT_KEEP, track: AUDIT_DROP_RED }
  }
  return { fill: undefined, track: AUDIT_KEEP }
})
const endTrack = computed(() => {
  if (props.finding.kind === 'rp') {
    return { fill: AUDIT_DROP_RED, track: AUDIT_KEEP }
  }
  return { fill: AUDIT_KEEP, track: undefined }
})

/** Note de la vue d'annulation, selon le type de correction. */
const undoNote = computed(() => {
  const kind =
    props.finding.correction === 'delete'
      ? 'points supprimés'
      : props.finding.correction === 'route-car'
        ? 'routage voiture'
        : 'routage vélo'
  return `Anomalie corrigée (${kind}). L'annulation restaure les points d'origine et remet le statut à « À traiter ».`
})

// ─── Cycle de vue ─────────────────────────────────────────────────────

/** Place la vue et le réglage selon le statut de l'anomalie. */
function syncView(): void {
  const f = props.finding
  if (f.status === 'pending') {
    currentView.value = 'main'
  } else if (f.status === 'fp') {
    currentView.value = 'fpMode'
  } else {
    currentView.value = 'undoMode'
  }
}

watch(() => [props.finding.id, props.finding.status], syncView, {
  immediate: true,
})

// Toute sortie de la vue de suppression abandonne l'aperçu (delClose).
watch(currentView, (view) => {
  if (view !== 'delSel') auditStore.clearDeletePreview()
})

/** Abandonne tous les aperçus en cours (suppression et routage). */
function clearPreviews(): void {
  auditStore.clearDeletePreview()
  auditStore.clearRoutePreview()
  auditStore.setRouteRange(null)
}

onUnmounted(() => {
  clearPreviews()
})

// ─── Suppression de points ────────────────────────────────────────────

/** Ouvre le réglage de la plage ; l'ouverture supprime tout l'intérieur. */
async function openDeleteSel(): Promise<void> {
  const f = props.finding
  if (f.status !== 'pending') return
  if (f.kind === 'rp' && zoneEnd.value - zoneStart.value < 2) {
    ui.showError('Emprise trop courte pour une suppression de points.')
    return
  }
  delRange.value = [zoneStart.value, zoneEnd.value]
  currentView.value = 'delSel'
  await refreshPreview()
}

/** Curseur Début déplacé : l'aperçu est recalculé avec la nouvelle borne. */
async function onStartMoved(value: number): Promise<void> {
  delRange.value = [value, delRange.value[1]]
  await refreshPreview()
}

/** Curseur Fin déplacé. */
async function onEndMoved(value: number): Promise<void> {
  delRange.value = [delRange.value[0], value]
  await refreshPreview()
}

/**
 * Recalcule l'aperçu puis réécrit les bornes effectives dans les curseurs.
 *
 * Le clampage (curseurs qui ne se croisent pas, plage dans l'emprise) est fait
 * côté Rust, comme `updateDeleteSelUI` le fait sur les `<input>`.
 */
async function refreshPreview(): Promise<void> {
  try {
    await auditStore.previewDelete(delRange.value[0], delRange.value[1])
    const applied = auditStore.deletePreview
    if (applied) delRange.value = [applied.start, applied.end]
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Aperçu impossible."
    ui.showError(msg)
  }
}

/** Validation : applique la suppression et ferme le panneau. */
async function doDelete(): Promise<void> {
  try {
    await auditStore.applyDelete(
      props.finding.id,
      delRange.value[0],
      delRange.value[1],
    )
    emit('close')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Suppression refusée.'
    ui.showError(msg)
  }
}

function cancelDelSel(): void {
  auditStore.clearDeletePreview()
  currentView.value = 'main'
}

// ─── Faux positif ─────────────────────────────────────────────────────

async function markFp(): Promise<void> {
  try {
    await auditStore.markFp(props.finding.id)
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Marquage impossible.'
    ui.showError(msg)
  }
}

async function unmarkFp(): Promise<void> {
  try {
    await auditStore.unmarkFp(props.finding.id)
    currentView.value = 'main'
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Retrait impossible.'
    ui.showError(msg)
  }
}

// ─── Annulation ───────────────────────────────────────────────────────

async function undo(): Promise<void> {
  try {
    await auditStore.undoCorrection(props.finding.id)
    currentView.value = 'main'
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Annulation impossible.'
    ui.showError(msg)
  }
}

// ─── Routage OpenRouteService (IHM §7) ────────────────────────────────

/** Note d'aide de la vue de sélection des ancres. */
const ROUTE_NOTE =
  'La zone sélectionnée sera remplacée par le tracé routier entre les points Début et Fin (conservés). Points proposés par défaut : les ancres de l\'anomalie. Extension possible de ±3 points, sans mordre sur les anomalies voisines. Une zone plus courte donne une réponse plus rapide.'

/** Aperçu de routage courant, s'il porte sur l'anomalie affichée. */
const routePreview = computed(() =>
  auditStore.routePreview && auditStore.selectedFindingId === props.finding.id
    ? auditStore.routePreview
    : null,
)

/** Libellés des curseurs de routage. */
const routeStartNo = computed(() => `pt ${routeRange.value[0] + 1}`)
const routeEndNo = computed(() => `pt ${routeRange.value[1] + 1}`)

/**
 * Ouvre la vue de sélection des ancres (IHM §7.1).
 *
 * Le domaine des curseurs est borné par les anomalies voisines **non
 * corrigées** : impossible de mordre leur emprise. Les ancres par défaut sont
 * les points de contexte sain (AR) ou les ancres radiales (RP).
 */
function openRouteSel(): void {
  const f = props.finding
  if (f.status !== 'pending') return

  const total = auditStore.working.length
  let lo = 0
  let hi = total - 1
  for (const other of auditStore.findings) {
    if (other.id === f.id || other.status === 'corrected') continue
    const part = other.parts[0]
    const mine = f.parts[0]
    if (!part || !mine) continue
    if (part.e < mine.s) lo = Math.max(lo, part.e + 1)
    if (part.s > mine.e) hi = Math.min(hi, part.s - 1)
  }

  let u = f.parts[0]?.s ?? 0
  let v = f.parts[0]?.e ?? 0
  if (f.kind === 'rp') {
    const anchors = auditStore.overlayOf(f.id)?.anchors
    if (anchors) {
      u = anchors.up
      v = anchors.dn
    }
  } else {
    u = f.ctx.up ?? u
    v = f.ctx.dn ?? v
  }

  routeDomain.value = {
    startMin: Math.max(0, lo, u - 3),
    startMax: Math.min(v - 1, hi),
    endMin: Math.max(0, lo, u - 3) + 1,
    endMax: Math.min(total - 1, hi, v + 3),
  }
  routeRange.value = [u, v]
  clampRouteRange()
  auditStore.setRouteRange([routeRange.value[0], routeRange.value[1]])
  currentView.value = 'routeSel'
}

/** Clamp en direct : les ancres ne se croisent jamais (§7.1). */
function clampRouteRange(): void {
  let [start, end] = routeRange.value
  const d = routeDomain.value
  start = Math.min(Math.max(start, d.startMin), d.startMax)
  end = Math.min(Math.max(end, d.endMin), d.endMax)
  if (start > end - 1) start = end - 1
  if (end < start + 1) end = start + 1
  routeRange.value = [start, end]
  auditStore.setRouteRange([start, end])
}

function onRouteStartMoved(value: number): void {
  routeRange.value = [value, routeRange.value[1]]
  clampRouteRange()
}

function onRouteEndMoved(value: number): void {
  routeRange.value = [routeRange.value[0], value]
  clampRouteRange()
}

/** Coordonnée d'ancrage : la trace de travail, jamais un point snapé (§18.2). */
function anchorAt(index: number): LatLon {
  const p = auditStore.working[index]
  return { lat: p?.lat ?? 0, lon: p?.lon ?? 0 }
}

/** Demande les deux tracés, ou annule la demande en cours (§18.4). */
async function requestRoute(): Promise<void> {
  const [start, end] = routeRange.value
  const result = await ors.requestRoutes(anchorAt(start), anchorAt(end))
  if (!result) return

  auditStore.setRoutePreview({
    car: result.car,
    bike: result.bike,
    identical: result.identical,
    start,
    end,
  })
  // Profil par défaut : la voiture, ou le profil effectivement disponible.
  selectedProfile.value = result.car ? 'driving-car' : 'cycling-road'
  currentView.value = 'choice'
}

/** Trace d'un profil : `null` si le profil est indisponible. */
function routeOf(profile: OrsProfile) {
  const preview = routePreview.value
  if (!preview) return null
  return profile === 'driving-car' ? preview.car : preview.bike
}

/** Métriques d'un profil : « X,XX km · 12 min », ou « indisponible ». */
function routeInfo(profile: OrsProfile): string {
  const route = routeOf(profile)
  if (!route) return 'indisponible'
  return `${formatKm(route.distance)} · ${formatOrsDuration(route.duration)}`
}

/** Distance en kilomètres, deux décimales, virgule française. */
function formatKm(meters: number): string {
  return `${(meters / 1000).toFixed(2).replace('.', ',')} km`
}

/** Durée au format de la référence : « 45 s », « 12 min », « 1,5 h ». */
function formatOrsDuration(seconds: number): string {
  if (seconds < 90) return `${Math.round(seconds)} s`
  if (seconds < 5400) return `${Math.round(seconds / 60)} min`
  return `${(seconds / 3600).toFixed(1).replace('.', ',')} h`
}

/** Note de la vue de choix lorsque les deux tracés sont identiques (§7.3). */
const identicalNote = computed(() => {
  const preview = routePreview.value
  if (!preview?.identical || !preview.car) return ''
  return `Tracé unique — voiture et vélo de route identiques (${formatKm(preview.car.distance)}).`
})

function cancelRouteSel(): void {
  clearPreviews()
  currentView.value = 'main'
}

function cancelRouteChoice(): void {
  clearPreviews()
  currentView.value = 'main'
}

/**
 * Applique le tracé retenu et ferme le panneau (IHM §7.4).
 *
 * Le profil est imposé à la voiture quand les deux tracés sont identiques : les
 * radios sont alors masqués.
 */
async function applyRoute(): Promise<void> {
  const preview = routePreview.value
  const finding = props.finding
  if (!preview || finding.status !== 'pending') return

  const profile: OrsProfile = preview.identical ? 'driving-car' : selectedProfile.value
  const route = routeOf(profile)
  if (!route) {
    ui.showError('Ce tracé est indisponible.')
    return
  }

  try {
    await auditStore.applyRoute(
      finding.id,
      preview.start,
      preview.end,
      route.coords,
      profile,
    )
    auditStore.clearRoutePreview()
    auditStore.setRouteRange(null)
    emit('close')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Routage refusé.'
    ui.showError(msg)
  }
}
</script>

<template>
  <v-card class="audit-action-panel" variant="elevated" elevation="8">
    <v-card-title class="d-flex justify-space-between align-center text-body-2">
      <span>{{ title }}</span>
      <v-btn
        icon="mdi-close"
        size="small"
        variant="text"
        title="Fermer"
        @click="emit('close')"
      />
    </v-card-title>

    <!-- Vue 1 — Main -->
    <v-card-actions v-if="currentView === 'main'" class="flex-column align-stretch">
      <v-btn
        block
        color="primary"
        prepend-icon="mdi-compass-outline"
        @click="openRouteSel"
      >
        Demande de routage
      </v-btn>
      <v-btn
        block
        color="warning"
        prepend-icon="mdi-content-cut"
        @click="openDeleteSel"
      >
        Suppression de points
      </v-btn>
      <v-btn
        block
        color="info"
        prepend-icon="mdi-cancel"
        @click="markFp"
      >
        Faux positif
      </v-btn>
    </v-card-actions>

    <!-- Vue 2 — RouteSel -->
    <v-card-text v-if="currentView === 'routeSel'">
      <v-alert density="compact" variant="tonal" type="info" class="mb-4">
        {{ ROUTE_NOTE }}
      </v-alert>

      <div class="text-caption mb-1">Début — {{ routeStartNo }}</div>
      <v-slider
        :model-value="routeRange[0]"
        :min="routeDomain.startMin"
        :max="routeDomain.startMax"
        :step="1"
        thumb-color="warning"
        hide-details
        @update:model-value="onRouteStartMoved"
      />

      <div class="text-caption mb-1 mt-4">Fin — {{ routeEndNo }}</div>
      <v-slider
        :model-value="routeRange[1]"
        :min="routeDomain.endMin"
        :max="routeDomain.endMax"
        :step="1"
        thumb-color="warning"
        hide-details
        @update:model-value="onRouteEndMoved"
      />

      <v-row class="mt-3">
        <v-col>
          <v-btn block color="primary" :loading="ors.isLoading.value" @click="requestRoute">
            {{ ors.isLoading.value ? 'ORS… (cliquer pour annuler)' : 'Demander le routage' }}
          </v-btn>
        </v-col>
        <v-col>
          <v-btn block variant="text" @click="cancelRouteSel">Annuler</v-btn>
        </v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 3 — Choice -->
    <v-card-text v-if="currentView === 'choice'">
      <v-alert
        v-if="routePreview?.identical"
        density="compact"
        variant="tonal"
        type="info"
        class="mb-3"
      >
        {{ identicalNote }}
      </v-alert>

      <v-radio-group v-else v-model="selectedProfile" hide-details>
        <v-radio value="driving-car" :disabled="!routePreview?.car">
          <template #label>
            <span>
              Voiture
              <span class="text-caption ml-2">{{ routeInfo('driving-car') }}</span>
            </span>
          </template>
        </v-radio>
        <v-radio value="cycling-road" :disabled="!routePreview?.bike">
          <template #label>
            <span>
              Vélo de route
              <span class="text-caption ml-2">{{ routeInfo('cycling-road') }}</span>
            </span>
          </template>
        </v-radio>
      </v-radio-group>

      <v-row class="mt-3">
        <v-col>
          <v-btn block color="success" @click="applyRoute">Appliquer</v-btn>
        </v-col>
        <v-col>
          <v-btn block variant="text" @click="cancelRouteChoice">Annuler</v-btn>
        </v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 4 — DelSel -->
    <v-card-text v-if="currentView === 'delSel'">
      <v-alert density="compact" variant="tonal" type="warning" class="mb-4">
        {{ deleteNote }}
      </v-alert>

      <div class="text-caption mb-1">Début — {{ countStart }}</div>
      <v-slider
        :model-value="delRange[0]"
        :min="zoneStart"
        :max="zoneEnd"
        :step="1"
        :track-color="startTrack.track"
        :track-fill-color="startTrack.fill"
        thumb-color="info"
        hide-details
        @update:model-value="onStartMoved"
      />

      <div class="text-caption mb-1 mt-4">Fin — {{ countEnd }}</div>
      <v-slider
        :model-value="delRange[1]"
        :min="zoneStart"
        :max="zoneEnd"
        :step="1"
        :track-color="endTrack.track"
        :track-fill-color="endTrack.fill"
        thumb-color="info"
        hide-details
        @update:model-value="onEndMoved"
      />

      <v-row class="mt-3">
        <v-col>
          <v-btn block color="error" @click="doDelete">Supprimer</v-btn>
        </v-col>
        <v-col>
          <v-btn block variant="text" @click="cancelDelSel">Annuler</v-btn>
        </v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 5 — FpMode -->
    <v-card-text v-if="currentView === 'fpMode'">
      <p class="text-body-2 mb-3">
        Cette anomalie est marquée <b>faux positif</b> : elle sera laissée telle
        quelle dans le GPX généré.
      </p>
      <v-btn block variant="tonal" prepend-icon="mdi-undo" @click="unmarkFp">
        Retirer le marqueur
      </v-btn>
    </v-card-text>

    <!-- Vue 6 — UndoMode -->
    <v-card-text v-if="currentView === 'undoMode'">
      <p class="text-body-2 mb-3">{{ undoNote }}</p>
      <v-btn block color="warning" prepend-icon="mdi-undo" @click="undo">
        Annuler la correction
      </v-btn>
    </v-card-text>
  </v-card>
</template>

<style scoped>
.audit-action-panel {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 320px;
  z-index: 10;
}
</style>
