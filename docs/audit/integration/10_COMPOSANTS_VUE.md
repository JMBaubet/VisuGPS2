# Livrable 10 — Squelette des composants Vue

**Base** : `src/components/Audit/`
**Vue principale** : `src/views/Audit.vue`
**Store** : `src/stores/audit.ts` (Livrable 2)
**Référence comportementale** : `docs/audit/spec/IHM_V2.md`

⚠️ **IHM V2.0.md est une source d'inspiration, pas un contrat à copier.**
L'implémentation doit suivre les conventions VisuGPS2 (Vuetify 3, Pinia
setup stores, conventions `docs/CONVENTIONS.md`). Pas de palette CSS
custom, pas de composants HTML natifs.

---

## 1. Vue principale — `src/views/Audit.vue`

**Rôle** : orchestration. Charge la trace, monte la carte, affiche les
panneaux, gère les dialogues de sortie et d'application.

**Props** : aucune (le `traceId` arrive via `route.query.traceId`).

**State local** :
- `dialogExitOpen: boolean`
- `dialogApplyOpen: boolean`

**Lifecycle** :
- `onMounted` : charge les paramètres `Audit.*`, lance `runAudit`,
  initialise la carte.
- `onBeforeRouteLeave` : si `hasWorkInProgress`, bloque et ouvre
  `dialogExit`.

**Structure** :

```vue
<template>
  <v-container fluid class="fill-height pa-0 audit-container">
    <AuditToolbar
      :trace-name="traceName"
      :pending-count="auditStore.pendingCount"
      :corrected-count="auditStore.correctedCount"
      :fp-count="auditStore.fpCount"
      :can-apply="auditStore.canApply"
      @back="onBackClicked"
      @apply="onApplyClicked"
    />

    <div class="audit-layout">
      <AuditFindingsPanel
        :findings="auditStore.findings"
        :selected-id="auditStore.selectedFindingId"
        @select="onFindingSelected"
      />

      <AuditMap
        ref="mapRef"
        :points="auditStore.working"
        :findings="auditStore.findings"
        :selected-id="auditStore.selectedFindingId"
      />

      <AuditActionPanel
        v-if="auditStore.selectedFinding"
        :finding="auditStore.selectedFinding"
        @close="auditStore.selectFinding(null)"
      />
    </div>

    <ConfirmExitDialog
      v-model="dialogExitOpen"
      :work-count="auditStore.correctedCount + auditStore.fpCount"
      @confirm="onExitConfirmed"
      @cancel="dialogExitOpen = false"
    />

    <ConfirmApplyDialog
      v-model="dialogApplyOpen"
      @confirm="onApplyConfirmed"
      @cancel="dialogApplyOpen = false"
    />
  </v-container>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { useAuditStore, type AuditParams } from '../stores/audit'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import AuditToolbar from '../components/Audit/AuditToolbar.vue'
import AuditFindingsPanel from '../components/Audit/AuditFindingsPanel.vue'
import AuditMap from '../components/Audit/AuditMap.vue'
import AuditActionPanel from '../components/Audit/AuditActionPanel.vue'
import ConfirmExitDialog from '../components/Audit/dialogs/ConfirmExitDialog.vue'
import ConfirmApplyDialog from '../components/Audit/dialogs/ConfirmApplyDialog.vue'

const route = useRoute()
const router = useRouter()
const auditStore = useAuditStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

const mapRef = ref<InstanceType<typeof AuditMap> | null>(null)
const dialogExitOpen = ref(false)
const dialogApplyOpen = ref(false)

const traceId = computed(() => route.query.traceId as string | null)
const traceName = computed(() =>
  tracesStore.traces.find((t) => t.id === traceId.value)?.name ?? 'Trace',
)

function getSetting(path: string, fallback: number): number {
  const s = settingsStore.settings.find((x) => x.path === path)
  return s ? (s.value as number) : fallback
}

function buildParams(): AuditParams {
  return {
    consolM: getSetting('Audit.Consolidation.seuil', 0.5),
    tolDeg: getSetting('Audit.AR.toleranceDeg', 20),
    pairM: getSetting('Audit.AR.seuilPaireM', 50),
    maxpairs: getSetting('Audit.AR.maxPaires', 5),
    segM: getSetting('Audit.AR.branchesMaxM', 200),
    closeM: getSetting('Audit.RP.seuilFermetureM', 15),
    angleDeg: getSetting('Audit.RP.angleMinDeg', 270),
  }
}

onMounted(async () => {
  if (!traceId.value) {
    ui.showError('Aucune trace à auditer.')
    router.replace({ name: 'accueil' })
    return
  }
  try {
    await settingsStore.loadSettings()
    await auditStore.runAudit(traceId.value, buildParams())
  } catch (e) {
    ui.showError(`Erreur d'audit : ${e}`)
    router.replace({ name: 'accueil' })
  }
})

