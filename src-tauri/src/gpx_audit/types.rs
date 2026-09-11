// Contrat de données du module Audit GPX.
// Miroir exact des interfaces TypeScript définies dans src/stores/audit.ts.
//
// Avenant au Livrable 1 : toutes les **structs** portent
// `#[serde(rename_all = "camelCase")]`. Tauri 2.x convertit les **arguments**
// d'appel (`traceId` → `trace_id`) mais **pas** les champs imbriqués ni les
// retours de commande : sans cet attribut, les paramètres envoyés par le store
// (`{ consolM, tolDeg, … }`) échoueraient à se désérialiser et le front
// recevrait du snake_case. Les **enums** conservent leur propre `rename_all`.

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
#[serde(rename_all = "camelCase")]
pub struct FindingPair {
    pub aid: u32, // id stable du point amont
    pub bid: u32, // id stable du point aval
    pub a: usize, // index de référence amont
    pub b: usize, // index de référence aval
    pub d: f64,   // distance en mètres (toujours définie)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingPart {
    pub s: usize,
    pub e: usize,
    pub role: PartRole,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingContext {
    pub up: Option<usize>,
    pub dn: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingContextIds {
    pub up: Option<u32>,
    pub dn: Option<u32>,
}

// ─── Finding complet (contrat ANALYSE §4.2) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct UndoDelete {
    pub orig_pts: Vec<AuditPoint>,
    pub anchor_left_id: Option<u32>,
    pub anchor_right_id: Option<u32>,
    pub first_no: usize,
    pub absorbed_fp: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

// ─── Point et résultat de détection ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPoint {
    pub id: u32,
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditDetectionResult {
    pub trace_id: String,
    pub points: Vec<AuditPoint>, // trace consolidée (working initial)
    pub total_distance_m: f64,
    pub findings: Vec<Finding>,
    pub params: AuditParams,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct AuditState {
    pub points: Vec<AuditPoint>,
    pub findings: Vec<Finding>,
}
