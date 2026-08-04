# Référence des commandes Tauri

> Catalogue exhaustif des commandes backend↔frontend de VisuGPS2.
> Source de vérité : `src-tauri/src/lib.rs` (`invoke_handler`).

## Principes

- Toutes les commandes sont définies avec `#[tauri::command]` côté Rust.
- Côté frontend, on les appelle avec `invoke('<command>', { args })` depuis `@tauri-apps/api/core`.
- **Convention de nommage** : snake_case côté Rust, camelCase côté TypeScript. Tauri fait la conversion automatiquement (ex. `traceId` TS → `trace_id` Rust).
- **Types `Option<T>` Rust** : représentés par `null` côté TS (ex. `update_trace`).
- Les types sont en miroir exact entre les structs Rust (`#[derive(Serialize)]`) et les interfaces TS (`TraceMetadata`, `TraceStats`, `Point3D`...).

## Catalogue (26 commandes)

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
| `delete_trace` | `async (app, trace_id) -> Result<(), String>` | Supprime le fichier GPX + l'entrée du registre (écriture atomique). |
| `update_trace` | `async (app, trace_id, favorite: Option<bool>, is_displayed: Option<bool>) -> Result<(), String>` | Mise à jour partielle (PATCH) d'une trace. Seuls les champs `Some(...)` sont modifiés. |
| `get_trace_geometry` | `async (app, trace_id) -> Result<TraceGeometry, String>` | Géométrie GeoJSON d'une trace (lu depuis le cache, ou régénéré depuis le GPX en cas de migration). |

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
}
```

> `#[serde(default)]` sur `favorite` et `is_displayed` assure la **rétrocompatibilité** : un `traces.json` antérieur se charge avec `false`/`false` sans erreur.

### Édition / Keyframes (`edition.rs`)

Module dédié à la persistance des keyframes et overrides de montage (Modes 1 & 2).
Le pré-calcul et la fusion (blending) sont effectués côté frontend (ils nécessitent
une instance Mapbox avec terrain) ; ce module ne gère que la lecture/écriture
atomique des fichiers JSON sur disque.

| Commande | Signature Rust | Retour |
|---|---|---|
| `has_raw_keyframes` | `(app, trace_id) -> bool` | Vrai si `keyframes/{traceId}_raw_keyframes.json` existe (cache disponible). |
| `get_raw_keyframes` | `(app, trace_id) -> Result<RawKeyframesFile, String>` | Lit le fichier de keyframes bruts (erreur si absent). |
| `save_raw_keyframes` | `(app, trace_id, file: RawKeyframesFile) -> Result<(), String>` | Écriture atomique du fichier brut (appelé à la fin du pré-calcul). |
| `get_montage_overrides` | `(app, trace_id) -> Result<MontageOverridesFile, String>` | Lit les overrides ; renvoie un fichier vierge s'il n'existe pas (sans l'écrire). |
| `save_montage_overrides` | `(app, trace_id, file: MontageOverridesFile) -> Result<(), String>` | Écriture atomique des overrides (appelé à chaque modification). |
| `delete_keyframes` | `(app, trace_id) -> Result<(), String>` | Supprime les 3 fichiers `keyframes/{traceId}_*.json` (cascade à la suppression d'une trace). |

**Types** (miroir TS dans `src/utils/keyframes.ts`) :
```rust
pub struct RawKeyframesFile {
    pub trace_id: String,
    pub total_duration: u64,        // ms
    pub total_distance: f64,        // m
    pub reference_viewport: ReferenceViewport,  // résolution canonique du pré-calcul
    pub sample_rate: u64,           // ms
    pub keyframes: Vec<Keyframe>,
}

pub struct Keyframe {
    pub time: u64,                  // ms depuis le départ
    pub cam: CamState,              // lng/lat jamais modifié par override (§6.2)
    pub traceur: TraceurState,
}

pub struct MontageOverridesFile {
    pub overrides: Vec<Override>,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,  // anticipé vide (phase 2)
    #[serde(default)]
    pub pois: Vec<serde_json::Value>,      // anticipé vide (phase 2)
}
```

> Les fichiers vivent dans `{mode_dir}/keyframes/` (voir `DATA_STORAGE.md`).
> `delete_keyframes` est appelé en cascade par `traces.ts::supprimerTrace`
> après `delete_trace`.

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

**Dernière mise à jour** : 2026-08-03