onBeforeRouteLeave(() => {
  if (auditStore.hasWorkInProgress && !dialogExitOpen.value) {
    dialogExitOpen.value = true
    return false
  }
  auditStore.reset()
  return true
})

function onFindingSelected(findingId: string) {
  auditStore.selectFinding(findingId)
}

function onBackClicked() {
  router.push({ name: 'accueil' })
}

function onExitConfirmed() {
  dialogExitOpen.value = false
  auditStore.reset()
  router.push({ name: 'accueil' })
}

function onApplyClicked() {
  if (!auditStore.canApply) return
  dialogApplyOpen.value = true
}

async function onApplyConfirmed() {
  dialogApplyOpen.value = false
  try {
    await auditStore.validateAndRewrite()
    ui.showSuccess('Audit appliqué. Le GPX a été réécrit.')
    auditStore.reset()
    router.push({ name: 'accueil' })
  } catch (e) {
    ui.showError(`Échec de la validation : ${e}`)
  }
}
</script>

<style scoped>
.audit-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.audit-layout {
  display: flex;
  flex: 1;
  position: relative;
  overflow: hidden;
}
</style>
```

**Points clés** :
- `onBeforeRouteLeave` retourne `false` pour bloquer la navigation
  tant que `dialogExitOpen` n'est pas confirmé.
- Le `traceId` est lu depuis `route.query.traceId` (décision C.1).
- `auditStore.reset()` appelé à la sortie (décision C.2).

---

## 2. `AuditToolbar.vue`

**Rôle** : barre supérieure. Retour, titre, indicateur de progression,
bouton Appliquer.

**Props** :
- `traceName: string`
- `pendingCount: number`
- `correctedCount: number`
- `fpCount: number`
- `canApply: boolean`

**Emits** :
- `'back'`
- `'apply'`

```vue
<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Retour à l'accueil"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <v-icon icon="mdi-map-marker-path" class="mr-2" />
      Audit GPX — {{ traceName }}
    </v-app-bar-title>

    <template #append>
      <AuditProgressChip
        :pending="pendingCount"
        :corrected="correctedCount"
        :fp="fpCount"
        class="mr-3"
      />
      <v-btn
        color="success"
        :disabled="!canApply"
        prepend-icon="mdi-check"
        :title="
          canApply
            ? 'Appliquer les corrections et fermer'
            : 'Toutes les anomalies doivent être traitées (ou marquées faux positif)'
        "
        @click="emit('apply')"
      >
        Appliquer
      </v-btn>
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
import AuditProgressChip from './AuditProgressChip.vue'

defineProps<{
  traceName: string
  pendingCount: number
  correctedCount: number
  fpCount: number
  canApply: boolean
}>()

const emit = defineEmits<{
  back: []
  apply: []
}>()
</script>
```

---

## 3. `AuditProgressChip.vue`

**Rôle** : badge de progression.

**Props** :
- `pending: number`
- `corrected: number`
- `fp: number`

```vue
<template>
  <v-chip :color="chipColor" size="small" variant="tonal">
    <v-icon start :icon="chipIcon" />
    {{ done }}/{{ total }}
    <span v-if="fp > 0" class="ml-1">· {{ fp }} FP</span>
  </v-chip>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  pending: number
  corrected: number
  fp: number
}>()

const done = computed(() => props.corrected + props.fp)
const total = computed(() => done.value + props.pending)

const chipColor = computed(() => {
  if (props.pending === 0 && done.value === 0) return 'default'
  if (props.pending === 0 && props.fp === 0) return 'success'
  if (props.pending === 0) return 'warning'
  if (done.value > 0) return 'orange'
  return 'error'
})

