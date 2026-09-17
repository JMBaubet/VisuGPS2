// Contrat de données du module Multiride — détection des portions de trace
// parcourues plusieurs fois (passages multiples).
//
// Miroir exact des interfaces TypeScript définies dans src/stores/multiride.ts.
//
// Portage du module « Multi-Sens » de la spécification : les structures de la
// référence (documentation technique §4) sont reprises telles quelles, à la
// nomenclature près. Les identifiants du projet sont en camelCase à la
// sérialisation ; le **fichier de sortie**, lui, conserve les clés de la
// spécification (cf. `file.rs`).
//
// Toutes les structs portent `#[serde(rename_all = "camelCase")]` : Tauri 2.x
// convertit les arguments d'appel (`traceId` → `trace_id`) mais ni les champs
// imbriqués ni les retours de commande.

use serde::{Deserialize, Serialize};

// ─── Statut du registre des traces ────────────────────────────────────

/// Aucun passage multiple détecté : l'édition caméra est accessible.
pub const STATUS_NONE: &str = "none";
/// Au moins un passage détecté, non validé : l'édition caméra est fermée.
pub const STATUS_PENDING: &str = "pending";
/// Passages validés par l'utilisateur : l'édition caméra est accessible.
pub const STATUS_VALIDATED: &str = "validated";

// ─── Énumérations ─────────────────────────────────────────────────────

/// Sens d'un passage, relatif au passage de référence de son segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MultirideSens {
    /// Premier passage du segment — c'est lui qui donne le sens de référence.
    Reference,
    /// Passage parcouru dans le même sens que la référence.
    Aller,
    /// Passage parcouru en sens inverse.
    Retour,
}

// ─── Géométrie ────────────────────────────────────────────────────────

/// Coordonnées d'une borne de passage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultirideLatLon {
    pub lat: f64,
    pub lon: f64,
}

/// Point brut du GPX — entrée de la détection.
///
/// Volontairement distinct de `gpx_audit::types::AuditPoint` (qui porte un
/// identifiant stable et une altitude inutilisés ici) : les deux modules
/// partagent le même fichier source, pas leur modèle de données.
#[derive(Debug, Clone, Copy)]
pub struct MultiridePoint {
    pub lat: f64,
    pub lon: f64,
}

// ─── Paramètres ───────────────────────────────────────────────────────

/// Paramètres de détection (namespace `Multiride.Detection`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultirideParams {
    /// Tolérance de superposition (m) : distance maximale entre deux points
    /// pour qu'ils soient considérés comme « le même lieu géographique ».
    pub tolerance_m: f64,
    /// Longueur minimale d'un segment pour être signalé (m).
    pub longueur_min_m: f64,
    /// Pas de rééchantillonnage (m) — granularité de la détection.
    pub pas_echantillonnage_m: f64,
    /// Fusion des références proches (m). `0` désactive les fusions.
    pub fusion_references_m: f64,
}

// ─── Passage ──────────────────────────────────────────────────────────

