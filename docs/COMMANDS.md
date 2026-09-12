# Référence des commandes Tauri

> Catalogue exhaustif des commandes backend↔frontend de VisuGPS2.
> Source de vérité : `src-tauri/src/lib.rs` (`invoke_handler`).

## Principes

- Toutes les commandes sont définies avec `#[tauri::command]` côté Rust.
- Côté frontend, on les appelle avec `invoke('<command>', { args })` depuis `@tauri-apps/api/core`.
- **Convention de nommage** : snake_case côté Rust, camelCase côté TypeScript. Tauri fait la conversion automatiquement (ex. `traceId` TS → `trace_id` Rust).
- **Types `Option<T>` Rust** : représentés par `null` côté TS (ex. `update_trace`).
- Les types sont en miroir exact entre les structs Rust (`#[derive(Serialize)]`) et les interfaces TS (`TraceMetadata`, `TraceStats`, `Point3D`...).

## Catalogue (34 commandes)

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
| `delete_trace` | `async (app, trace_id) -> Result<(), String>` | Supprime le **dossier entier** `traces/{trace_id}/` (GPX, backup `.orig`, GeoJSON, keyframes, nettoyage) + l'entrée du registre (écriture atomique). |
| `update_trace` | `async (app, trace_id, favorite: Option<bool>, is_displayed: Option<bool>) -> Result<(), String>` | Mise à jour partielle (PATCH) d'une trace. Seuls les champs `Some(...)` sont modifiés. |
| `get_trace_geometry` | `async (app, trace_id) -> Result<TraceGeometry, String>` | Géométrie GeoJSON d'une trace (lu depuis le cache, ou régénéré depuis le GPX en cas de migration). |
| `get_trace_points` | `async (app, trace_id) -> Result<TracePoints, String>` | Points d'une trace avec altitude et distance cumulée 3D (re-parse le GPX original à la demande). |
| `save_keyframes` | `async (app, trace_id, viewport_aspect: String, keyframes_json: Value) -> Result<(), String>` | Sauvegarde un jeu de keyframes dans `keyframes/{trace_id}_{ratio}.json` (`_169` pour `"16:9"`, `_43` pour `"4:3"` ; écriture atomique tmp + rename). |
| `get_keyframes` | `async (app, trace_id, viewport_aspect: String) -> Result<Option<Value>, String>` | Charge les keyframes persistés d'une trace **pour un ratio donné** (`"16:9"`/`"4:3"`). Retourne `None` si le fichier est absent. |
| `delete_keyframes` | `async (app, trace_id, viewport_aspect: String) -> Result<(), String>` | Supprime le fichier `keyframes/{trace_id}_{ratio}.json` d'un ratio donné (tolérant si absent). |

### Audit GPX (`gpx_audit/commands.rs`)

> Le module Audit GPX **remplace** l'ancien module Nettoyage. Il détecte deux
> familles d'anomalies sur la trace consolidée : les **aller-retours** ponctuels
> (AR — rebonds, aiguilles de traceur) et les **boucles de giratoire** (RP — 270°,
> 360° et plus). Les corrections disponibles sont la **suppression de points**, le
> **routage OpenRouteService** et le marquage en **faux positif**, chacune
> annulable par anomalie. Une trace n'est **valide** que si elle est auditée
> (`audit_status = "clean"`) ; elle est sinon redirigée vers la vue `/audit`.
>
> **Les findings sont volatils** (décision 6) : l'état de travail vit dans le
> store Pinia `src/stores/audit.ts` et n'est **pas persisté** entre deux sessions.
> Seuls le GPX réécrit et `audit_status` survivent à la fermeture.
>
> Les commandes `audit_map_overlay`, `audit_delete_preview`,
> `audit_routes_identical` sont **pures** (aucun accès disque, aucun `AppHandle`) :
> ce sont des calculs délégués au backend parce qu'ils sont métriques.

