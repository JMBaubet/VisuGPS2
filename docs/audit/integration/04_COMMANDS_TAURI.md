# Livrable 4 — Commandes Tauri du module Audit

**Fichier cible** : `src-tauri/src/gpx_audit/commands.rs`

**Rôle** : exposer 7 commandes Tauri publiques, appelées par le store
Pinia `useAuditStore` (Livrable 2). Chaque commande est une fonction
pure (pas d'état persistant côté Rust) qui reçoit les données
nécessaires et retourne un résultat.

**Enregistrement** : dans `lib.rs`, à côté des commandes existantes.

---

## ⚠️ Avenant au Livrable 1

Avant d'implémenter `commands.rs`, appliquer cet avenant à
`src-tauri/src/gpx_audit/types.rs` :

> Ajouter `#[serde(rename_all = "camelCase")]` sur **toutes les structs**
> de `types.rs` (pas sur les enums, déjà couvertes par leur propre
> `rename_all`).

**Raison** : Tauri 2.x convertit automatiquement les **arguments** d'appel
(`traceId` → `trace_id`) mais **pas les retours** de commande. Pour que le
front reçoive du camelCase (cohérent avec `src/stores/audit.ts`), il faut
côté Rust que la sérialisation produise du camelCase.

**Impact** : les champs Rust restent en `snake_case` (convention Rust),
mais leur sérialisation JSON devient `camelCase`. Exemple :

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingPair {
    pub aid: u32,   // -> JSON : "aid"
    pub bid: u32,   // -> JSON : "bid"
    pub a: usize,   // -> JSON : "a"
    pub b: usize,   // -> JSON : "b"
    pub d: f64,     // -> JSON : "d"
}
```

`FindingPart` avec `pub s: usize` deviendra `"s"` en JSON — pas de
changement car le nom est déjà mono-caractère.

**Cas particuliers** :
- `FindingPair`, `FindingPart`, `LatLon` : pas de changement visible
- `Finding` avec `peak_id` → `"peakId"`, `total_angle` → `"totalAngle"`,
  `pair_idx` → `"pairIdx"`, `zone_ids` → `"zoneIds"`, `ctx_ids` → `"ctxIds"`,
  `core_ids` → `"coreIds"`, `turn_text` → `"turnText"` ✓
- `AuditDetectionResult` avec `trace_id`, `total_distance_m`,
  `duration_ms` → camelCase ✓
- `AuditParams` avec `consol_m`, `tol_deg`, ... → camelCase ✓
- `UndoDelete` avec `orig_pts`, `anchor_left_id`, ... → camelCase ✓
- `UndoRoute` avec `inserted_ids`, `route_pts`, `start_pt`, `end_pt`,
  `first_no`, `absorbed_fp` → camelCase ✓

**Le tag interne d'`UndoRecord` reste inchangé** :
`#[serde(tag = "type", rename_all = "lowercase")]` — les variantes
`Delete` / `Route` continuent de sérialiser en `"delete"` / `"route"`.

---

## 1. Signatures Rust (`commands.rs`)

```rust
// src-tauri/src/gpx_audit/commands.rs

use tauri::AppHandle;
use super::types::{
    AuditDetectionResult, AuditParams, AuditPoint, AuditState, Finding,
    LatLon,
};

// ─── 1. Détection ─────────────────────────────────────────────────────

/// Lance la détection AR + RP sur la trace indiquée.
///
/// Charge le GPX, consolide, construit la géométrie métrique, exécute
/// `detect_ar` puis `detect_rp`, fusionne et renumérote les findings,
/// retourne le tout.
#[tauri::command]
pub async fn audit_run_detection(
    app: AppHandle,
    trace_id: String,
    params: AuditParams,
) -> Result<AuditDetectionResult, String>;

// ─── 2. Suppression de points ─────────────────────────────────────────

/// Supprime la plage `[ds..=de]` et met à jour le finding associé.
#[tauri::command]
pub fn audit_apply_delete(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
    ds: usize,
    de: usize,
    next_point_id: u32,
) -> Result<AuditState, String>;

// ─── 3. Routage ───────────────────────────────────────────────────────

/// Remplace l'intérieur `[start+1..end-1]` par les points ORS.
///
/// `coords` contient le tracé complet renvoyé par ORS, ancres incluses.
/// Les ancres sont retirées côté Rust pour éviter les doublons.
#[tauri::command]
pub fn audit_apply_route(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
    start: usize,
    end: usize,
    coords: Vec<LatLon>,
    profile: String,        // "driving-car" | "cycling-road"
    next_point_id: u32,
) -> Result<AuditState, String>;

// ─── 4. Faux positif ──────────────────────────────────────────────────

/// Marque un finding comme faux positif (trace inchangée).
#[tauri::command]
pub fn audit_mark_fp(
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<Vec<Finding>, String>;

/// Retire le marqueur faux positif (retour à pending).
#[tauri::command]
pub fn audit_unmark_fp(
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<Vec<Finding>, String>;

// ─── 5. Annulation ────────────────────────────────────────────────────

/// Annule la correction d'un finding (undo).
#[tauri::command]
pub fn audit_undo_correction(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<AuditState, String>;

// ─── 6. Validation ────────────────────────────────────────────────────

/// Valide l'audit : réécrit le GPX, pose `audit_status = "clean"`.
///
/// Point de non-retour. Refuse si un finding est encore `pending`.
#[tauri::command]
pub async fn audit_validate(
    app: AppHandle,
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
) -> Result<serde_json::Value, String>;
```

---

## 2. Catalogue détaillé

### 2.1 `audit_run_detection`

| Aspect | Valeur |
|---|---|
| **Entrée** | `trace_id: String`, `params: AuditParams` |
| **Sortie** | `AuditDetectionResult` |
| **Appelé par** | `useAuditStore.runAudit()` |
| **Durée typique** | 50-200 ms (trace 5 000 pts) |

**Algorithme** :

```
1. Charger le GPX via import_gpx::get_trace_gpx_path(mode_dir, trace_id)
2. Parser le GPX (crate gpx) → Vec<(lat, lon, ele)>
3. Consolider avec params.consol_m
   → Vec<AuditPoint> avec id séquentiels (1..N)
4. Construire geo (projector équirectangulaire) :
   - proj.fwd/inv sur le premier point
   - px, py : Vec<f64>
   - cum : Vec<f64> distance cumulée
   - total : f64
   - ids : Vec<u32>
5. detect_ar(points, px, py, ids, params) → findings_ar
6. resample_geo(px, py, cum, total, RP_STEP_DEFAULT) → R
7. detect_rp(points, px, py, ids, total, params) → findings_rp
8. Fusionner : findings = findings_ar + findings_rp
9. Trier par parts[0].s croissant
10. Renuméroter par famille :
    - Compteur par label nu ("Aller-retour", "Boucle giratoire", "Tour de rond-point")
    - Suffixe " : n" (n = 1, 2, 3, ...)
    - Assigner un id unique : "ar-1", "ar-2", "rp-1", ...
11. Retourner AuditDetectionResult { trace_id, points, total_distance_m,
    findings, params, duration_ms }
```

**Gestion d'erreurs** :
- Fichier GPX introuvable → `Err("Fichier GPX introuvable: {path}")`
- Parsing échoué → `Err("GPX illisible: {details}")`
- Moins de 5 points après consolidation → `Err("Trace trop courte après consolidation.")`

**Logging** : `println!("[audit] detection trace={} points={} findings={} durée={}ms", ...)`

---

### 2.2 `audit_apply_delete`

| Aspect | Valeur |
|---|---|
| **Entrée** | `trace_id`, `points`, `findings`, `finding_id`, `ds`, `de`, `next_point_id` |
| **Sortie** | `AuditState { points, findings }` |
| **Appelé par** | `useAuditStore.applyDelete()` |

**Algorithme** : délègue à `corrections::apply_delete` (Livrable 9).

```
1. Appeler corrections::apply_delete(points, findings, finding_id, ds, de, next_point_id)
2. Retourner le résultat
```

**Note** : `trace_id` n'est pas utilisé dans cette commande (la trace
n'est pas réécrite sur disque avant validation), mais il est passé pour
uniformité et pour d'éventuels logs.

**Gestion d'erreurs** : voir CORRECTIONS §5.2 (nesting guard, longueur
restante ≥ 2).