/// Un emprunt d'un segment : portion de trace continue, qualifiée par un sens.
///
/// C'est l'unité de restitution **et** l'unité du fichier de sortie : chaque
/// passage produit une Feature dont la géométrie est bornée à ses deux
/// extrémités (spécification §F-17).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiridePassage {
    /// Numéro du segment (1-based) auquel le passage appartient.
    pub segment: usize,
    /// Numéro du passage dans son segment (1-based).
    pub passage: usize,
    pub sens: MultirideSens,
    /// Segment marqué faux positif par l'utilisateur (exclu de l'export).
    pub faux_positif: bool,
    /// Index du point d'entrée dans les `<trkpt>` du GPX d'origine (1-based).
    pub point_entree: usize,
    /// Index du point de sortie (1-based).
    pub point_sortie: usize,
    /// Distance cumulée depuis le départ de la trace (km) — et non distance
    /// euclidienne entre les deux bornes.
    pub km_entree: f64,
    pub km_sortie: f64,
    /// `km_sortie − km_entree` (km).
    pub longueur_km: f64,
    /// Le segment du passage a subi une fusion manuelle.
    pub fusionne: bool,
    /// Segment **approuvé** par l'utilisateur : un vrai passage multiple, rien
    /// à changer.
    ///
    /// Troisième état d'un segment, exclusif des deux autres — écarté,
    /// fusionné —, et comme eux un fait de segment porté par chaque emprunt.
    /// C'est le geste le plus fréquent : sans lui, approuver un segment ne se
    /// dirait pas, et l'avancement ne pourrait jamais être complet.
    #[serde(default)]
    pub valide: bool,
    /// Emprunts des deux segments **tels qu'ils étaient avant la fusion** —
    /// de quoi l'annuler.
    ///
    /// Présent sur les seuls emprunts d'un segment fusionné, et `None`
    /// partout ailleurs (détection neuve, segment écarté). Les emprunts
    /// enregistrés conservent leurs propres champs, **y compris un
    /// `avant_fusion` antérieur** : la fusion de trois segments consécutifs
    /// s'annule donc en cascade, la plus récente d'abord.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avant_fusion: Option<Vec<MultiridePassage>>,
    /// Borne d'entrée — première coordonnée de la géométrie du fichier.
    pub entree: MultirideLatLon,
    /// Borne de sortie — seconde coordonnée de la géométrie du fichier.
    pub sortie: MultirideLatLon,
}

// ─── État complet ─────────────────────────────────────────────────────

/// État complet d'une détection de passages multiples.
///
/// Cette structure est à la fois l'**état de travail** de la vue (ajustements
/// compris) et la représentation interne du **fichier de description** écrit
/// dans le dossier de la trace (`traces/{trace_id}/multiride.json`). Le
/// fichier en est la projection à la nomenclature de la spécification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultirideArchive {
    /// Version du format (`file::FILE_VERSION`).
    pub version: u32,
    /// Trace à laquelle l'état appartient.
    pub trace_id: String,
    /// Nom du fichier GPX d'origine.
    pub source: String,
    /// Horodatage ISO 8601 UTC de la dernière écriture.
    pub updated_at: String,
    /// L'utilisateur a validé la détection : la barrière de l'édition caméra
    /// est levée. Les ajustements restent possibles après validation (ils
    /// n'ont pas d'incidence sur l'édition caméra) et le statut ne rebascule
    /// pas.
    pub valide: bool,
    /// Paramètres ayant produit la détection — ils voyagent avec l'état, de
    /// sorte qu'une relance soit possible sans relire le drawer.
    pub params: MultirideParams,
    /// Nombre de points de la trace d'origine.
    pub trace_point_count: usize,
    /// Longueur totale de la trace, le long de la trace (km).
    pub trace_length_km: f64,
    /// Le pas d'échantillonnage a été relevé automatiquement pour contenir le
    /// nombre de points échantillonnés (garde-fou de la spécification §6.3).
    pub pas_plafonne: bool,
    /// Passages détectés, tous segments confondus, triés par segment puis par
    /// numéro de passage.
    pub passages: Vec<MultiridePassage>,
}

impl MultirideArchive {
    /// Statut à poser dans le registre des traces pour cet état.
    ///
    /// L'absence de passage prime sur la validation : `none` est un **fait sur
    /// la trace** (aucune portion répétée), là où `validated` est une décision
    /// de l'utilisateur sur une détection non vide. Une détection vide, même
    /// marquée validée, n'a donc pas d'autre statut que `none` — et la barrière
    /// reste ouverte dans les deux cas, il n'y a rien à protéger.
    pub fn status(&self) -> &'static str {
        if self.passages.is_empty() {
            STATUS_NONE
        } else if self.valide {
            STATUS_VALIDATED
        } else {
            STATUS_PENDING
        }
    }
}

/// Résultat de la commande `multiride_detect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultirideDetectionResult {
    /// État écrit sur disque (fichier de description inclus).
    pub archive: MultirideArchive,
    /// Statut posé dans le registre des traces (`archive.status()`).
    pub status: String,
    /// Durée de la détection (ms) — restituée dans la synthèse de la vue.
    pub duration_ms: u64,
}
