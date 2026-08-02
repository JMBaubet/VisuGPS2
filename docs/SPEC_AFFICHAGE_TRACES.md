# Spécifications — Favoris & Affichage des traces sur la carte

> ✅ **Statut** : **implémenté**. Ce document était à l'origine une spécification « à implémenter » ; les fonctionnalités décrites (favoris, affichage dégradé, couches Mapbox) sont en place. La section §8-bis (focus carte), le paramètre `Carte.Traces.dureeFlyTo` (§3) et l'état `focusedTraceId` (§10) ont été ajoutés lors de la refonte de `Circuit.vue`.

## 1. Contexte et objectif

Application **VisuGPS2** : Tauri (backend Rust + frontend Vue 3 / Vuetify 3), Mapbox GL JS v3, Pinia, paramètres TOML.

Étant donné une liste de traces GPX (circuits dont le point de départ et d'arrivée sont proches), on veut :

1. **(Dés)sélectionner des favoris** : un cluster Mapbox contenant au moins un favori doit s'afficher dans une couleur paramétrable (jaune par défaut) ; les traces favorites sont dessinées sur la carte dans cette même couleur, avec une épaisseur paramétrable (6 par défaut).
2. **Afficher / masquer une ou plusieurs traces** sur la carte : à l'importation, créer une **LineString GeoJSON** (au sens Mapbox) à partir des coordonnées GPS du fichier, la sauvegarder dans l'espace de travail selon le mode d'exécution actif, puis la rendre à la demande via un **dégradé bleu → rouge** (avec couleur intermédiaire optionnelle), épaisseur 4, au-dessus des favoris.

## 2. État déjà implémenté (ne pas refaire)

- `TraceMetadata` possède déjà `favorite: boolean` et `is_displayed: boolean` (store `src/stores/traces.ts` + miroir Rust `src-tauri/src/import_gpx.rs`).
- Commandes Tauri existantes : `get_traces`, `import_gpx_file`, `delete_trace`, `update_trace(trace_id, favorite?, is_displayed?)`.
- `Circuit.vue` émet déjà `toggle-favorite` et `toggle-display` ; `CircuitsDrawer.vue` appelle déjà `tracesStore.updateTrace(...)`. Il expose en outre deux lignes d'icônes d'action (masquées par opacité hors survol) : ligne de titre (Éditer, Groupes, Météo, Visualiser) et ligne Distance/Dénivelé (Supprimer, Exporter, Info, Affichage, Favoris). Il pilote le focus carte via `tracesStore.focusedTraceId` (cf. §8-bis).
- `Map.vue` affiche déjà les **points de départ** en clusters Mapbox (source `traces`, couches `clusters` / `cluster-count` / `unclustered-point`).
- Stockage GPX : `{app_data_dir}/{active_mode}/gpx/` ; registre `{app_data_dir}/{active_mode}/traces.json`.
- Paramètres : `src-tauri/settings.default.toml`, types `int|float|bool|secret|list|rgba|material_primary|material_extended|monitor`. **Les couleurs sont toujours au format `#RRGGBBAA`.**

## 3. Nouveaux paramètres (`src-tauri/settings.default.toml`)

Ajouter un nouveau namespace `[Carte]` (ne **pas** toucher aux namespaces existants). Tous les paramètres ci-dessous sont des feuilles (ont un champ `type`).

**Toutes les couleurs affichées sur la carte Mapbox sont de type `material_extended`** (sélecteur de palette Material Design, nuances 50→900). Le stockage reste au format `#RRGGBBAA` (validé comme `rgba` côté backend dans `settings.rs`) ; la contrainte « Material » est une garantie côté UI. Les valeurs par défaut ci-dessous sont toutes des teintes Material valides (nuance 500).

```toml
# --- Favoris ---
[Carte.Favoris.couleurCluster]
description = "Couleur des clusters contenant au moins un favori"
documentation = """
Couleur appliquée au cercle d'un cluster Mapbox dès qu'il contient
au moins une trace marquée comme favorite. Jaune Material (yellow 500).
"""
type = "material_extended"
default = "#FFEB3BFF"

[Carte.Favoris.couleurTrace]
description = "Couleur de tracé des traces favorites"
documentation = """
Couleur utilisée pour dessiner la totalité de la LineString d'une trace
marquée comme favorite. Identique à la couleur du cluster favori par défaut.
"""
type = "material_extended"
default = "#FFEB3BFF"

[Carte.Favoris.epaisseur]
description = "Épaisseur des traces favorites (px)"
documentation = """
Épaisseur de ligne (en pixels) des traces favorites sur la carte.
"""
type = "int"
default = 6
min = 1
max = 20
step = 1
unit = "px"

# --- Traces affichées (dégradé) ---
[Carte.Traces.couleurDebut]
description = "Couleur de début du dégradé"
documentation = "Couleur du tracé au point de départ de la trace. Bleu Material (blue 500)."
type = "material_extended"
default = "#2196F3FF"

[Carte.Traces.activerCouleurMilieu]
description = "Activer la couleur intermédiaire du dégradé"
documentation = """
Si activé, le dégradé passe par une couleur intermédiaire (mauve) à la
position définie par `positionCouleurMilieu`. Si désactivé, dégradé direct
début → fin.
"""
type = "bool"
default = true

[Carte.Traces.couleurMilieu]
description = "Couleur intermédiaire du dégradé"
documentation = "Couleur intermédiaire (mauve) du dégradé. Violet Material (purple 500)."
type = "material_extended"
default = "#9C27B0FF"

[Carte.Traces.positionCouleurMilieu]
description = "Position de la couleur intermédiaire (%)"
documentation = """
Position de la couleur intermédiaire le long de la trace, exprimée en
pourcentage de la distance cumulée (0 = départ, 100 = arrivée).
"""
type = "float"
default = 0.5
min = 0.0
max = 1.0
step = 0.05

[Carte.Traces.couleurFin]
description = "Couleur de fin du dégradé"
documentation = "Couleur du tracé au point d'arrivée de la trace. Rouge Material (red 500)."
type = "material_extended"
default = "#F44336FF"

[Carte.Traces.epaisseur]
description = "Épaisseur des traces affichées (px)"
documentation = "Épaisseur de ligne (en pixels) des traces affichées sur la carte."
type = "int"
default = 4
min = 1
max = 20
step = 1
unit = "px"

[Carte.Traces.dureeFlyTo]
description = "Durée de l'animation flyTo du focus carte (ms)"
documentation = """
Durée en millisecondes de l'animation de déplacement de la carte lors du focus
sur un circuit (clic sur Info dans Circuit.vue) et de son retour à la vue
précédente.
"""
type = "int"
default = 500
min = 100
max = 2000
step = 100
unit = "ms"
```

> Note 1 — cohérence : `positionCouleurMilieu` est stockée en **0.0–1.0** (float). Le libellé affiche « % » mais l'UI doit présenter 0–100 % à l'utilisateur et convertir.
>
> Note 2 — teintes existantes : les couleurs de paliers des clusters non-favoris (`#51bbd6`, `#f1f075`, `#f28cb1`) sont actuellement **codées en dur** dans `Map.vue` et ne sont pas Material. Hors périmètre immédiat, mais on pourra les paramétrer en `material_extended` de la même manière si souhaité.

## 4. Conversion couleur `#RRGGBBAA` → `rgba()`

Mapbox GL JS n'accepte pas fiablement le format `#RRGGBBAA`. **Réutiliser la fonction existante** `hexToRgbaString()` dans `src/utils/materialColors.ts` (déjà utilisée par les sélecteurs Material) plutôt que d'en créer une nouvelle :

```ts
// Déjà présent dans src/utils/materialColors.ts :
export function hexToRgbaString(hex: string): string  // "#RRGGBBAA" -> "rgba(r, g, b, a)"
```

Dans `Map.vue`, importer et utiliser :

```ts
import { hexToRgbaString } from '../../utils/materialColors'
const color = hexToRgbaString(settingsStore.getParamDef('Carte.Favoris.couleurCluster')?.value ?? '#FFEB3BFF')
```

## 5. Spécification A — Création de la LineString à l'importation (Backend Rust)

### 5.1 Fichiers concernés
- `src-tauri/src/import_gpx.rs`
- `src-tauri/src/lib.rs` (enregistrement de la nouvelle commande)

### 5.2 Collecte des coordonnées
`compute_stats` parcourt déjà tous les points dans l'ordre. En extraire **en parallèle** un `Vec<[f64; 2]>` de coordonnées **au format Mapbox `[lon, lat]`** (le GPX fournit `lat = pt.y()`, `lon = pt.x()` — donc `[lon, lat] = [pt.x(), pt.y()]`).

Recommandation : faire renvoyer par `compute_stats` un tuple `(TraceStats, points_count, coords: Vec<[f64; 2]>)`, ou créer une fonction `extract_line_coordinates(gpx) -> Vec<[f64; 2]>` réutilisant la même boucle d'itération (`gpx.tracks → segments → points`).

### 5.3 Stockage du fichier LineString et lien ID ↔ trace

**Clé commune = l'UUID `id`** (déjà généré à l'import, `uuid::Uuid::new_v4()`, et déjà présent dans `TraceMetadata.id`).

