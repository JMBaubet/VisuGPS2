# Plan — Clusters Mapbox des points de départ + liste triée par distance

## Constat : adapter la spec à NOTRE architecture

La spec jointe décrit une architecture autonome (`fetch('/traces.json')`, nouveau `mapStore.js` Options API, nouveau `MapClusters.vue`, nouveau `TracesList.vue`, alias `@/`). Aucun de ces éléments n'existe chez vous. Le plan ci-dessous réimplémente **les mêmes fonctionnalités** en respectant strictement vos conventions documentées dans `docs/CONVENTIONS.md` et `docs/ARCHITECTURE.md`.

| Spec (à ignorer) | Notre architecture (à respecter) |
|---|---|
| `fetch('/traces.json')` | Traces déjà chargées via `useTracesStore.loadTraces()` → `invoke('get_traces')` |
| Nouveau `mapStore.js` (Options API) | Étendre le **Setup Store** `useTracesStore` existant |
| Nouveau `MapClusters.vue` | Étendre `Map.vue` existant |
| Nouveau `TracesList.vue` (lecture seule) | Réordonner `CircuitsDrawer.vue` **en place** (choix validé) |
| `distanceService.js` (objet littéral) | Nouvel util `src/utils/geo.ts` (fonctions nommées exportées, comme `format.ts`) |
| `@/` alias | Imports **relatifs** (`../../`) |
| Token `'VOTRE_TOKEN'` en dur | Déjà lu depuis `Systeme.Key.mapBox` via `useSettingsStore` |
| Carte = traces filtrées | **Toutes les traces** sur la carte (choix validé) |

## Fonctionnalités conservées de la spec
1. Clustering Mapbox des points de départ (`stats.start_point`) avec 3 couches.
2. Tri par distance (Haversine) au centre courant de la carte.
3. Synchronisation temps réel carte ↔ liste : `moveend` → recalcul → réordonnancement.
4. Clic cluster → `easeTo` zoom d'expansion ; clic point → popup.
5. **Pas d'indicateur de distance** visible (conforme à la spec).

