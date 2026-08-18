# Référence des commandes Tauri

> Catalogue exhaustif des commandes backend↔frontend de VisuGPS2.
> Source de vérité : `src-tauri/src/lib.rs` (`invoke_handler`).

## Principes

- Toutes les commandes sont définies avec `#[tauri::command]` côté Rust.
- Côté frontend, on les appelle avec `invoke('<command>', { args })` depuis `@tauri-apps/api/core`.
- **Convention de nommage** : snake_case côté Rust, camelCase côté TypeScript. Tauri fait la conversion automatiquement (ex. `traceId` TS → `trace_id` Rust).
- **Types `Option<T>` Rust** : représentés par `null` côté TS (ex. `update_trace`).
- Les types sont en miroir exact entre les structs Rust (`#[derive(Serialize)]`) et les interfaces TS (`TraceMetadata`, `TraceStats`, `Point3D`...).

## Catalogue (29 commandes)

### Application

| Commande | Signature Rust | Module | Retour |
|---|---|---|---|
| `exit_app` | `(app_handle)` | `lib.rs` | Quitte l'application (`app_handle.exit(0)`). |

### Affichage / Multi-écrans (`display.rs`)

| Commande | Signature Rust | Retour |
|---|---|---|
| `get_displays` | `(_window) -> Vec<MonitorInfo>` | Liste des écrans détectés (nom, position, taille, scale factor). |
| `open_second_window` | `async (app) -> Result<(), String>` | Place la fenêtre `screen-bis` sur l'écran secondaire (critère lu dans `Affichage.moniteurs.secondaire`). |
| `close_second_window` | `async (app) -> Result<(), String>` | Quitte le fullscreen puis masque `screen-bis`. |

**Type `MonitorInfo`** :
```rust
pub struct MonitorInfo {
    pub name: Option<String>,      // Nom de l'écran
    pub position: (i32, i32),      // X, Y
    pub size: (u32, u32),          // largeur, hauteur (pixels)
    pub scale_factor: f64,         // DPI scaling
}
```

### Modes d'exécution (`gestionMode.rs`)

| Commande | Signature Rust | Retour |
|---|---|---|
| `get_execution_env` | `async (app) -> Result<ExecutionEnv, String>` | `is_dev`, `active_mode_dev`, `active_mode_prod`. |
| `get_modes` | `async (app) -> Result<Vec<ModeInfo>, String>` | Liste des modes depuis `ModeExe.toml`. |
| `create_mode` | `async (app, nom, descrition) -> Result<(), String>` | Crée un mode `EVAL_*` + son dossier. |
| `update_mode` | `async (app, old_nom, new_nom, descrition) -> Result<(), String>` | Modifie un mode (renomme dossier, màj `.env` si actif). |
| `delete_mode` | `async (app, nom) -> Result<(), String>` | Supprime un mode (interdit pour `OPE` et le mode actif). |
| `select_mode` | `async (app, nom) -> Result<(), String>` | Active un mode (écrit `.env`), puis `exit(0)` (dev) ou `restart()` (prod). |

**Types** :
```rust
pub struct ExecutionEnv {
    pub is_dev: bool,
    pub active_mode_dev: String,
    pub active_mode_prod: String,
}

pub struct ModeInfo {
    pub nom: String,
    pub descrition: String,
    pub création: String,    // serde rename (accent conservé)
    pub révision: String,    // serde rename (accent conservé)
}
```

**Règles métier** : `OPE` est non modifiable/supprimable. Les autres modes doivent être préfixés `EVAL_`. Impossible de supprimer le mode actif.

### Paramètres / Settings (`settings.rs`)

| Commande | Signature Rust | Retour |
|---|---|---|
| `get_settings` | `async (state) -> Result<Vec<SettingDefinition>, String>` | Tous les paramètres fusionnés (défaut + surcharges). Secrets masqués `********`. |
| `get_settings_meta` | `async (state) -> Result<SettingsMeta, String>` | Organisation du drawer (table `[_meta]` du TOML : vues, groupes système, actions, handlers, libellés/icônes des catégories). |
| `update_setting` | `async (state, path, value) -> Result<(), String>` | Valide et persiste un paramètre (chiffre les secrets). |
| `reset_setting` | `async (state, path) -> Result<(), String>` | Supprime la surcharge (retour à la valeur par défaut). |
| `get_setting_value` | `async (state, path) -> Result<Value, String>` | Valeur effective (déchiffrée pour les secrets). Usage interne (ex. token Mapbox). |