À l'import, **après** génération de l'UUID `id` (ligne `let id = uuid::Uuid::new_v4().to_string();` existante) mais **avant** la construction de `TraceMetadata` :

1. Définir `get_geojson_dir(mode_dir) -> PathBuf` retournant `mode_dir.join("geojson")` (le créer, comme `get_gpx_dir`).
2. Construire la Feature GeoJSON. Le `properties.id` **doit** reprendre l'UUID de la trace :

```jsonc
{
  "type": "Feature",
  "geometry": { "type": "LineString", "coordinates": [[lon, lat], ...] },
  "properties": { "id": "<UUID>", "name": "<name>" }
}
```

3. Écrire ce JSON dans `{mode_dir}/geojson/{id}.geojson` — le **nom du fichier est l'UUID**, qui est aussi `TraceMetadata.id`.

#### Schéma du lien ID ↔ trace

```
                  UUID (généré une fois à l'import)
                  ─────────────────────────────────
                          │
        ┌─────────────────┼──────────────────────┐
        ▼                 ▼                      ▼
   gpx/{id?}.gpx     traces.json            geojson/{id}.geojson
   (fichier GPX      [ { "id": "<UUID>",    { "type":"Feature",
    copié, nommé        "filename": "...",     "properties": {
    par nom de           "favorite": …,          "id": "<UUID>",
    fichier)             "is_displayed": …,     "name": "…"
                         "stats": {…},          },
                         …                      "geometry": LineString
                       }, … ]
                       }
```