const chipIcon = computed(() => {
  if (props.pending === 0 && done.value === 0) return 'mdi-information-outline'
  if (props.pending === 0) return 'mdi-check-circle'
  if (done.value > 0) return 'mdi-progress-clock'
  return 'mdi-alert-circle'
})
</script>
```

---

## 4. `AuditFindingsPanel.vue`

**Rôle** : liste latérale des findings avec chips de statut.

**Props** :
- `findings: Finding[]`
- `selectedId: string | null`

**Emits** :
- `'select'` (findingId: string)

```vue
<template>
  <v-navigation-drawer permanent width="340" location="left" class="audit-findings">
    <v-list density="compact" nav>
      <v-list-item
        v-for="f in findings"
        :key="f.id"
        :active="f.id === selectedId"
        @click="emit('select', f.id)"
      >
        <template #prepend>
          <v-icon
            :icon="f.kind === 'rp' ? 'mdi-rotate-360' : 'mdi-swap-horizontal'"
            :color="kindColor(f)"
          />
        </template>

        <v-list-item-title>{{ f.label }}</v-list-item-title>
        <v-list-item-subtitle>{{ f.summary }}</v-list-item-subtitle>

        <template #append>
          <v-chip :color="statusColor(f)" size="x-small" variant="tonal">
            {{ statusText(f) }}
          </v-chip>
        </template>
      </v-list-item>
    </v-list>

    <v-empty-state
      v-if="findings.length === 0"
      icon="mdi-check-circle"
      title="Aucune anomalie"
      text="La trace est propre."
    />

    <template #append>
      <AuditSynthesis
        :findings="findings"
        :total-distance-m="totalDistanceM"
      />
    </template>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import type { Finding } from '../../stores/audit'
import AuditSynthesis from './AuditSynthesis.vue'

defineProps<{
  findings: Finding[]
  selectedId: string | null
  totalDistanceM: number
}>()

const emit = defineEmits<{
  select: [findingId: string]
}>()

function kindColor(f: Finding): string {
  if (f.status === 'fp') return 'info'
  if (f.status === 'corrected') return 'success'
  return f.kind === 'rp' ? 'warning' : 'error'
}

function statusColor(f: Finding): string {
  if (f.status === 'fp') return 'warning'
  if (f.status === 'corrected') return 'success'
  return 'error'
}

function statusText(f: Finding): string {
  if (f.status === 'pending') return 'À traiter'
  if (f.status === 'fp') return 'Faux positif'
  if (f.correction === 'delete') return 'Points supprimés'
  if (f.correction === 'route-car') return 'Routage voiture'
  if (f.correction === 'route-bike') return 'Routage vélo'
  return 'Corrigé'
}
</script>
```

---

## 5. `AuditSynthesis.vue`

**Rôle** : synthèse globale (compteurs).

**Props** :
- `findings: Finding[]`
- `totalDistanceM: number`

```vue
<template>
  <v-expansion-panels variant="accordion" class="ma-2">
    <v-expansion-panel>
      <v-expansion-panel-title>
        <v-icon icon="mdi-chart-box" class="mr-2" />
        Synthèse
      </v-expansion-panel-title>
      <v-expansion-panel-text>
        <v-list density="compact">
          <v-list-item>
            <v-list-item-title>Anomalies</v-list-item-title>
            <template #append><b>{{ findings.length }}</b></template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Aller-retours</v-list-item-title>
            <template #append>{{ countByKind('ar') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Boucles giratoires</v-list-item-title>
            <template #append>{{ countByKind('rp') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>À traiter</v-list-item-title>
            <template #append>{{ countByStatus('pending') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Faux positifs</v-list-item-title>
            <template #append>{{ countByStatus('fp') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Corrigées</v-list-item-title>
            <template #append>{{ countByStatus('corrected') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Distance</v-list-item-title>
            <template #append>{{ (totalDistanceM / 1000).toFixed(1) }} km</template>
          </v-list-item>
        </v-list>
      </v-expansion-panel-text>
    </v-expansion-panel>
  </v-expansion-panels>
</template>

<script setup lang="ts">
import type { Finding, FindingKind, FindingStatus } from '../../stores/audit'

const props = defineProps<{
  findings: Finding[]
  totalDistanceM: number
}>()

function countByKind(kind: FindingKind): number {
  return props.findings.filter((f) => f.kind === kind).length
}

