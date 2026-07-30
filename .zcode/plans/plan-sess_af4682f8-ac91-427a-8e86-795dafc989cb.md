# Focus carte lors du clic sur Info d'un circuit

## Objectif
Au clic sur **Info** dans Circuit.vue, la carte Map.vue cadre automatiquement (fitBounds) sur la trace concernée — seule celle-ci est visible (nouveau layer dédié, les layers favoris/affichées masqués). À la fermeture de l'extension, on restaure la vue (flyTo retour) et on réaffiche les autres traces.

## Architecture
Circuit.vue et Map.vue sont **frères** ( enfants de `Accueil.vue`), ils ne communiquent que via les stores. Le canal utilisé pour le focus sera un nouvel état **`focusedTraceId`** dans `tracesStore`, observé par un `watch` dans Map.vue.

Le paramètre de durée du flyTo est ajouté via le système de settings dynamique (TOML → backend générique → store → UI) — **aucune modification Rust**.

---

## Fichiers modifiés (5)

### 1. `src-tauri/settings.default.toml` — nouveau paramètre
Dans la section `[Carte.Traces...]` (après `epaisseur`, ~ligne 166), ajouter un paramètre `int` borné :
```toml
[Carte.Traces.dureeFlyTo]
description = "Durée de l'animation flyTo du focus carte (ms)"
documentation = """
Durée en millisecondes de l'animation de déplacement de la carte lors du focus
sur un circuit (clic sur Info) et de son retour à la vue précédente.
"""
type = "int"
default = 500
min = 100
max = 2000
step = 100
unit = "ms"
```
Le backend `flatten_settings` (settings.rs) le découvrira automatiquement.

### 2. `src/components/Accueil/SettingsDrawer.vue` — entrée UI
Ajouter un `v-list-item` sous le subheader « Carte — Traces affichées » (~ligne 124) :
```vue
<v-list-item prepend-icon="mdi-timer-outline" title="Durée d'animation du focus"
  value="carte-traces-flyto" @click="openParam('Carte.Traces.dureeFlyTo')"></v-list-item>
```
`ParameterCard` + `InputInt` le rendront automatiquement (slider + champ, comme `epaisseur`).

### 3. `src/stores/traces.ts` — état `focusedTraceId`
Ajouter :
```ts
/** id de la trace « focus » temporaire (clic Info), ou null. Piloté par Map.vue. */
const focusedTraceId = ref<string | null>(null)
```
Exposé dans le `return` du store. Pas de commande backend : c'est un état UI éphémère.

### 4. `src/components/Accueil/Map.vue` — cœur de la fonctionnalité

**(a) Nouveau layer `focus-traces-line`** — créé dans `onLoad` (après `addDisplayedTracesLayer()`). Clone du style de `displayed-traces-line` (dégradé via `buildGradientExpression()`, `lineMetrics: true`), épaisseur `Carte.Traces.epaisseur`. Source `focus-traces` initialisée vide. **Initialement invisible** (`visibility: 'none'`).

**(b) Sauvegarde de la vue** — deux `let` module-level :
```ts
let savedCenter: mapboxgl.LngLat | null = null
let savedZoom = 0
```

**(c) Helper `computeBounds(feature)`** — parcourt `geometry.coordinates` de la LineString et construit un `new mapboxgl.LngLatBounds()` via `.extend()`. (Pas de turf : le projet n'en utilise pas.)

**(d) `watch(() => tracesStore.focusedTraceId)`** avec garde anti-race (epoch) :
```ts
let focusEpoch = 0
watch(() => tracesStore.focusedTraceId, async (newId) => {
  if (!map || !mapReady) return
  const epoch = ++focusEpoch
  const duration = getParam('Carte.Traces.dureeFlyTo') ?? 500

  if (newId) {
    // --- OUVERTURE ---
    savedCenter = map.getCenter(); savedZoom = map.getZoom()
    map.setLayoutProperty('favorites-line', 'visibility', 'none')
    map.setLayoutProperty('displayed-traces-line', 'visibility', 'none')
    const geom = await tracesStore.getTraceGeometry(newId)
    if (epoch !== focusEpoch) return          // annulé entre-temps
    const src = map.getSource('focus-traces') as mapboxgl.GeoJSONSource
    src.setData({ type: 'FeatureCollection', features: [geom] })
    map.setLayoutProperty('focus-traces-line', 'visibility', 'visible')
    map.fitBounds(computeBounds(geom), { padding: 60, duration, essential: true })
  } else {
    // --- FERMETURE ---
    map.setLayoutProperty('focus-traces-line', 'visibility', 'none')
    map.setLayoutProperty('favorites-line', 'visibility', 'visible')
    map.setLayoutProperty('displayed-traces-line', 'visibility', 'visible')
    if (savedCenter) {
      map.flyTo({ center: savedCenter, zoom: savedZoom, duration, essential: true })
    }
  }
})
```
Note : masquer via `setLayoutProperty(..., 'visibility', 'none')` conserve les données en source — la réaffichage est instantané, pas de reload.

**(e) Le watcher existant `tracesStore.traces` (deep)** ne se déclenche pas pendant le focus (pas de modification de `traces`), donc pas de conflit avec `refreshLineLayers()`.

### 5. `src/components/Accueil/Circuit.vue` — relier Info au store

Transformer le bouton Info : au lieu d'un simple toggle local, il pilote aussi `tracesStore.focusedTraceId`. Gestion multi-cartes propre (une seule extension ouverte à la fois) :

```ts
import { useTracesStore } from '../../stores/traces'
const tracesStore = useTracesStore()

// Remplace l'inline toggle du bouton Info par :
function onInfoClick() {
  if (infoExpanded.value) {
    infoExpanded.value = false
    if (tracesStore.focusedTraceId === props.trace.id) tracesStore.focusedTraceId = null
  } else {
    infoExpanded.value = true
    tracesStore.focusedTraceId = props.trace.id
  }
}

// mouseleave de la section : fermer + libérer le focus
function onInfoLeave() {
  infoExpanded.value = false
  if (tracesStore.focusedTraceId === props.trace.id) tracesStore.focusedTraceId = null
}

// Synchronisation : si une autre carte prend le focus, fermer celle-ci
watch(() => tracesStore.focusedTraceId, (newId) => {
  if (newId !== props.trace.id && infoExpanded.value) infoExpanded.value = false
})
```
- Bouton Info : `@click="onInfoClick"` (au lieu de `infoExpanded = !infoExpanded`).
- Section extension : `@mouseleave="onInfoLeave"` (au lieu de `infoExpanded = false`).
- Ajout de l'import `watch` dans les imports `vue`.

---

## Points de vigilance / choix techniques
- **fitBounds (aller) vs flyTo (retour)** : fitBounds calcule automatiquement centre+zoom pour utiliser au mieux la surface (demande explicite). Il accepte une option `duration`, donc animé. Le retour utilise flyTo vers le centre/zoom sauvegardés. Les deux respectent le paramètre `dureeFlyTo`.
- **Garde anti-race (epoch)** : si l'utilisateur ferme l'extension pendant le chargement async de la géométrie, on évite d'afficher le focus après un retour.
- **Multi-cartes** : le watcher de Circuit.vue ferme automatiquement une extension si une autre carte prend le focus.
- **Aucun changement Rust**, aucun changement CircuitsDrawer.vue (les emits delete/toggle-* restent intacts).

## Vérification finale
- `npx vue-tsc --noEmit` (compilation TS + template).
- Test manuel via `npm run tauri dev` : clic Info → cadrage + isolation, mouseleave → retour + réaffichage.