| Artefact               | Où ?                                  | Contient l'UUID ?                  |
|------------------------|---------------------------------------|------------------------------------|
| `TraceMetadata`        | `traces.json` (registre)              | Oui — champ `id`                   |
| Fichier GPX original   | `gpx/{filename}` (nom = `filename`)   | Non — retrouvé via le registre     |
| **Géométrie LineString** | `geojson/{id}.geojson` (nom = `id`)   | Oui — `properties.id` + nom fichier |

**Pour dessiner une trace sur la carte** : le frontend connaît `TraceMetadata.id` (via `get_traces`) → il appelle `get_trace_geometry(traceId)` → le backend lit `geojson/{traceId}.geojson`. C'est l'UUID qui fait le lien, **pas** le `filename`.

#### Pourquoi `id` plutôt que `filename` ?
- **Stabilité** : un renommage de trace (futur) changerait `filename` mais pas `id`.
- **Unicité garantie** : l'UUID est la clé primaire du registre (déjà utilisée par `delete_trace` et `update_trace`).
- Le `filename` peut subir des collisions résolues par suffixe (`unique_filename`), l'UUID non.

> Décision : la géométrie n'est **pas** sérialisée dans `traces.json` (volume potentiellement important). Elle vit dans `geojson/{id}.geojson`, cohérent avec la séparation `gpx/` + `traces.json`.

### 5.4 Nouvelle commande `get_trace_geometry`

```rust
#[derive(serde::Serialize)]
pub struct TraceGeometry {
    pub id: String,
    pub geometry: serde_json::Value, // Feature GeoJSON (LineString)
}

/// Retourne la Feature LineString GeoJSON d'une trace par son identifiant.
#[tauri::command]
pub async fn get_trace_geometry(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TraceGeometry, String> { /* lit {mode_dir}/geojson/{trace_id}.geojson */ }
```

- Enregistrer `import_gpx::get_trace_geometry` dans le `invoke_handler!` de `lib.rs`.

### 5.5 Nettoyage à la suppression
Dans `delete_trace`, supprimer **aussi** `{mode_dir}/geojson/{id}.geojson` (tolérant si absent, comme pour le fichier GPX). Conserver le comportement atomique existant du registre.

### 5.6 Robustesse
- Si le GPX n'a qu'un seul point : écrire quand même une LineString valide (Mapbox tolère ≥ 2 points ; lever une erreur claire si < 2 points plutôt que de produire une géométrie invalide).
- Toute erreur de lecture/écriture doit remonter une `Err(String)` explicite en français (cohérent avec le reste du module).