function countByStatus(status: FindingStatus): number {
  return props.findings.filter((f) => f.status === status).length
}
</script>
```

---

## 6. `AuditMap.vue`

**Rôle** : 3ᵉ instance Mapbox GL dédiée à l'audit.

**Props** :
- `points: AuditPoint[]`
- `findings: Finding[]`
- `selectedId: string | null`

**State local** :
- `map: shallowRef<mapboxgl.Map | null>`
- `mapReady: boolean`

**Registres de couches** :
- `traceRegistry` (trace de base)
- `appliedRegistry` (routage appliqué)
- `anomalyRegistry` (findings)
- `previewRegistry` (aperçus de routage)
- `deleteRegistry` (chemins de suppression)
- `sliderMarkers` (marqueurs de sliders)
- `labelMarkers` (étiquettes draguables)

```vue
<template>
  <div class="audit-map-wrapper">
    <div ref="mapContainer" class="map-canvas" />
    <v-empty-state
      v-if="!hasMapboxKey"
      class="map-placeholder"
      icon="mdi-map-outline"
      title="Clé Mapbox requise"
      text="Renseignez la clé dans les paramètres (Systeme.Key.mapBox)."
    />
  </div>
</template>

<script setup lang="ts">
import { ref, shallowRef, computed, onMounted, onUnmounted, watch } from 'vue'
import mapboxgl from 'mapbox-gl'
import 'mapbox-gl/dist/mapbox-gl.css'
import type { AuditPoint, Finding } from '../../stores/audit'
import { useSettingsStore } from '../../stores/settings'

const props = defineProps<{
  points: AuditPoint[]
  findings: Finding[]
  selectedId: string | null
}>()

const mapContainer = ref<HTMLDivElement | null>(null)
const map = shallowRef<mapboxgl.Map | null>(null)
const mapReady = ref(false)
const readyQueue: Array<() => void> = []

const settingsStore = useSettingsStore()

const hasMapboxKey = computed(() => {
  const s = settingsStore.settings.find((x) => x.path === 'Systeme.Key.mapBox')
  return !!s && !!s.value
})

function whenMapReady(fn: () => void) {
  if (mapReady.value && map.value) fn()
  else readyQueue.push(fn)
}

function initMap() {
  const keySetting = settingsStore.settings.find(
    (x) => x.path === 'Systeme.Key.mapBox',
  )
  if (!keySetting || !keySetting.value || !mapContainer.value) return

  mapboxgl.accessToken = keySetting.value as string
  map.value = new mapboxgl.Map({
    container: mapContainer.value,
    style: 'mapbox://styles/mapbox/outdoors-v12',
    center: [2.6, 46.6],
    zoom: 6,
  })

  map.value.on('load', () => {
    mapReady.value = true
    initLayers()
    const q = readyQueue.splice(0)
    for (const fn of q) fn()
    renderTrace()
    renderFindings()
  })

  map.value.addControl(new mapboxgl.NavigationControl({ showCompass: false }), 'bottom-right')
}

function initLayers() {
  // À IMPLÉMENTER : 5 registres (traceRegistry, appliedRegistry,
  // anomalyRegistry, previewRegistry, deleteRegistry)
  // Référence : structure du HTML de référence, sections 3.4 et 3.5
}

function renderTrace() {
  whenMapReady(() => {
    // À IMPLÉMENTER : ligne de trace + marqueurs start/end
    // + fitBounds sur la trace
  })
}

function renderFindings() {
  whenMapReady(() => {
    // À IMPLÉMENTER : features GeoJSON pour chaque finding
    // - trait de zone (rouge ou bleu clair si FP ou gris si corrigé)
    // - trait de cœur (RP uniquement)
    // - points individuels
    // - ancres mauves (RP uniquement)
    // - croix Kåsa (RP uniquement, marker HTML)
  })
}

function highlightSelected() {
  whenMapReady(() => {
    // À IMPLÉMENTER : centrage fitBounds sur le finding sélectionné
  })
}

onMounted(() => {
  if (hasMapboxKey.value) initMap()
})

onUnmounted(() => {
  if (map.value) {
    map.value.remove()
    map.value = null
    mapReady.value = false
  }
})

watch(() => props.points, renderTrace, { deep: true })
watch(() => props.findings, renderFindings, { deep: true })
watch(() => props.selectedId, highlightSelected)
</script>