| Commande | Signature Rust | Retour |
|---|---|---|
| `audit_run_detection` | `async (app, trace_id: String, params: AuditParams) -> Result<AuditDetectionResult, String>` | Charge le GPX de la trace, **consolide** les points (seuil `Audit.Consolidation.seuil`), exécute `detect_ar` puis `detect_rp` et retourne la trace de travail (`points`), les findings et la distance totale. Aucune persistance. Seule commande à lire le GPX. |
| `audit_map_overlay` | `(points: Vec<AuditPoint>, findings: Vec<Finding>, close_m: f64) -> Result<Vec<FindingOverlay>, String>` | Éléments de rendu des anomalies : ancres de routage des boucles RP et étiquettes des points. Recalculé à chaque rendu de carte (lazy, à l'image de `rpAnchors` du HTML de référence). |
| `audit_delete_preview` | `(points: Vec<AuditPoint>, finding: Finding, start: usize, end: usize, close_m: f64) -> Result<DeletePreview, String>` | Aperçu **prospectif** d'une suppression sur `[start, end]` — la trace de travail n'est pas modifiée. Recalculé à chaque mouvement de curseur. |
| `audit_routes_identical` | `(car: Vec<LatLon>, car_distance: f64, bike: Vec<LatLon>, bike_distance: f64) -> bool` | Compare les deux tracés ORS (voiture / vélo) : longueurs à **2 %** près et distance de Hausdorff discrète ≤ **15 m** dans les deux sens. Calcul métrique ; la requête réseau vit dans la composable `useAuditOrs`. |
| `audit_apply_delete` | `(trace_id: String, points, findings, finding_id: String, ds: usize, de: usize, next_point_id: u32) -> Result<AuditState, String>` | Applique la suppression des points `[ds, de]` : retire les points, resynchronise les index, absorbe les findings faux positifs **imbriqués** (garde de nesting) et enregistre l'undo. Retourne l'état d'audit complet. |
| `audit_apply_route` | `(trace_id: String, points, findings, finding_id: String, start: usize, end: usize, coords: Vec<LatLon>, profile: String, next_point_id: u32) -> Result<AuditState, String>` | Remplace le segment `[start, end]` par le tracé OpenRouteService `coords` (profil `"car"` / `"bike"`), avec les mêmes resynchronisation, absorption et undo que la suppression. |
| `audit_mark_fp` | `(findings: Vec<Finding>, finding_id: String) -> Result<Vec<Finding>, String>` | Marque une anomalie en **faux positif** (`status = Fp`). Refuse si elle est déjà `Fp`. |
| `audit_unmark_fp` | `(findings: Vec<Finding>, finding_id: String) -> Result<Vec<Finding>, String>` | Symétrique de `audit_mark_fp` : repasse une anomalie `Fp` en `Pending`. |
| `audit_undo_correction` | `(trace_id: String, points, findings, finding_id: String) -> Result<AuditState, String>` | Annule la correction portée par une anomalie (restaure les points d'origine, retire les points insérés, réintègre les faux positifs absorbés). |
| `audit_validate` | `async (app, trace_id: String, points: Vec<AuditPoint>, findings: Vec<Finding>) -> Result<TraceMetadata, String>` | **Point de non-retour.** Refuse tant qu'un finding est `pending`. Réécrit le GPX (backup `{filename}.gpx.orig` posé **une seule fois**, jamais écrasé), régénère geojson/stats/hash et pose `audit_status = "clean"`. Le GPX est écrit **avant** `traces.json` : un échec de réécriture laisse le statut intact. |

**Type `AuditParams`** (miroir TS `AuditParams` dans `src/stores/audit.ts`) — assemblé par la vue depuis les réglages `Audit.*` :
```rust
pub struct AuditParams {
    pub consol_m: f64,   // Audit.Consolidation.seuil      (défaut 0.5 m)
    pub tol_deg: f64,    // Audit.AR.toleranceDeg          (défaut 20°)
    pub pair_m: f64,     // Audit.AR.seuilPaireM           (défaut 50 m)
    pub maxpairs: u32,   // Audit.AR.maxPaires             (défaut 5)
    pub seg_m: f64,      // Audit.AR.branchesMaxM          (défaut 200 m)
    pub close_m: f64,    // Audit.RP.seuilFermetureM       (défaut 15 m)
    pub angle_deg: u32,  // Audit.RP.angleMinDeg           (défaut 270°)
}
```

**Type `AuditPoint`** — point de la trace de travail, identifié par un **id stable** (et non par son index) :
```rust
pub struct AuditPoint {
    pub id: u32,           // id stable, jamais réutilisé (next_point_id)
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
}
```

**Type `Finding`** — une anomalie détectée. Les index `peak` / `pairs` / `parts` / `core_ids` / `zone_ids` sont exprimés **dans l'espace de la trace de travail courante** (invariant C4) ; les `*_ids` sont les identifiants **stables** qui survivent à une suppression :
```rust
pub struct Finding {
    pub id: String,                    // "ar-1", "rp-2", …
    pub kind: FindingKind,             // Ar | Rp
    pub label: String,
    pub summary: String,
    pub peak: usize,                   // index du sommet
    pub peak_id: u32,
    pub pairs: Vec<FindingPair>,       // paires miroir (aid, bid, a, b, d)
    pub pair_idx: Vec<usize>,          // [a1, b1, a2, b2, …]
    pub ecart: Option<f64>,            // écart de cap (AR)
    pub total_angle: Option<i32>,      // angle cumulé signé (RP)
    pub turn_text: Option<String>,     // « 1 tour », « 3/4 de tour »…
    pub core_ids: Vec<u32>,
    pub zone_ids: Vec<u32>,
    pub ctx_ids: FindingContextIds,
    pub ctx: FindingContext,
    pub parts: Vec<FindingPart>,       // segments colorés (PartRole)
    pub status: FindingStatus,         // Pending | Corrected | Fp
    pub correction: Option<CorrectionType>,  // Delete | Route | Fp
    pub undo: Option<UndoRecord>,      // Delete | Route — pour l'annulation
}
```

**Types `AuditState` et `AuditDetectionResult`** — respectivement l'état de travail retourné par les commandes de correction, et le résultat de la détection initiale :
```rust
pub struct AuditDetectionResult {
    pub trace_id: String,
    pub points: Vec<AuditPoint>,       // trace consolidée (trace de travail initiale)
    pub total_distance_m: f64,
    pub findings: Vec<Finding>,
    pub params: AuditParams,
    pub duration_ms: u64,
}

pub struct AuditState {
    pub trace_id: String,
    pub points: Vec<AuditPoint>,
    pub findings: Vec<Finding>,
    pub next_point_id: u32,
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
    #[serde(default = "default_audit_status")]
    pub audit_status: String,             // "clean" | "needs_review"
}
```

> `#[serde(default)]` sur `favorite`/`is_displayed` et `#[serde(default = "default_audit_status")]` sur `audit_status` assurent la **rétrocompatibilité** : un `traces.json` antérieur se charge sans erreur (`false`/`false`/`"needs_review"`).
>
> **Registres pré-audit (D1)** : `load_registry` détecte la présence de la clé `"cleaning_status"` (format des versions antérieures au module Audit) et retourne alors une liste **vide**, sans jamais réécrire le fichier. Les traces concernées disparaissent de l'interface, mais leurs fichiers GPX restent intacts sur disque — voir [DATA_STORAGE.md](./DATA_STORAGE.md). Le registre est réécrit au prochain import, au nouveau format.

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

**Dernière mise à jour** : 2026-08-19