> Utilise un `State<SettingsState>` (géré via `app.manage()` dans le `setup`), pas `AppHandle`.

### Traces GPX (`import_gpx.rs`)

| Commande | Signature Rust | Retour |
|---|---|---|
| `import_gpx_file` | `async (app) -> Result<TraceMetadata, String>` | Sélecteur natif, parse, hash, copie, stats, màj registre. |
| `get_traces` | `async (app) -> Result<Vec<TraceMetadata>, String>` | Liste les traces du mode actif depuis `traces.json`. |
| `delete_trace` | `async (app, trace_id) -> Result<(), String>` | Supprime le fichier GPX + le GeoJSON + les **deux** fichiers keyframes (`_169`/`_43`, + l'ancien non suffixé) + le fichier de travail `cleaning/{id}.json` + le backup `{filename}.gpx.orig` + l'entrée du registre (écriture atomique). |
| `update_trace` | `async (app, trace_id, favorite: Option<bool>, is_displayed: Option<bool>) -> Result<(), String>` | Mise à jour partielle (PATCH) d'une trace. Seuls les champs `Some(...)` sont modifiés. |
| `get_trace_geometry` | `async (app, trace_id) -> Result<TraceGeometry, String>` | Géométrie GeoJSON d'une trace (lu depuis le cache, ou régénéré depuis le GPX en cas de migration). |
| `get_trace_points` | `async (app, trace_id) -> Result<TracePoints, String>` | Points d'une trace avec altitude et distance cumulée 3D (re-parse le GPX original à la demande). |
| `save_keyframes` | `async (app, trace_id, viewport_aspect: String, keyframes_json: Value) -> Result<(), String>` | Sauvegarde un jeu de keyframes dans `keyframes/{trace_id}_{ratio}.json` (`_169` pour `"16:9"`, `_43` pour `"4:3"` ; écriture atomique tmp + rename). |
| `get_keyframes` | `async (app, trace_id, viewport_aspect: String) -> Result<Option<Value>, String>` | Charge les keyframes persistés d'une trace **pour un ratio donné** (`"16:9"`/`"4:3"`). Retourne `None` si le fichier est absent. |
| `delete_keyframes` | `async (app, trace_id, viewport_aspect: String) -> Result<(), String>` | Supprime le fichier `keyframes/{trace_id}_{ratio}.json` d'un ratio donné (tolérant si absent). |

### Nettoyage de trace (`cleaning.rs`)

> Une trace n'est **valide** que si elle est « propre » (`cleaning_status = "clean"`). La détection ne fait que **proposer** des cas ; la **validation de chaque cas est de la responsabilité de l'utilisateur** (`corrected` / `kept`), et le GPX original n'est remplacé qu'à la finalisation, une fois **tous** les cas validés.

| Commande | Signature Rust | Retour |
|---|---|---|
| `detect_trace_anomalies` | `async (app, trace_id, tolerance_deg: f64) -> Result<Vec<CleaningCase>, String>` | Détecte les anomalies (rebroussements ~180° sous la tolérance de cap) en re-parsant le GPX original. Aucune persistance. |
| `get_cleaning_state` | `async (app, trace_id, tolerance_deg: f64) -> Result<CleaningState, String>` | État de nettoyage : le fichier de travail `cleaning/{trace_id}.json` s'il existe (corrections en cours), sinon une détection fraîche. |
| `save_cleaning_state` | `async (app, trace_id, state_json: Value) -> Result<(), String>` | Sauvegarde partielle (écriture atomique) ; passe la trace en `"in_progress"`. Le GPX original reste intact. |
| `reset_cleaning` | `async (app, trace_id) -> Result<(), String>` | Abandonne les corrections (supprime le fichier de travail) et remet la trace en `"needs_review"`. |
| `finalize_cleaning` | `async (app, trace_id, state_json: Value) -> Result<TraceMetadata, String>` | Applique les corrections validées, génère le GPX nettoyé, **sauvegarde l'original en `{filename}.gpx.orig`**, régénère geojson/stats/hash, passe la trace en `"clean"`. Refuse tant qu'un cas est `"pending"`. |

**Type `CleaningState`** (miroir TS `CleaningState` dans `src/stores/cleaning.ts`) :
```rust
pub struct CleaningCase {
    pub id: String,                    // "c1", "c2", …
    pub kind: CleaningCaseKind,        // spike | out_and_back | parallel
    pub start_index: usize,            // zone d'intérêt (index originaux)
    pub end_index: usize,
    pub apex_indices: Vec<usize>,      // points de rebroussement
    pub bearing_delta_deg: f64,        // écart de cap max mesuré
    pub suggested_delete_ranges: Vec<[usize; 2]>, // proposition (pré-remplissage)
    pub state: String,                 // "pending" | "corrected" | "kept"
    pub correction: Correction,
}

pub struct Correction {
    pub delete_ranges: Vec<[usize; 2]>,   // index originaux à supprimer
    pub insert_points: Vec<InsertPoint>,  // points à ajouter (après un index original)
}

pub struct InsertPoint {
    pub after_index: usize,               // index original du point conservé après lequel insérer
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,                 // interpolation proposée côté frontend
    pub time: Option<String>,             // ISO 8601, écrit tel quel dans le GPX
}
```

### Paramètres / Settings (`settings.rs`)

**Types `TracePoint` et `TracePoints`** (miroir TS `TracePoint` dans `src/stores/traces.ts`) :
```rust
pub struct TracePoint {
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,       // altitude (peut être None)
    pub distance_m: f64,        // distance cumulée 3D depuis le départ (mètres)
}

pub struct TracePoints {
    pub id: String,             // UUID de la trace
    pub points: Vec<TracePoint>,
}
```

**Type `TraceMetadata`** (miroir TS dans `src/stores/traces.ts`) :
```rust
pub struct TraceMetadata {
    pub id: String,                       // UUID v4
    pub name: String,
    pub source: String,                   // Strava, Garmin Connect, OpenRunner, RideWithGPS, Inconnu
    pub source_url: Option<String>,
    pub activity_type: Option<String>,    // running, cycling, hiking…
    pub filename: String,                 // nom dans gpx/
    pub import_date: String,              // ISO 8601 UTC
    pub stats: TraceStats,
    pub hash: String,                     // "sha256:…"
    #[serde(default)]
    pub favorite: bool,                   // marquer comme favori (persisté)
    #[serde(default)]
    pub is_displayed: bool,               // afficher sur la carte (persisté)
    #[serde(default = "default_cleaning_status")]
    pub cleaning_status: String,          // "clean" | "needs_review" | "in_progress"
}
```

> `#[serde(default)]` sur `favorite`/`is_displayed` et `#[serde(default = "default_cleaning_status")]` sur `cleaning_status` assurent la **rétrocompatibilité** : un `traces.json` antérieur se charge sans erreur (`false`/`false`/`"clean"`). En outre, `load_registry` **normalise** toute chaîne vide en `"clean"` (registres corrompus ou intermédiaires).

## Exemples d'appel côté frontend

```typescript
import { invoke } from '@tauri-apps/api/core'

// Lecture
const traces = await invoke<TraceMetadata[]>('get_traces')

// Suppression (camelCase → trace_id)
await invoke('delete_trace', { traceId: id })

// Mise à jour partielle (null = ne pas modifier)
await invoke('update_trace', {
  traceId: id,
  favorite: true,        // ou null pour ignorer
  isDisplayed: null,     // ignoré
})
```

## Ajouter une nouvelle commande

1. Définir `#[tauri::command] pub async fn ma_commande(...) -> Result<T, String>` dans le module Rust approprié.
2. L'ajouter dans `lib.rs` → `tauri::generate_handler![..., import_gpx::ma_commande]`.
3. Si la commande utilise un plugin, l'enregistrer via `.plugin(...)` dans `lib.rs` et la permission dans `capabilities/default.json`.
4. Côté TS, appeler avec `invoke('ma_commande', { argsEnCamelCase })`.
5. **Mettre à jour ce fichier** (`COMMANDS.md`).

---

**Dernière mise à jour** : 2026-08-18