<style scoped>
.audit-map-wrapper {
  flex: 1;
  position: relative;
  height: 100%;
}
.map-canvas {
  width: 100%;
  height: 100%;
}
.map-placeholder {
  position: absolute;
  inset: 0;
  background: #dfe3e8;
}
</style>
```

**Points de vigilance** :
- `shallowRef` pour la carte (objet impératif, non réactif).
- `map.remove()` dans `onUnmounted` (obligatoire, sinon fuite).
- Les features GeoJSON portent leurs popups dans `properties.popupHtml`.
  Un seul popup actif à la fois (voir `showPopupAt` du HTML de référence).
- Référence : les 5 registres du HTML sont à porter tels quels.
  Chaque registre expose `add(id, fc, layerSpec, onClick)`,
  `update(id, fc)`, `remove(id)`, `clear()`.

---

## 7. `AuditActionPanel.vue`

**Rôle** : panneau d'action contextuel pour le finding sélectionné.
6 vues exclusives.

**Props** :
- `finding: Finding`

**Emits** :
- `'close'`

**State local** :
- `currentView: 'main' | 'routeSel' | 'choice' | 'delSel' | 'fpMode' | 'undoMode'`
- `routeStart: number`, `routeEnd: number`
- `deleteStart: number`, `deleteEnd: number`
- `pendingRoutes: { car: any; bike: any; identical: boolean } | null`
- `selectedProfile: 'driving-car' | 'cycling-road'`

```vue
<template>
  <v-card class="audit-action-panel" variant="elevated" elevation="8">
    <v-card-title class="d-flex justify-space-between align-center">
      <span class="text-body-1">{{ finding.label }}</span>
      <v-btn
        icon="mdi-close"
        size="small"
        variant="text"
        @click="emit('close')"
      />
    </v-card-title>

    <!-- Vue 1 — Main -->
    <v-card-actions v-if="currentView === 'main'" class="flex-column">
      <v-btn
        block
        color="primary"
        prepend-icon="mdi-routes"
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
      <p class="text-caption mb-2">{{ routeHelpText }}</p>
      <v-range-slider
        v-model="routeRange"
        :min="0"
        :max="pointsCount - 1"
        strict
        thumb-label="always"
      />
      <v-row class="mt-3">
        <v-col><v-btn block color="primary" @click="doRoute">Routage</v-btn></v-col>
        <v-col><v-btn block variant="text" @click="cancelRouteSel">Annuler</v-btn></v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 3 — Choice -->
    <v-card-text v-if="currentView === 'choice'">
      <v-radio-group v-model="selectedProfile">
        <v-radio value="driving-car" label="Voiture" />
        <v-radio value="cycling-road" label="Vélo de route" />
      </v-radio-group>
      <p class="text-caption">{{ routeInfo }}</p>
      <v-row class="mt-3">
        <v-col><v-btn block color="success" @click="applyRoute">Appliquer</v-btn></v-col>
        <v-col><v-btn block variant="text" @click="cancelRouteChoice">Annuler</v-btn></v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 4 — DelSel -->
    <v-card-text v-if="currentView === 'delSel'">
      <p class="text-caption mb-2">{{ deleteHelpText }}</p>
      <v-range-slider
        v-model="deleteRange"
        :min="finding.parts[0].s"
        :max="finding.parts[0].e"
        strict
        thumb-label="always"
      />
      <v-row class="mt-3">
        <v-col><v-btn block color="error" @click="doDelete">Supprimer</v-btn></v-col>
        <v-col><v-btn block variant="text" @click="cancelDelSel">Annuler</v-btn></v-col>
      </v-row>
    </v-card-text>

    <!-- Vue 5 — FpMode -->
    <v-card-text v-if="currentView === 'fpMode'">
      <p class="text-body-2 mb-3">
        Cette anomalie est marquée <b>faux positif</b>. Elle sera
        laissée telle quelle dans le GPX généré.
      </p>
      <v-btn block variant="tonal" prepend-icon="mdi-undo" @click="unmarkFp">
        Retirer le marqueur
      </v-btn>
    </v-card-text>

    <!-- Vue 6 — UndoMode -->
    <v-card-text v-if="currentView === 'undoMode'">
      <p class="text-body-2 mb-3">{{ undoDescription }}</p>
      <v-btn block color="warning" prepend-icon="mdi-undo" @click="undo">
        Annuler la correction
      </v-btn>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useAuditStore, type Finding } from '../../stores/audit'