## 6. Spécification B — Gestion des favoris sur la carte

### 6.1 Couleur des clusters favoris
Dans `Map.vue`, modifier la source `traces` et la couche `clusters` :

1. `buildTracesGeoJSON()` : ajouter la propriété numérique `favorite` à chaque Feature :
   ```ts
   properties: { /* existant… */ favorite: t.favorite ? 1 : 0 }
   ```
2. Sur la source `traces`, ajouter :
   ```ts
   clusterProperties: { hasFavorite: ['max', ['get', 'favorite']] }
   ```
   (`max` d'un booléen 0/1 → 1 si le cluster contient au moins un favori.)
3. Couleur de la couche `clusters` : favori en priorité, puis paliers existants :
   ```ts
   'circle-color': [
     'case', ['==', ['get', 'hasFavorite'], 1], <couleurFavori>,
     ['step', ['get', 'point_count'], '#51bbd6', 10, '#f1f075', 100, '#f28cb1'],
   ]
   ```

### 6.2 Tracé des LineStrings favorites
Ajouter une source `favorites` (GeoJSON FeatureCollection de LineStrings) et une couche ligne :

```ts
map.addSource('favorites', { type: 'geojson', data: { type:'FeatureCollection', features: [] } })
map.addLayer({
  id: 'favorites-line',
  type: 'line',
  source: 'favorites',
  layout: { 'line-cap': 'round', 'line-join': 'round' },
  paint: {
    'line-color': <couleurTraceFavori>,
    'line-width': <epaisseurFavori>, // 6
  },
})
```

- Les features proviennent de `get_trace_geometry(id)` pour chaque trace où `favorite === true`.
- Ordre d'ajout des couches : `favorites-line` **avant** la couche des traces affichées (cf. §7) pour que les traces affichées soient au-dessus.

### 6.3 Réactivité
Sur changement de `tracesStore.traces` (watch existant à étendre) :
- recréer les données de la source `traces` (points) ;
- recharger les géométries des favoris (`favorite === true`) et mettre à jour la source `favorites` ;
- recharger les géométries affichées (`is_displayed === true`) et mettre à jour la source `displayed-traces` (cf. §7).

Optimisation : maintenir un **cache** `{ [id]: Feature }` des géométries déjà chargées pour ne pas re-lire le backend à chaque bascule.

## 7. Spécification C — Affichage / masquage des traces (dégradé)

### 7.1 Source et couche
```ts
map.addSource('displayed-traces', { type: 'geojson', data: { type:'FeatureCollection', features: [] } })
map.addLayer({
  id: 'displayed-traces-line',
  type: 'line',
  source: 'displayed-traces',
  layout: { 'line-cap': 'round', 'line-join': 'round' },
  paint: {
    'line-width': <epaisseurTrace>, // 4
    'line-gradient': <expressionDégradé>,
  },
})
```

> `line-gradient` n'est applicable qu'à une source `geojson` unique ; on groupe donc toutes les traces affichées dans **une** FeatureCollection.

### 7.2 Expression de dégradé
Le dégradé s'appuie sur `line-progress` (0 → 1 le long de chaque segment). `mid` = `positionCouleurMilieu` (0–1) :

```ts
const gradient = activerCouleurMilieu
  ? ['interpolate', ['linear', ['line-progress']],
     0, hexToRgbaString(couleurDebut),
     mid, hexToRgbaString(couleurMilieu),
     1, hexToRgbaString(couleurFin)]
  : ['interpolate', ['linear', ['line-progress']],
     0, hexToRgbaString(couleurDebut),
     1, hexToRgbaString(couleurFin)]
```

- Appliquer via `map.setPaintProperty('displayed-traces-line', 'line-gradient', gradient)`.
- En cas de modification des paramètres (watch des settings), recalculer `gradient` et le repousser.

### 7.3 Contenu des features affichées
Pour chaque trace où `is_displayed === true` : charger sa LineString via `get_trace_geometry(id)` (avec le cache de §6.3) et l'ajouter à la FeatureCollection de `displayed-traces`.

## 8. z-order (synthèse des couches)

Ordre d'empilement final (du bas vers le haut) :

1. `clusters`, `cluster-count`, `unclustered-point` (points de départ)
2. `favorites-line` (LineString favoris, couleur favori, épaisseur 6)
3. `displayed-traces-line` (LineString dégradé, épaisseur 4) — **au-dessus des favoris**
4. `focus-traces-line` (LineString isolée en mode focus, même dégradé, épaisseur 4) — **au-dessus de tout**, masquée par défaut (`visibility: 'none'`)

Respecter cet ordre lors des `addLayer` (ou utiliser `map.moveLayer(...)` pour garantir la position).

Cas particulier : une trace peut être **à la fois favorite et affichée**. Elle apparaît donc deux fois (couche favori + couche affichée) ; la couche affichée recouvre la couche favori, conformément à « l'affichage doit être au-dessus du favori si besoin ».

### 8-bis. Mode focus carte (isolation d'une trace)

Le clic sur le bouton **Info** d'un circuit (`Circuit.vue`) déclenche un **focus temporaire** : la carte cadre la trace concernée et l'affiche seule, puis revient à la vue précédente à la fermeture.

**Canal de communication** (`Circuit.vue` ↔ `Map.vue` sont frères) : l'état `focusedTraceId` du store `traces.ts`. `Circuit.vue` l'affecte au clic Info et le remet à `null` à la fermeture (curseur qui quitte la carte) ; `Map.vue` l'observe via un `watch`.

**Ouverture du focus** (`Map.vue`) :
1. Sauvegarde de la vue courante (`map.getCenter()` / `map.getZoom()`) pour restauration.
2. Masquage des couches `favorites-line` et `displayed-traces-line` (`setLayoutProperty(..., 'visibility', 'none')`) — les données restent en source, le ré-affichage est donc instantané.
3. Chargement de la LineString via `getTraceGeometry(id)` et affichage dans la couche dédiée `focus-traces-line` (`visibility: 'visible'`).
4. `map.fitBounds(bounds, { padding: 60, duration })` : le centre et le zoom sont calculés automatiquement pour exploiter au mieux la surface. Les `bounds` sont calculés en parcourant les coordonnées de la LineString (`new mapboxgl.LngLatBounds()` + `extend`).

**Fermeture du focus** :
1. Masquage de `focus-traces-line`.
2. Ré-affichage de `favorites-line` et `displayed-traces-line`.
3. `map.flyTo({ center, zoom, duration })` vers la vue sauvegardée.

**Durée** : régie par le paramètre `Carte.Traces.dureeFlyTo` (cf. §3), lu via `settingsStore`.

**Points d'attention** :
- *Anti-race* : un compteur d'epoch évite d'afficher un focus après une fermeture si la géométrie arrive tard (chargement async).
- *Stabilité du tri* : pendant un focus, `moveend` **ne met pas à jour** `mapCenter` et `refreshVisibleTraceIds()` est court-circuité (sinon `visibleTracesByDistance` se recalcule et la liste `CircuitsDrawer` se réordonne). Cf. `Map.vue` `onMapMoveEnd` et `refreshVisibleTraceIds`.
- *Multi-cartes* : une seule extension Info ouverte à la fois — la prise d'un nouveau focus ferme automatiquement les autres (watcher dans `Circuit.vue`).

## 9. Lecture des paramètres côté frontend

Dans `Map.vue`, les paramètres sont déjà accessibles via `settingsStore` (chargé au montage de `Accueil.vue`). Préférer :

```ts
const v = (p: string) => settingsStore.getParamDef(p)?.value
```

avec **fallback sur la valeur par défaut** documentée si le paramètre est absent. Utiliser `hexToRgbaString(v('Carte.Favoris.couleurCluster') ?? '#FFEB3BFF')`.

Recharger le token Mapbox comme aujourd'hui ; ne pas recréer la carte si seuls les paramètres de style changent — utiliser `setPaintProperty` / `setData`.

## 10. Store frontend (`src/stores/traces.ts`)

Ajouter :

```ts
/** Récupère la géométrie (LineString) d'une trace depuis le backend. */
async function getTraceGeometry(traceId: string): Promise<GeoJSON.Feature> {
  return invoke<{ id: string; geometry: GeoJSON.Feature }>('get_trace_geometry', { traceId })
    .then(r => r.geometry)
}
```

Et un cache optionnel `Map<string, GeoJSON.Feature>` pour éviter les lectures répétées.

### État `focusedTraceId` (mode focus carte)

```ts
/**
 * id de la trace « focus » temporaire (clic sur Info dans Circuit.vue), ou null.
 * État UI éphémère observé par Map.vue pour isoler et cadrer la trace.
 */
const focusedTraceId = ref<string | null>(null)
```

État purement frontend (aucune commande Tauri, non persisté), exposé dans le `return` public du store.

### État `visibleTraceIds` et getter `visibleTracesByDistance` (filtrage viewport)

```ts
/**
 * Identifiants des traces visibles dans le viewport courant
 * (feuilles des clusters rendus + points individuels non clusterisés).
 */
const visibleTraceIds = ref<Set<string>>(new Set())

/**
 * Traces visibles, triées par distance croissante au centre.
 * Filtre sortedTracesByDistance pour ne garder que les traces
 * dont l'ID figure dans visibleTraceIds.
 */
const visibleTracesByDistance = computed(() =>
  sortedTracesByDistance.value.filter(t => visibleTraceIds.value.has(t.id)),
)

/**
 * Met à jour l'ensemble des identifiants visibles.
 * Appelé par Map.vue après queryRenderedFeatures + getClusterLeaves.
 */
function setVisibleTraceIds(ids: Iterable<string>) {
  visibleTraceIds.value = new Set(ids)
}
```

État purement frontend (aucune commande Tauri, non persisté). Mis à jour par `Map.vue` via `refreshVisibleTraceIds()` qui combine `queryRenderedFeatures` sur les couches `unclustered-point` et `clusters`, puis `getClusterLeaves` pour chaque cluster. Le calcul est différé jusqu'à l'état stable de la carte (événement `idle`) pour garantir que les clusters sont rendus.

## 11. Conventions à respecter

- `<script setup lang="ts">`, imports relatifs, commentaires en français, docstrings JSDoc comme dans les fichiers existants.
- Ordre des coordonnées : source GPX `lat/lon` → Mapbox `[lon, lat]`.
- camelCase côté TS (`traceId`) ↔ snake_case côté Rust (`trace_id`) via la conversion automatique Tauri.
- Écritures disque atomiques (tmp + rename) comme `save_registry`.
- Aucune donnée sensible (GPX) envoyée hors backend ; la géométrie transite via commande Tauri.
- Ne casser ni le clustering existant, ni le tri par distance, ni la synchro carte ↔ liste.

## 12. Checklist d'implémentation

- [x] `settings.default.toml` : ajout du namespace `[Carte.*]` (dont `dureeFlyTo`).
- [x] `import_gpx.rs` : `get_geojson_dir`, génération `geojson/{id}.geojson` à l'import, commande `get_trace_geometry`, nettoyage dans `delete_trace`.
- [x] `lib.rs` : enregistrement de `get_trace_geometry`.
- [x] `utils/materialColors.ts` : réutiliser `hexToRgbaString` (déjà existant) dans `Map.vue`.
- [x] `stores/traces.ts` : `getTraceGeometry` + cache, état `focusedTraceId`.
- [x] `Map.vue` : `favorite` dans les features, `clusterProperties.hasFavorite`, couleur cluster conditionnelle, couches `favorites-line` et `displayed-traces-line`, expression `line-gradient`, réactivité (traces + settings), ordre des couches.
- [x] `Map.vue` : couche `focus-traces-line`, helper `computeBounds`, `watch(focusedTraceId)` (sauvegarde de vue, isolation, `fitBounds`, `flyTo`), `moveend` sans màj du centre pendant le focus.
- [x] `Map.vue` : filtrage viewport — `refreshVisibleTraceIds()` avec `queryRenderedFeatures` sur `unclustered-point` et `clusters`, `getClusterLeaves` pour extraire les feuilles, dédoublonnage (`Set`), pattern d'epoch (`visibleEpoch`) anti-race. Déclenché par `scheduleVisibleRefresh()` → événement `idle` + handler `moveend` + `load`. Guard `focusedTraceId` pour stabilité du drawer.
- [x] `stores/traces.ts` : état `visibleTraceIds` (réactif `Set<string>`), action `setVisibleTraceIds()`, getter `visibleTracesByDistance`.
- [x] `CircuitsDrawer.vue` : consommation de `visibleTracesByDistance` au lieu de `sortedTracesByDistance` (paramètre `nbrCircuits` conservé comme plafond parmi les visibles).
- [x] `Circuit.vue` : refonte UI (2 lignes d'icônes d'action masquées par opacité, extension Info via `v-expand-transition`, focus via `focusedTraceId`).
- [x] Vérifier : compilation Rust (`cargo build`), `npm run build` (vue-tsc + vite), test visuel cluster favori + bascule favori/affichage + focus carte.