## Fichiers de référence à relire avant de coder
- `src/stores/traces.ts` (pattern Setup Store, types `TraceMetadata`/`Point3D`)
- `src/components/Accueil/Map.vue` (intégration Mapbox minimale à étendre)
- `src/components/Accueil/CircuitsDrawer.vue` (la `v-for` à modifier)
- `src/utils/format.ts` (modèle d'utilitaire : fonctions nommées exportées)
- `docs/CONVENTIONS.md` (règles : Setup Store, `<script setup>`, naming)

---

## Phase 1 — Utilitaire géographique `src/utils/geo.ts` (nouveau)

Créer `src/utils/geo.ts` sur le modèle de `format.ts` (fonctions nommées exportées, JSDoc, **pas** d'objet littéral contrairement à la spec).

```ts
/** Convertit des degrés en radians. */
export function toRadians(deg: number): number {
  return (deg * Math.PI) / 180
}

/**
 * Distance géodésique entre deux points (formule de Haversine), en mètres.
 * R = 6 371 000 m (cohérent avec import_gpx.rs).
 */
export function haversineMeters(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const R = 6_371_000
  const dLat = toRadians(lat2 - lat1)
  const dLon = toRadians(lon2 - lon1)
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRadians(lat1)) * Math.cos(toRadians(lat2)) * Math.sin(dLon / 2) ** 2
  return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a))
}
```

> Note : on garde le tri **côté front** car il dépend d'un état réactif (centre de la carte) qui change à chaque déplacement. Aucune commande Tauri à ajouter — le calcul est léger.

---

## Phase 2 — Étendre `src/stores/traces.ts` (centre carte + tri)

Ajouter dans le Setup Store existant (ne **pas** créer de store séparé) :

**État** (après `loading` existant) :
```ts
/** Centre courant de la carte (cohérent avec Map.vue initial). */
const mapCenter = ref<{ lat: number; lon: number }>({ lat: 43.7, lon: 2.0 })
```

**Getter** (à côté de `traceCount`) :
```ts
/** Traces triées par distance croissante au centre courant de la carte. */
const sortedTracesByDistance = computed(() =>
  [...traces.value]
    .map(t => ({
      trace: t,
      distance: haversineMeters(
        mapCenter.value.lat, mapCenter.value.lon,
        t.stats.start_point.lat, t.stats.start_point.lon,
      ),
    }))
    .sort((a, b) => a.distance - b.distance)
    .map(entry => entry.trace),
)
```
> On garde la distance uniquement pour le tri interne — elle **n'est pas** exposée ni affichée (conforme à la spec « sans indicateur »). Importer `haversineMeters` depuis `../../utils/geo`.

**Action** :
```ts
/** Met à jour le centre de la carte (appelé sur moveend). */
function updateMapCenter(lat: number, lon: number) {
  mapCenter.value = { lat, lon }
}
```

**Exposition** : ajouter `mapCenter`, `sortedTracesByDistance`, `updateMapCenter` au `return` public.

---

## Phase 3 — Étendre `src/components/Accueil/Map.vue` (clusters + centre)

Transformer le `Map.vue` minimal actuel. Ajouts :

1. **Importer** `useTracesStore` ; récupérer `const tracesStore = useTracesStore()`.
2. **Source GeoJSON clusterisée** créée au `map.on('load', ...)` à partir de **toutes** les traces :
   - `map.addSource('traces', { type: 'geojson', data, cluster: true, clusterRadius: 50, clusterMaxZoom: 14 })`
   - Coordonnées Mapbox `[lon, lat]` → `coordinates: [t.stats.start_point.lon, t.stats.start_point.lat]` (inversion impérative).
3. **3 couches** : `clusters` (circle, couleurs par paliers), `cluster-count` (symbol), `unclustered-point` (circle bleu). Styles repris de la spec §3.3.
4. **Émission du centre** : `map.on('moveend', ...)` → `tracesStore.updateMapCenter(c.lat, c.lng)` **avec debounce** (la spec §5 le recommande). Implémenter un `let moveEndTimer` + `setTimeout(150)`. Initialiser aussi le centre au `load`.
5. **Clics** : `map.on('click', 'clusters', handleClusterClick)` (récupère `cluster_id` → `getClusterExpansionZoom` → `map.easeTo`) ; `map.on('click', 'unclustered-point', handlePointClick)` (popup avec nom de la trace + coordonnées, via `formatCoordinate` de `utils/format.ts`).
6. **Curseur** : `map.on('mouseenter', 'clusters'/'unclustered-point', ...)` → `map.getCanvas().style.cursor = 'pointer'` (+ reset sur `mouseleave`).
7. **Réactivité** : `watch(() => tracesStore.traces, ...)` → si la source existe, `map.getSource('traces').setData(geojsonMiseÀJour)` pour suivre imports/suppressions sans recréer la carte.
8. **Nettoyage** `onUnmounted` : conserver `map.remove()` (les listeners partent avec).

Le token reste lu depuis `Systeme.Key.mapBox` (inchangé). Aucune modification à `Accueil.vue`.

---

## Phase 4 — Réordonner `src/components/Accueil/CircuitsDrawer.vue`

Modification unique et minimale :

```diff
- <Circuit v-for="trace in tracesStore.traces" ... />
+ <Circuit v-for="trace in tracesStore.sortedTracesByDistance" ... />
```

Le `:key="trace.id"` (id stable) est conservé → pas de problème de cohérence d'état au réordonnancement.

**Optionnel (léger)** : ajuster le sous-titre du compteur de traces pour mentionner le tri, ex. `{{ n }} trace(s) · triées par distance`. À valider à l'implémentation selon le rendu visuel.

Les interactions de gestion (favoris, suppression, import, bascule affichage) sont **conservées** : ce n'est pas en conflit avec le tri par distance — c'est bien la même liste, simplement réordonnée.

---

## Phase 5 — Vérification & cohérence

- `npm run build` (type-check TS strict + build Vite) doit passer.
- Vérifier : imports relatifs partout (pas de `@/`), aucune commande Tauri ajoutée, aucun nouveau store, types miroir Rust inchangés.
- Pas de mise à jour de `docs/COMMANDS.md` (aucune commande ajoutée).

---

## Ce qui n'est PAS fait (par cohérence avec l'architecture)
- ❌ Pas de `fetch('/traces.json')` — les traces viennent du backend.
- ❌ Pas de nouveau `MapClusters.vue` / `TracesList.vue` — on étend l'existant.
- ❌ Pas de `mapStore` séparé — on étend `useTracesStore`.
- ❌ Pas d'objet `DistanceService` — utilitaire en fonctions nommées.
- ❌ Pas de filtre `is_displayed` sur la carte (toutes les traces, selon votre choix).

## Résumé des fichiers touchés
| Fichier | Action |
|---|---|
| `src/utils/geo.ts` | **Créer** (Haversine, tri) |
| `src/stores/traces.ts` | **Modifier** (`mapCenter`, `sortedTracesByDistance`, `updateMapCenter`) |
| `src/components/Accueil/Map.vue` | **Modifier** (clusters, couches, centre, clics) |
| `src/components/Accueil/CircuitsDrawer.vue` | **Modifier** (`v-for` → `sortedTracesByDistance`) |