import { useUiStore } from '../../stores/ui'

const props = defineProps<{ finding: Finding }>()
const emit = defineEmits<{ close: [] }>()

const auditStore = useAuditStore()
const ui = useUiStore()

const currentView = ref<
  'main' | 'routeSel' | 'choice' | 'delSel' | 'fpMode' | 'undoMode'
>('main')

const routeRange = ref<[number, number]>([0, 0])
const deleteRange = ref<[number, number]>([0, 0])
const selectedProfile = ref<'driving-car' | 'cycling-road'>('driving-car')
const pendingRoutes = ref<{
  car: any
  bike: any
  identical: boolean
} | null>(null)

const pointsCount = computed(() => auditStore.working.length)

const routeHelpText = computed(
  () => 'Positionnez les ancres Début/Fin. La zone intérieure sera remplacée par le tracé ORS.',
)
const deleteHelpText = computed(
  () =>
    props.finding.kind === 'rp'
      ? 'Curseurs = points conservés. La zone entre les curseurs sera supprimée.'
      : 'Plage directe : les points entre Début et Fin seront supprimés.',
)
const routeInfo = computed(() => {
  if (!pendingRoutes.value) return ''
  if (pendingRoutes.value.identical) {
    return `Tracé unique — voiture et vélo identiques (${(pendingRoutes.value.car.distance / 1000).toFixed(2)} km).`
  }
  const carKm = (pendingRoutes.value.car?.distance ?? 0) / 1000
  const bikeKm = (pendingRoutes.value.bike?.distance ?? 0) / 1000
  return `Voiture : ${carKm.toFixed(2)} km · Vélo : ${bikeKm.toFixed(2)} km`
})

const undoDescription = computed(() => {
  if (props.finding.correction === 'delete') return 'La suppression de points sera annulée.'
  if (props.finding.correction === 'route-car') return 'Le routage voiture sera annulé.'
  if (props.finding.correction === 'route-bike') return 'Le routage vélo sera annulé.'
  return ''
})

// Initialisation de la vue selon le statut du finding
watch(
  () => props.finding.status,
  (status) => {
    if (status === 'fp') currentView.value = 'fpMode'
    else if (status === 'corrected') currentView.value = 'undoMode'
    else currentView.value = 'main'
  },
  { immediate: true },
)

// À IMPLÉMENTER :
// - openRouteSel() : positionner routeRange sur [ctx.up, ctx.dn] ou ancres RP
// - doRoute() : appeler ORS puis passer à 'choice'
// - applyRoute() : auditStore.applyRoute(...) puis emit('close')
// - openDeleteSel() : positionner deleteRange sur [parts[0].s, parts[0].e]
// - doDelete() : auditStore.applyDelete(...) puis emit('close')
// - markFp() : auditStore.markFp(...) puis emit('close')
// - unmarkFp() : auditStore.unmarkFp(...)
// - undo() : auditStore.undoCorrection(...)
// - cancelRouteSel / cancelRouteChoice / cancelDelSel : retour à 'main'

function openRouteSel() { currentView.value = 'routeSel' }
function openDeleteSel() {
  deleteRange.value = [props.finding.parts[0].s, props.finding.parts[0].e]
  currentView.value = 'delSel'
}
function cancelRouteSel() { currentView.value = 'main' }
function cancelRouteChoice() { currentView.value = 'main' }
function cancelDelSel() { currentView.value = 'main' }

async function doRoute() {
  ui.showInfo('Le routage ORS sera implémenté dans une sous-étape suivante.')
}
async function applyRoute() {
  ui.showInfo('Le routage ORS sera implémenté dans une sous-étape suivante.')
}
async function doDelete() {
  try {
    await auditStore.applyDelete(props.finding.id, deleteRange.value[0], deleteRange.value[1])
    emit('close')
  } catch (e) {
    ui.showError(String(e))
  }
}
async function markFp() {
  try {
    await auditStore.markFp(props.finding.id)
    currentView.value = 'fpMode'
  } catch (e) {
    ui.showError(String(e))
  }
}
async function unmarkFp() {
  try {
    await auditStore.unmarkFp(props.finding.id)
    currentView.value = 'main'
  } catch (e) {
    ui.showError(String(e))
  }
}
async function undo() {
  try {
    await auditStore.undoCorrection(props.finding.id)
    currentView.value = 'main'
  } catch (e) {
    ui.showError(String(e))
  }
}
</script>

