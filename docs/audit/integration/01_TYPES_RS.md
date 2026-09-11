# Livrable 1 — Contrat Rust `types.rs`

**Fichier cible** : `src-tauri/src/gpx_audit/types.rs`

**Rôle** : définir toutes les structures de données du module Audit GPX.
Miroir exact des interfaces TypeScript de `src/stores/audit.ts` (Livrable 2).

**Conventions de sérialisation** :
- `snake_case` par défaut (aligné avec `import_gpx.rs`)
- Enums en `lowercase` (ex. `FindingKind::Ar` → `"ar"`)
- `CorrectionType` en `kebab-case` (ex. `RouteCar` → `"route-car"`)
- `Option<T>` sérialisé en `null` (pas de `skip_serializing_if`)
- `UndoRecord` tagué par `type` (tag interne)

---

## Contenu du fichier

```rust
// src-tauri/src/gpx_audit/types.rs
//
// Contrat de données du module Audit GPX.
// Miroir exact des interfaces TypeScript définies dans src/stores/audit.ts.

use serde::{Deserialize, Serialize};

// ─── Énumérations ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingKind {
    Ar, // aller-retour
    Rp, // boucle giratoire
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingStatus {
    Pending,
    Corrected,
    Fp, // faux positif
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorrectionType {
    Delete,
    RouteCar,
    RouteBike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartRole {
    Warn,
    Info,
}

// ─── Contenu d'un finding ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingPair {
    pub aid: u32, // id stable du point amont
    pub bid: u32, // id stable du point aval
    pub a: usize, // index de référence amont
    pub b: usize, // index de référence aval
    pub d: f64,   // distance en mètres (toujours définie)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingPart {
    pub s: usize,
    pub e: usize,
    pub role: PartRole,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingContext {
    pub up: Option<usize>,
    pub dn: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingContextIds {
    pub up: Option<u32>,
    pub dn: Option<u32>,
}

// ─── Finding complet (contrat ANALYSE §4.2) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,     // "ar-1", "rp-2", ...
    pub kind: FindingKind,
    pub label: String,  // "Aller-retour : 1"
    pub summary: String,
    pub peak: usize,
    pub peak_id: u32,
    pub pairs: Vec<FindingPair>,
    pub pair_idx: Vec<usize>, // [a1, b1, a2, b2, ...]

    // AR uniquement
    pub ecart: Option<f64>,
    pub d1: Option<f64>,
    pub d2: Option<f64>,

    // RP uniquement
    pub total_angle: Option<i32>,
    pub turn_text: Option<String>,
    pub core_ids: Vec<u32>,

    pub zone_ids: Vec<u32>,
    pub ctx_ids: FindingContextIds,
    pub ctx: FindingContext,
    pub parts: Vec<FindingPart>,

    // Statut et correction
    pub status: FindingStatus,
    pub correction: Option<CorrectionType>,

    // Undo (volatile, reconstruit à chaque édition)
    pub undo: Option<UndoRecord>,
}

// ─── Structures d'undo (contrat CORRECTIONS §2.3) ─────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UndoRecord {
    Delete(UndoDelete),
    Route(UndoRoute),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoDelete {
    pub orig_pts: Vec<AuditPoint>,
    pub anchor_left_id: Option<u32>,
    pub anchor_right_id: Option<u32>,
    pub first_no: usize,
    pub absorbed_fp: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRoute {
    pub orig_pts: Vec<AuditPoint>,
    pub inserted_ids: Vec<u32>,
    pub route_pts: Vec<LatLon>,
    pub start_pt: LatLon,
    pub end_pt: LatLon,
    pub first_no: usize,
    pub absorbed_fp: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

// ─── Point et résultat de détection ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPoint {
    pub id: u32,
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDetectionResult {
    pub trace_id: String,
    pub points: Vec<AuditPoint>, // trace consolidée (working initial)
    pub total_distance_m: f64,
    pub findings: Vec<Finding>,
    pub params: AuditParams,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditParams {
    pub consol_m: f64,
    pub tol_deg: f64,
    pub pair_m: f64,
    pub maxpairs: u32,
    pub seg_m: f64,
    pub close_m: f64,
    pub angle_deg: u32,
}

// ─── État retourné par les commandes de correction ────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditState {
    pub points: Vec<AuditPoint>,
    pub findings: Vec<Finding>,
}
```

---

## Notes pour Zcode

1. **Imports** : ce fichier n'importe que `serde`. Aucune dépendance à `super::` ni à des modules internes.
2. **Ordre des déclarations** : respecter l'ordre ci-dessus (enums → structs simples → structs composées → structs de résultat). Cela facilite la lecture.
3. **`usize` vs `u32`** :
   - `usize` pour les **index de référence** (taille du tableau `Vec<AuditPoint>`)
   - `u32` pour les **ids stables** des points (compteur `next_point_id`, jamais recyclé)
4. **`Option<T>`** : toujours sérialisé en `null`. Ne pas ajouter `#[serde(skip_serializing_if = "Option::is_none")]` — cela casserait le contrat avec TypeScript qui attend `null`.
5. **`UndoRecord`** : le tag `"type"` est interne (pas de `#[serde(tag = "type", content = "data")]`). Sérialisation attendue :
   ```json
   { "type": "delete", "orig_pts": [...], ... }
   { "type": "route",  "orig_pts": [...], ... }
   ```
6. **Pas de `#[derive(Default)]`** : aucun type n'a de valeur par défaut pertinente. La construction est toujours explicite.
7. **`Clone` est requis** partout (utilisé par les instantanés d'undo, cf. CORRECTIONS §2.3).

---

## Tests de validation (à ajouter dans `tests/mod.rs`)

Ces tests vérifient uniquement la **sérialisation** (pas la logique) :

```rust
#[cfg(test)]
mod serialization_tests {
    use super::*;

    #[test]
    fn test_finding_kind_serde() {
        assert_eq!(serde_json::to_string(&FindingKind::Ar).unwrap(), "\"ar\"");
        assert_eq!(serde_json::to_string(&FindingKind::Rp).unwrap(), "\"rp\"");
    }

    #[test]
    fn test_correction_type_kebab() {
        assert_eq!(
            serde_json::to_string(&CorrectionType::RouteCar).unwrap(),
            "\"route-car\""
        );
        assert_eq!(
            serde_json::to_string(&CorrectionType::RouteBike).unwrap(),
            "\"route-bike\""
        );
    }

    #[test]
    fn test_undo_record_tagged() {
        let undo = UndoRecord::Delete(UndoDelete {
            orig_pts: vec![],
            anchor_left_id: Some(1),
            anchor_right_id: None,
            first_no: 5,
            absorbed_fp: vec![],
        });
        let json = serde_json::to_string(&undo).unwrap();
        assert!(json.contains("\"type\":\"delete\""));
        assert!(json.contains("\"anchor_left_id\":1"));
        assert!(json.contains("\"anchor_right_id\":null"));
    }

    #[test]
    fn test_option_serialized_as_null() {
        let ctx = FindingContextIds { up: Some(3), dn: None };
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(json, r#"{"up":3,"dn":null}"#);
    }
}
```

---

## Critères de validation

- `cargo check` sans warning
- `cargo test --lib gpx_audit::types` : 4 tests verts
- Inspection visuelle : aucun `#[serde(skip_serializing_if)]`, aucun `#[derive(Default)]`