---

### 2.3 `audit_apply_route`

| Aspect | Valeur |
|---|---|
| **Entrée** | `trace_id`, `points`, `findings`, `finding_id`, `start`, `end`, `coords`, `profile`, `next_point_id` |
| **Sortie** | `AuditState` |
| **Appelé par** | `useAuditStore.applyRoute()` |

**Algorithme** : délègue à `corrections::apply_route`.

**Détail des `coords`** :

Le frontend a reçu d'ORS une Feature GeoJSON contenant N coordonnées
`[lon, lat, ele?]`. Il les a converties en `LatLon { lat, lon }` (sans
élévation pour l'instant) et les a transmises telles quelles.

Côté Rust, `apply_route` doit :
1. Retirer les **première** et **dernière** coordonnées (ce sont les
   ancres, déjà présentes dans `points[start]` et `points[end]`)
2. Créer un `AuditPoint` par coordonnée restante avec `id = next_point_id + k`
3. Insérer entre `points[start]` et `points[end]`

**Garde** : `coords.len() >= 2` obligatoire. Sinon
`Err("Tracé ORS invalide (moins de 2 points).")`.

**Profil** : `"driving-car"` ou `"cycling-road"`. Tout autre →
`Err("Profil ORS inconnu : {profile}")`.

---

### 2.4 `audit_mark_fp`

| Aspect | Valeur |
|---|---|
| **Entrée** | `findings`, `finding_id` |
| **Sortie** | `Vec<Finding>` |
| **Appelé par** | `useAuditStore.markFp()` |

**Algorithme** : délègue à `corrections::mark_fp`.

```
1. Trouver le finding par id
2. Si status != pending → Err("Finding déjà traité.")
3. Poser status = Fp, correction = None
4. Retourner la liste complète
```

**Note** : la trace est **inchangée**.

---

### 2.5 `audit_unmark_fp`

Symétrique à `audit_mark_fp`. Refuse si `status != Fp`.

---

### 2.6 `audit_undo_correction`

| Aspect | Valeur |
|---|---|
| **Entrée** | `trace_id`, `points`, `findings`, `finding_id` |
| **Sortie** | `AuditState` |
| **Appelé par** | `useAuditStore.undoCorrection()` |

**Algorithme** : délègue à `corrections::undo_correction` (Livrable 9).

**Garde-fous** (CORRECTIONS §8) :
- Type `route` : 3 garde-fous successifs (usedByOther, pos, contiguïté)
- Type `delete` : réinsertion par ancre (droite prioritaire)

---

### 2.7 `audit_validate`

| Aspect | Valeur |
|---|---|
| **Entrée** | `trace_id`, `points`, `findings` |
| **Sortie** | `serde_json::Value` (TraceMetadata mise à jour) |
| **Appelé par** | `useAuditStore.validateAndRewrite()` |
| **Point de non-retour** | Oui |

**Algorithme** :

```
1. Vérifier tous les findings : aucun ne doit être Pending
   → Sinon Err("Toutes les anomalies doivent être traitées
                avant de valider l'audit.")
2. Vérifier points.len() >= 2
   → Sinon Err("Trace dégénérée.")
3. Calculer les chemins :
   - gpx_path = import_gpx::get_trace_gpx_path(mode_dir, trace_id, filename)
   - orig_path = gpx_path.with_extension("gpx.orig")
   - geojson_path = import_gpx::get_geojson_path(mode_dir, trace_id)
4. Backup .orig si absent (jamais écrasé) :
   if !orig_path.exists() { fs::copy(&gpx_path, &orig_path)?; }
5. Réécrire le GPX :
   - Récupérer les métadonnées source (source_gpx_attrs, source_meta_xml,
     source_trk_name) → stockées dans TraceMetadata ? Non : ces infos
     viennent du fichier GPX actuel. Les relire au moment de la
     validation.
   - Appeler export::rewrite_gpx(gpx_path, points, source_meta,
     source_attrs, source_trk_name, app_name)
6. Régénérer trace.geojson (import_gpx::generate_geojson)
7. Recalculer stats (import_gpx::compute_stats)
8. Recalculer hash SHA256
9. Mettre à jour traces.json :
   - audit_status = "clean"
   - stats, hash mis à jour
   - écriture atomique (tmp + rename)
10. Retourner la TraceMetadata mise à jour (serde_json::Value)
```

**Nom de l'application** : lire `Audit.Application.nom` via
`settings::get_setting_value`.

**Logging** : `println!("[audit] validation trace={} points={} findings={}",
trace_id, points.len(), findings.len())`.

---

## 3. Enregistrement dans `lib.rs`

```rust
mod gpx_audit;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_state = settings::init_settings_state(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.manage(settings_state);
            display::setup_display(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ─── Commandes existantes ───
            exit_app,
            get_displays,
            open_second_window,
            close_second_window,
            gestionMode::get_execution_env,
            gestionMode::get_modes,
            gestionMode::create_mode,
            gestionMode::update_mode,
            gestionMode::delete_mode,
            gestionMode::select_mode,
            settings::get_settings,
            settings::get_settings_meta,
            settings::update_setting,
            settings::reset_setting,
            settings::get_setting_value,
            import_gpx::import_gpx_file,
            import_gpx::get_traces,
            import_gpx::delete_trace,
            import_gpx::update_trace,
            import_gpx::get_trace_geometry,
            import_gpx::get_trace_points,

            // ─── Audit GPX (Livrable 4) ───
            gpx_audit::commands::audit_run_detection,
            gpx_audit::commands::audit_apply_delete,
            gpx_audit::commands::audit_apply_route,
            gpx_audit::commands::audit_mark_fp,
            gpx_audit::commands::audit_unmark_fp,
            gpx_audit::commands::audit_undo_correction,
            gpx_audit::commands::audit_validate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Note Phase 1 uniquement** : tant que seules les commandes du Livrable 4
sont enregistrées, `mod cleaning;` doit **rester** dans `lib.rs` pour ne
pas casser les commandes existantes (`detect_trace_anomalies`,
`get_cleaning_state`, etc.). La suppression de `cleaning` se fera dans
la phase de migration finale.

---

## 4. Fichiers internes (`gpx_audit/mod.rs`)

```rust
// src-tauri/src/gpx_audit/mod.rs

pub mod types;
pub mod ar;
pub mod rp;          // Phase 2
pub mod anchor;      // Phase 2
pub mod geometry;
pub mod consolidation;
pub mod corrections; // Phase 3
pub mod migration;   // Phase 3
pub mod export;      // Phase 3
pub mod commands;

#[cfg(test)]
mod tests;
```

---

## 5. Conventions et pièges

### 5.1 Convention d'appel côté front

```typescript
// Le front passe du camelCase
await invoke('audit_apply_delete', {
  traceId: currentTraceId.value,
  points: working.value,
  findings: findings.value,
  findingId,
  ds,
  de,
  nextPointId: nextPointId.value,
})
```

**Ce que Tauri fait** :
- **Arguments** : convertit `traceId` → `trace_id` automatiquement
- **Retours** : ne convertit **rien** — c'est `#[serde(rename_all = "camelCase")]`
  qui produit du camelCase côté Rust (cf. Avenant Livrable 1)

### 5.2 Pas de `Result<T, String>` pour les erreurs fatales

Toutes les commandes retournent `Result<T, String>` (convention du
projet, voir `import_gpx.rs`). Le front reçoit le message d'erreur et
peut l'afficher via un toast.

### 5.3 Pas de `unwrap()` dans le code de commande

Les commandes appellent les fonctions internes qui retournent
`Result<_, String>`. Aucun `unwrap()` sur une entrée utilisateur.

### 5.4 Logging

Convention existante : `println!` pour les logs info, `eprintln!` pour
les erreurs. Aucune dépendance à `log` ou `tracing`.

**Format** : `[audit] <commande> <détails>`

Exemples :
```
[audit] detection trace=abc123 points=1250 findings=3 durée=87ms
[audit] apply_delete trace=abc123 finding=ar-1 plage=[10..14] -> 1245 points
[audit] validate trace=abc123 points=1245 findings=3 (1 FP)
```

### 5.5 Absence de commande `audit_close`

Le state est côté frontend (Pinia). Aucune ressource Rust à libérer. Le
`reset()` du store suffit (décision C.2).

### 5.6 Performance

Les commandes `audit_apply_delete`, `audit_apply_route`,
`audit_mark_fp`, `audit_unmark_fp`, `audit_undo_correction` sont
**synchrones** (pas `async`) : elles ne font aucun I/O, seulement du
calcul en mémoire. Durée typique : < 5 ms.

Seules `audit_run_detection` et `audit_validate` sont **async** : elles
lisent/écrivent sur disque.

---

## 6. Tests d'intégration à écrire

Fichier : `src-tauri/src/gpx_audit/tests/commands_test.rs`

Ces tests ne peuvent pas utiliser Tauri directement (besoin d'un
`AppHandle`). On teste donc les **fonctions internes** appelées par les
commandes :

```rust
#[test]
fn test_run_detection_returns_valid_result() {
    // Mock : charger un GPX depuis test_files/
    // Appeler la fonction interne run_detection_impl()
    // Vérifier : findings.len() >= 0, points.len() >= 2, duration_ms > 0
}

#[test]
fn test_run_detection_rejects_short_trace() {
    // GPX avec 3 points -> Err
}

#[test]
fn test_apply_delete_rejects_nesting() {
    // 2 findings imbriqués, tentative de suppression englobante
    // -> Err("Correction refusée...")
}

#[test]
fn test_apply_route_validates_profile() {
    // profile = "unknown" -> Err
}

#[test]
fn test_mark_fp_rejects_corrected() {
    // Finding déjà corrected -> Err
}

#[test]
fn test_undo_restores_state() {
    // apply_delete puis undo -> état identique
}

#[test]
fn test_validate_rejects_pending() {
    // findings avec 1 pending -> Err
}

#[test]
fn test_validate_updates_audit_status() {
    // Après validate, traces.json contient audit_status = "clean"
}
```

---

## 7. Critères de validation

- [ ] `cargo check` sans warning ni erreur
- [ ] `cargo test --lib gpx_audit::commands` : 8 tests verts
- [ ] Les 7 commandes sont enregistrées dans `lib.rs`
- [ ] Aucun `unwrap()` dans `commands.rs`
- [ ] Le front peut appeler chaque commande avec succès (test manuel
      via l'IHM après Phase 4)
- [ ] Les logs `[audit]` apparaissent en console lors des appels

---

## 8. Points de vigilance

| # | Piège | Mitigation |
|---|---|---|
| 1 | `#[serde(rename_all = "camelCase")]` manquant → noms snake_case en JSON | Vérifier chaque struct de `types.rs` |
| 2 | `UndoRecord` : `#[serde(tag = "type", rename_all = "lowercase")]` reste, ne pas ajouter `camelCase` (le tag devient `"Type"`) | Ne pas toucher à l'attribut existant |
| 3 | `next_point_id` : le front le calcule après chaque action ; Rust doit **l'utiliser** tel quel pour allouer les nouveaux ids (jamais recalculer côté Rust) | Faire confiance au front, valider seulement `next_point_id > 0` |
| 4 | `coords` ORS : le front envoie le tracé complet ; Rust **retire les 2 extrémités** avant insertion | Test avec 2 coords exactement → 0 mids insérés (intérieur vide) |
| 5 | `audit_validate` : si le GPX existe déjà en `.orig`, ne **pas** l'écraser | `if !orig_path.exists()` avant `fs::copy` |
| 6 | `audit_validate` : échec de réécriture → `audit_status` reste `needs_review` | Écrire le GPX **avant** de mettre à jour `traces.json` |
| 7 | `audit_run_detection` : point d'entrée unique, mais le `trace_id` doit être validé (existe dans `traces.json`) | Vérifier avant de charger le GPX |
| 8 | Format `LatLon` : le front envoie `{ lat, lon }` (camelCase) ; Rust reçoit `LatLon { lat, lon }` (déjà camelCase, pas de conversion) | Vérifier que la struct a bien `#[serde(rename_all = "camelCase")]` |