<style scoped>
.audit-action-panel {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 320px;
  z-index: 10;
}
</style>
```

**Points clés** :
- La vue `routeSel` et `choice` demandent une **intégration ORS**
  (deux clés, bascule 403/429, timeout 12 s, annulation). Cette partie
  est décrite dans `IHM_V2.md` §7 et §18. Elle sera portée dans une
  sous-étape **Phase 4.2** (voir §12).
- Les boutons du panneau ferment le panneau après succès (émission
  `'close'`).

---

## 8. `ConfirmExitDialog.vue`

**Rôle** : confirmation avant de quitter `/audit` si travail en cours.

**Props** :
- `modelValue: boolean`
- `workCount: number`

**Emits** :
- `'update:modelValue'`
- `'confirm'`
- `'cancel'`

```vue
<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Travail en cours</v-card-title>
      <v-card-text>
        <p>
          <b>{{ workCount }}</b> anomalie(s) ont été traitées.
        </p>
        <p class="mt-2">
          Si vous quittez maintenant, ces corrections seront perdues
          (l'audit n'est pas sauvegardé automatiquement).
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="error" @click="emit('confirm')">
          Quitter et perdre les corrections
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: boolean
  workCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
```

---

## 9. `ConfirmApplyDialog.vue`

**Rôle** : confirmation avant réécriture du GPX (point de non-retour).

**Props** :
- `modelValue: boolean`

**Emits** :
- `'update:modelValue'`
- `'confirm'`
- `'cancel'`

```vue
<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Appliquer les corrections ?</v-card-title>
      <v-card-text>
        <p>
          Le GPX va être <b>réécrit</b> avec les corrections appliquées.
          L'original sera sauvegardé en <code>.gpx.orig</code> (une seule
          fois, jamais écrasé).
        </p>
        <p class="mt-2">
          Cette action est <b>irréversible</b>. La trace sera ensuite
          marquée comme auditée et prête pour l'édition caméra.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="success" @click="emit('confirm')">
          Appliquer et fermer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
defineProps<{ modelValue: boolean }>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
```

---

## 10. `InputString.vue` (composant paramètre)

**Fichier** : `src/components/parameters/InputString.vue`

**Rôle** : composant de saisie pour le nouveau type `"string"`.

```vue
<template>
  <v-text-field
    :model-value="modelValue"
    :label="label"
    :hint="hint"
    :persistent-hint="!!hint"
    variant="outlined"
    density="comfortable"
    @update:model-value="onInput"
  />
</template>

