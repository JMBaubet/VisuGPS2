//! Archive d'audit persistée dans le dossier de la trace (`audit.json`).
//!
//! **Avenant à la décision 6** : l'état de travail n'est plus volatil. La trace
//! de travail, les findings et leurs traitements sont écrits sur disque **au fil
//! des actions** (après la détection, puis après chaque traitement), ce qui
//! donne deux usages :
//! - **reprise** d'une session interrompue (trace `needs_review`) ;
//! - **consultation** d'un audit validé (trace `clean`), en lecture seule.
//!
//! Deux garanties, reprises de `export.rs` :
//! - l'écriture est **atomique** (`pipeline::write_atomic`, `.tmp` + `rename`) :
//!   un échec laisse l'archive précédente intacte ;
//! - la lecture est **tolérante** : archive absente, illisible, d'une version
//!   inconnue ou rattachée à une autre trace → `None`, sans jamais échouer. Le
//!   module retombe alors sur la détection, qui reste la source de vérité.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::export::iso_now;
use super::pipeline;
use super::types::{AuditParams, AuditPoint, Finding};

/// Version du format d'archive. Toute archive d'une autre version est ignorée
/// par `load_archive` (repli sur la détection).
pub const ARCHIVE_VERSION: u32 = 1;

/// Nom du fichier d'archive, dans le dossier de la trace.
const ARCHIVE_FILE_NAME: &str = "audit.json";

/// État complet d'un audit : trace de travail, findings et traitements.
///
/// `points` porte l'**espace d'index** des findings (invariant C4) : c'est lui
/// qui permet de restituer la carte, la liste et les zones d'anomalie telles
/// qu'elles étaient, sans réexécuter la détection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditArchive {
    /// Version du format (`ARCHIVE_VERSION`).
    pub version: u32,
    /// Trace à laquelle l'archive appartient.
    pub trace_id: String,
    /// Horodatage ISO 8601 UTC de la dernière écriture.
    pub updated_at: String,
    /// `true` quand l'audit a été appliqué (GPX réécrit) : l'archive est alors
    /// une pièce de consultation, et non plus une session de travail.
    pub validated: bool,
    /// Paramètres du détecteur ayant produit les findings (rejoués à
    /// l'identique par les aperçus et les éléments de rendu de la carte).
    pub params: AuditParams,
    /// Trace de travail (points consolidés et corrigés).
    pub points: Vec<AuditPoint>,
    /// Findings, avec leur statut, leur correction et leur enregistrement
    /// d'annulation — ce dernier conserve les points supprimés et le tracé ORS
    /// remplaçant, seule source du « avant / après » en consultation.
    pub findings: Vec<Finding>,
}

/// Chemin de l'archive d'une trace : `{mode}/traces/{trace_id}/audit.json`.
pub fn archive_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("traces").join(trace_id).join(ARCHIVE_FILE_NAME)
}

/// Construit l'archive d'un état de travail : horodatage et version courants,
/// le statut de validation étant fourni par l'appelant.
pub fn build_archive(
    trace_id: &str,
    validated: bool,
    params: AuditParams,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
) -> AuditArchive {
    AuditArchive {
        version: ARCHIVE_VERSION,
        trace_id: trace_id.to_string(),
        updated_at: iso_now(),
        validated,
        params,
        points,
        findings,
    }
}

/// Écrit l'archive (écriture atomique). Le dossier de la trace est créé au
/// besoin.
///
/// Un échec est **remonté** à l'appelant : l'audit reste utilisable en mémoire,
/// mais la trace du travail n'est plus garantie — le store le notifie à
/// l'utilisateur plutôt que de l'ignorer.
pub fn save_archive(path: &Path, archive: &AuditArchive) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Création du dossier d'archive ({}): {}",
                parent.display(),
                e
            )
        })?;
    }
    let json = serde_json::to_string_pretty(archive)
        .map_err(|e| format!("Sérialisation de l'archive d'audit : {}", e))?;
    pipeline::write_atomic(path, json.as_bytes())
}

/// Lit l'archive d'une trace.
///
/// Retourne `None` si l'archive est absente, illisible, d'une version inconnue
/// ou rattachée à une autre trace : l'appelant retombe sur la détection.
pub fn load_archive(path: &Path, trace_id: &str) -> Option<AuditArchive> {
    let content = std::fs::read_to_string(path).ok()?;
    let archive: AuditArchive = serde_json::from_str(&content).ok()?;
    if archive.version != ARCHIVE_VERSION || archive.trace_id != trace_id {
        return None;
    }
    Some(archive)
}
