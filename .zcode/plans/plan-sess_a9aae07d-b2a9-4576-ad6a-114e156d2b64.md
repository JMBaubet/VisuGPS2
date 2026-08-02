# Plan : Filtrage dynamique du CircuitsDrawer par viewport (zoom + centre)

## Objectif
Le `CircuitsDrawer` n'affiche plus seulement « les N plus proches du centre », mais **les circuits effectivement rendus à l'écran** (feuilles des clusters visibles + points individuels visibles), triés par distance au centre puis plafonnés à `nbrCircuits` (paramètre conservé comme limite maximale parmi les visibles).

## Architecture choisie
La carte (`map`) est une closure privée de `Map.vue`. Tout calcul basé sur `queryRenderedFeatures` / `getClusterLeaves` doit donc se faire **dans `Map.vue`**, puis le résultat (IDs visibles) est poussé dans le store `traces` pour réactivité vers le drawer. On réutilise le pattern existant (`moveend` debouncé → store → computed).

---

## Modifications

### 1. `src/stores/traces.ts` — nouvel état + getter
- Ajouter un état `visibleTraceIds = ref<Set<string>>(new Set())` (état UI éphémère, non persisté, reset au focus/close).
- Ajouter une action `setVisibleTraceIds(ids: Iterable<string>)` qui réaffecte un nouveau `Set` (déclenche la réactivité).
- Ajouter un getter `visibleTracesByDistance = computed(...)` : prend `sortedTracesByDistance.value.filter(t => visibleTraceIds.value.has(t.id))`.
- Exposer `visibleTraceIds`, `setVisibleTraceIds`, `visibleTracesByDistance` (on garde `sortedTracesByDistance` exporté, inchangé).

### 2. `src/components/Accueil/Map.vue` — calcul des IDs visibles
- Nouvelle fonction asynchrone `refreshVisibleTraceIds()` :
  - Guard : `if (!map || !mapReady) return` et **`if (tracesStore.focusedTraceId) return`** (stabilité du drawer pendant le focus, cohérent avec `onMapMoveEnd`).
  - `map.queryRenderedFeatures({ layers: ['unclustered-point'] })` → collecte des `id` des points individuels.
  - `map.queryRenderedFeatures({ layers: ['clusters'] })` → pour chaque cluster, `source.getClusterLeaves(clusterId, pointCount, 0, cb)` promisifié ; on collecte les `id` des feuilles. Gestion d'erreur par cluster (résout `[]` en cas d'erreur, n'interrompt pas les autres).
  - Dédoublonnage via `Set`, puis `tracesStore.setVisibleTraceIds([...ids])`.
- Pattern d'epoch (`visibleEpoch`) comme celui de `popupEpoch` : incrémenté au début de chaque calcul ; si l'epoch change pendant les `getClusterLeaves` asynchrones (nouveau `moveend`), on abandonne sans écrire dans le store (évite les IDs périmés).
- Branchement du déclencheur **dans le handler `moveend` debouncé existant** (`onMapMoveEnd`, Map.vue:695-706), après `updateMapCenter` :
  ```
  moveEndTimer = setTimeout(async () => {
    if (tracesStore.focusedTraceId) return
    const c = map!.getCenter()
    tracesStore.updateMapCenter(c.lat, c.lng)
    await refreshVisibleTraceIds()
  }, MOVE_END_DEBOUNCE_MS)
  ```
- Branchement **initial / après CRUD traces** via l'événement `sourcedata` (les clusters se recalculent de façon asynchrone après `source.setData`) :
  ```
  map.on('sourcedata', (e) => {
    if (e.isSourceLoaded && e.sourceId === 'traces' && e.sourceDataType === 'content') {
      refreshVisibleTraceIds()
    }
  })
  ```
  Ajouté dans `initializeMap` à côté des autres `map.on(...)` (Map.vue:741-754). Ceci couvre le 1er chargement et toute modification de la source via le `watch(traces)` (Map.vue:771-787).

### 3. `src/components/Accueil/CircuitsDrawer.vue` — consommer le nouveau getter
- Ligne 29-31 : remplacer `tracesStore.sortedTracesByDistance.slice(0, maxCircuits.value)` par `tracesStore.visibleTracesByDistance.slice(0, maxCircuits.value)`.
- `maxCircuits` reste lu depuis `Accueil.nbrCircuits.list` (paramètre conservé, devient le plafond parmi les visibles).

### 4. Documentation du paramètre — `src-tauri/settings.default.toml`
- Mettre à jour le champ `documentation` de `[Accueil.nbrCircuits.list]` (lignes 79-83) : préciser que seuls les circuits **visibles dans le viewport** (clusters + points individuels) sont pris en compte, puis plafonnés aux N plus proches du centre.

---

## Points de robustesse (alignés sur la spec fournie)
- **Anti-race** : epoch partagé entre déclenchements, abandon des résultats périmés (cf. `popupEpoch` existant).
- **Erreurs `getClusterLeaves`** (cluster_id expiré) : capturées par cluster, résolvent en `[]`, n'interrompent pas le reste.
- **Stabilité au focus** : guard `focusedTraceId` dans `refreshVisibleTraceIds` ET dans le handler debouncé (le drawer reste figé pendant un focus, comme aujourd'hui).
- **Pagination** (`getClusterLeaves`) : pour l'échelle visée (GPX de loisir, `nbrCircuits` ≤ 20), on passe `pointCount` comme limite (pas de pagination), conformément à PF-02 (< 5000 points). Si besoin futur, la promisification permet d'ajouter une boucle d'offset sans changer l'appelant.
- **Couche correcte** : on cible `'unclustered-point'` (singulier, conforme au code Map.vue:199) et non le `unclustered-points` pluriel du document générique.

## Cas de test (critères d'acceptation)
- Zoom très éloigné, 1 cluster de N circuits à l'écran → drawer affiche min(N, nbrCircuits) circuits.
- Zoom intermédiaire répartissant les circuits en plusieurs clusters visibles → drawer affiche la somme des feuilles, plafonnée.
- Zoom serré : mélange clusters + points individuels → drawer = feuilles des clusters + points individuels, dédoublonnés.
- Pan vers zone vide → drawer se vide.
- Focus (clic Info) → le drawer reste stable pendant l'animation, se rafraîchit au retour.
- Import/suppression/bascule favori → `sourcedata` déclenche le recalcul des visibles.

## Non-périmètre
- Pas de modification du backend Rust.
- Pas d'API serveur (les détails circuits viennent déjà du store local).
- `sortedTracesByDistance` reste exporté (aucun retrait, ajout pur).