<script setup lang="ts">
defineProps<{
  modelValue: string
  label: string
  hint?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

function onInput(value: string) {
  emit('update:modelValue', value)
}
</script>
```

**Intégration dans `ParameterCard.vue`** :

```vue
<template>
  <div>
    <!-- ... branches existantes ... -->
    <InputString
      v-else-if="setting.type === 'string'"
      v-model="localValue"
      :label="setting.description"
      :hint="setting.documentation"
    />
    <!-- ... -->
  </div>
</template>
```

---

## 11. Modifications ciblées

### 11.1 `src/router/index.ts`

```typescript
// Retirer la route 'nettoyage'
// Ajouter la route 'audit' (lazy)
{
  path: '/audit',
  name: 'audit',
  component: () => import('../views/Audit.vue'),
}
```

### 11.2 `src/components/Accueil/Circuit.vue`

```typescript
function editerCircuit() {
  editionStore.selectTrace(trace.id)
  if (trace.audit_status !== 'clean') {
    router.push({ name: 'audit', query: { traceId: trace.id } })
  } else {
    router.push({ name: 'editionCamera' })
  }
}
```

Badge du circuit :
```vue
<v-chip v-if="trace.audit_status !== 'clean'" color="orange" size="small">
  À auditer
</v-chip>
```

Icône Éditer :
```vue
<v-icon>
  {{ trace.audit_status !== 'clean' ? 'mdi-map-marker-path' : 'mdi-pencil' }}
</v-icon>
```

### 11.3 `src/views/EditionCamera.vue`

```typescript
// Garde-fou au montage
const trace = tracesStore.traces.find((t) => t.id === selectedTraceId.value)
if (trace && trace.audit_status !== 'clean') {
  router.replace({ name: 'audit', query: { traceId: trace.id } })
  return
}
```

---

## 12. Sous-étapes recommandées pour la Phase 4 (frontend)

La Phase 4 (frontend) est la plus longue car elle contient l'IHM
complète. Découpage en 4 sous-étapes :

### Sous-étape 4.1 — Squelette et liste

**Fichiers** : `Audit.vue`, `AuditToolbar.vue`, `AuditProgressChip.vue`,
`AuditFindingsPanel.vue`, `AuditSynthesis.vue`, `ConfirmExitDialog.vue`,
`ConfirmApplyDialog.vue`, `router/index.ts` (route audit).

**Sans carte**. Validation : la vue s'ouvre, la liste des findings
s'affiche, le bouton Retour fonctionne, le dialogue de sortie s'ouvre
si travail en cours.

### Sous-étape 4.2 — Carte

**Fichier** : `AuditMap.vue`.

Portage des 5 registres de couches (trace, anomalies, preview,
applied, delete). Marquage des findings sur la carte. Centrage sur
sélection.

Validation : la trace s'affiche, les findings sont visualisables,
le clic sur un finding centre la carte.

### Sous-étape 4.3 — Panneau d'action (sans ORS)

**Fichier** : `AuditActionPanel.vue`.

Vues `main`, `delSel`, `fpMode`, `undoMode`. Suppression, faux positif,
undo fonctionnels. Vues `routeSel` et `choice` présentes mais affichant
un message "à venir".

Validation : les corrections autres que routage fonctionnent.

### Sous-étape 4.4 — Intégration ORS

**Fichiers** : `AuditActionPanel.vue` (vues `routeSel` et `choice`),
nouveau composable `useAuditOrs.ts`.

Portage de la logique ORS (IHM §7 et §18) : deux clés, bascule 403/429,
timeout 12 s, annulation, test d'identité des tracés.

Validation : le routage fonctionne, les previews s'affichent, l'apply
applique bien.

---

## 13. Critères de validation globaux (Phase 4)

- [ ] `npx vue-tsc --noEmit` passe sans erreur
- [ ] La route `/audit` s'ouvre avec un `traceId` en query
- [ ] Les 6 vues du panneau d'action sont fonctionnelles
- [ ] Le dialogue de sortie s'ouvre si travail en cours
- [ ] Le bouton Appliquer est actif seulement si `pending === 0`
- [ ] Le dialogue d'application s'ouvre avant réécriture
- [ ] Après application, retour à l'Accueil + toast de succès
- [ ] Les 5 catégories de paramètres apparaissent dans le drawer
- [ ] `InputString.vue` fonctionne pour `Audit.Application.nom`
- [ ] Aucune référence à `cleaningStore` ou `cleaning_status`
- [ ] Aucune référence à `Cleaning.vue`

---

## 14. Points de vigilance

| # | Piège | Mitigation |
|---|---|---|
| 1 | `mapboxgl.Marker` impératif vs réactivité Vue | `shallowRef` pour les markers, gestion manuelle du cycle de vie |
| 2 | Fuite mémoire si `map.remove()` oublié | `onUnmounted` obligatoire |
| 3 | Carte non chargée → `whenMapReady` | File d'attente comme le HTML de référence |
| 4 | Double instance Mapbox GL (Accueil + Edition + Audit) | Chaque vue monte/démonte la sienne |
| 5 | Dialogue bloquant la navigation | `onBeforeRouteLeave` retourne `false` pour bloquer |
| 6 | Store non reset après application | `auditStore.reset()` dans `onApplyConfirmed` |
| 7 | ORS : deux requêtes (car + bike) partagées | `Promise.allSettled` + AbortController commun |
| 8 | ORS : bascule 403/429 sur clé secondaire | Gérer proprement les deux clés (voir IHM §18.4) |
| 9 | Preview ORS non nettoyée si annulation | `clearRoutePreview()` au changement de vue |
| 10 | Bouton Appliquer actif sans avoir rien traité | `canApply = hasWorkInProgress && allProcessed` |