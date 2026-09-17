//! Fichier de description des passages multiples, dans le dossier de la trace
//! (`traces/{trace_id}/multiride.json`).
//!
//! Le format est celui de la spécification (§F-17 / annexe 13.4) : une
//! `FeatureCollection` dont le bloc `properties` porte le contexte de la trace
//! et les paramètres actifs, et dont chaque `Feature` décrit **un passage** —
//! géométrie `LineString` à exactement deux coordonnées (les bornes), et
//! métadonnées d'indices et de distances cumulées permettant de rejoindre la
//! trace d'origine (annexe 13.6).
//!
//! Quatre ajouts au format de la spécification, pour en faire un état
//! **restaurable** en plus d'un contrat de sortie :
//! - `version` et `trace_id` dans `properties` — versionnage du format (une
//!   version inconnue est refusée) et rattachement à la trace ;
//! - `valide` dans `properties` — marque de validation par l'utilisateur ;
//! - `faux_positif` sur chaque `Feature` — les passages d'un segment marqué
//!   faux positif **restent** dans le fichier (c'est l'export qui les exclut,
//!   §CA-13) : sans eux, une réouverture de la vue ne pourrait plus les
//!   distinguer d'un segment ordinaire ;
//! - `avant_fusion` sur les Features d'un segment **fusionné** — les emprunts
//!   des deux segments tels qu'ils étaient avant la fusion, qui est
//!   destructive et ne se recalcule pas. Comme le champ est additif et
//!   optionnel, un fichier écrit avant son introduction se lit toujours : le
//!   segment fusionné n'est simplement plus annulable, et la version du format
//!   reste donc `1`.
//!
//! Deux garanties, reprises de `gpx_audit::archive` :
//! - l'écriture est **atomique** (`pipeline::write_atomic`, `.tmp` + `rename`) :
//!   un échec laisse le fichier précédent intact ;
//! - la lecture est **tolérante** : fichier absent, illisible, d'une version
//!   inconnue ou rattaché à une autre trace → `None`, sans jamais échouer.
//!   L'appelant relance alors la détection, qui reste la source de vérité.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gpx_audit::export::iso_now;
use crate::gpx_audit::pipeline::write_atomic;

use super::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultirideSens,
};

/// Version du format du fichier. Un fichier d'une autre version est ignoré par
/// `load_file` (repli sur une nouvelle détection).
pub const FILE_VERSION: u32 = 1;

/// Nom du fichier, dans le dossier de la trace.
const FILE_NAME: &str = "multiride.json";

/// Rappel du format compact, à l'attention des consommateurs du fichier.
const NOTE: &str = "Chaque Feature représente un passage. La géométrie LineString ne contient que les 2 points bornes (entrée, sortie). Pour reconstituer la portion de trace, joindre point_entree / point_sortie avec la trace d'origine.";

/// Chemin du fichier d'une trace : `{mode}/traces/{trace_id}/multiride.json`.
pub fn file_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("traces").join(trace_id).join(FILE_NAME)
}

// ─── Format du fichier (nomenclature de la spécification) ─────────────

/// Racine du fichier. Les clés suivent la spécification : ce format est un
/// **contrat de sortie**, consommé en aval par la vue Visualisation, et non la
/// représentation interne du module (elle, en camelCase).
#[derive(Debug, Serialize, Deserialize)]
struct MultirideFile {
    #[serde(rename = "type")]
    kind: String,
    properties: FileProperties,
    features: Vec<FileFeature>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileProperties {
    version: u32,
    trace_id: String,
    valide: bool,
    /// Nom du fichier GPX d'origine.
    source: String,
    /// Date d'écriture, ISO 8601.
    date: String,
    trace: FileTrace,
    parametres: FileParams,
    ajustements: FileAdjustments,
    note: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileTrace {
    point_count: usize,
    length_km: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileParams {
    tolerance_m: f64,
    longueur_min_m: f64,
    pas_echantillonnage_m: f64,
    fusion_references_m: f64,
    pas_plafonne: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileAdjustments {
    /// Nombre de segments ayant subi une fusion manuelle.
    fusions_manuelles: usize,
    /// Nombre de segments marqués faux positif (exclus de l'export).
    faux_positifs_exclus: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileFeature {
    #[serde(rename = "type")]
    kind: String,
    properties: FileFeatureProperties,
    geometry: FileGeometry,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileFeatureProperties {
    segment: usize,
    passage: usize,
    sens: MultirideSens,
    faux_positif: bool,
    point_entree: usize,
    point_sortie: usize,
    km_entree: f64,
    km_sortie: f64,
    longueur_km: f64,
    fusionne: bool,
    /// Emprunts d'avant la fusion qui a réuni ce segment au précédent.
    ///
    /// Absent hors d'un segment fusionné — et donc absent des fichiers écrits
    /// avant l'introduction du champ, qui se relisent sans erreur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avant_fusion: Option<Vec<FileFeature>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileGeometry {
    #[serde(rename = "type")]
    kind: String,
    /// Deux points `[lon, lat]` : l'entrée du passage, puis sa sortie.
    coordinates: [[f64; 2]; 2],
}

// ─── Construction et conversion ───────────────────────────────────────

/// Construit l'état d'une détection : version et horodatage courants, état non
/// validé, aucun ajustement.
pub fn build_archive(
    trace_id: &str,
    source: &str,
    params: MultirideParams,
    trace_point_count: usize,
    trace_length_km: f64,
    pas_plafonne: bool,
    passages: Vec<MultiridePassage>,
) -> MultirideArchive {
    MultirideArchive {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        source: source.to_string(),
        updated_at: iso_now(),
        valide: false,
        params,
        trace_point_count,
        trace_length_km,
        pas_plafonne,
        passages,
    }
}

/// Nombre de segments distincts dont au moins un passage satisfait `flag`.
fn count_segments(passages: &[MultiridePassage], flag: impl Fn(&MultiridePassage) -> bool) -> usize {
    passages
        .iter()
        .filter(|p| flag(p))
        .map(|p| p.segment)
        .collect::<BTreeSet<usize>>()
        .len()
}

/// Projette un emprunt sur sa Feature : bornes en géométrie, métadonnées en
/// propriétés.
///
/// L'enregistrement d'annulation d'une fusion est projeté **par la même
/// fonction** que l'emprunt de tête : l'instantané est donc un arbre de
/// Features, où chaque niveau garde la forme du contrat de sortie.
fn passage_to_feature(passage: &MultiridePassage) -> FileFeature {
    FileFeature {
        kind: "Feature".to_string(),
        properties: FileFeatureProperties {
            segment: passage.segment,
            passage: passage.passage,
            sens: passage.sens,
            faux_positif: passage.faux_positif,
            point_entree: passage.point_entree,
            point_sortie: passage.point_sortie,
            km_entree: passage.km_entree,
            km_sortie: passage.km_sortie,
            longueur_km: passage.longueur_km,
            fusionne: passage.fusionne,
            avant_fusion: passage.avant_fusion.as_ref().map(|saved| {
                saved.iter().map(passage_to_feature).collect()
            }),
        },
        geometry: FileGeometry {
            kind: "LineString".to_string(),
            coordinates: [
                [passage.entree.lon, passage.entree.lat],
                [passage.sortie.lon, passage.sortie.lat],
            ],
        },
    }
}

/// Relit une Feature vers l'emprunt qu'elle décrit (réciproque de
/// `passage_to_feature`, enregistrement d'annulation compris).
fn feature_to_passage(feature: FileFeature) -> MultiridePassage {
    MultiridePassage {
        segment: feature.properties.segment,
        passage: feature.properties.passage,
        sens: feature.properties.sens,
        faux_positif: feature.properties.faux_positif,
        point_entree: feature.properties.point_entree,
        point_sortie: feature.properties.point_sortie,
        km_entree: feature.properties.km_entree,
        km_sortie: feature.properties.km_sortie,
        longueur_km: feature.properties.longueur_km,
        fusionne: feature.properties.fusionne,
        avant_fusion: feature
            .properties
            .avant_fusion
            .map(|saved| saved.into_iter().map(feature_to_passage).collect()),
        entree: MultirideLatLon {
            lat: feature.geometry.coordinates[0][1],
            lon: feature.geometry.coordinates[0][0],
        },
        sortie: MultirideLatLon {
            lat: feature.geometry.coordinates[1][1],
            lon: feature.geometry.coordinates[1][0],
        },
    }
}

/// Projette l'état interne sur le format de la spécification.
fn to_file(archive: &MultirideArchive) -> MultirideFile {
    let features = archive.passages.iter().map(passage_to_feature).collect();

    MultirideFile {
        kind: "FeatureCollection".to_string(),
        properties: FileProperties {
            version: archive.version,
            trace_id: archive.trace_id.clone(),
            valide: archive.valide,
            source: archive.source.clone(),
            date: archive.updated_at.clone(),
            trace: FileTrace {
                point_count: archive.trace_point_count,
                length_km: archive.trace_length_km,
            },
            parametres: FileParams {
                tolerance_m: archive.params.tolerance_m,
                longueur_min_m: archive.params.longueur_min_m,
                pas_echantillonnage_m: archive.params.pas_echantillonnage_m,
                fusion_references_m: archive.params.fusion_references_m,
                pas_plafonne: archive.pas_plafonne,
            },
            ajustements: FileAdjustments {
                fusions_manuelles: count_segments(&archive.passages, |p| p.fusionne),
                faux_positifs_exclus: count_segments(&archive.passages, |p| p.faux_positif),
            },
            note: NOTE.to_string(),
        },
        features,
    }
}

/// Relit le format de la spécification vers l'état interne.
fn from_file(file: MultirideFile) -> MultirideArchive {
    let properties = file.properties;
    MultirideArchive {
        version: properties.version,
        trace_id: properties.trace_id,
        source: properties.source,
        updated_at: properties.date,
        valide: properties.valide,
        params: MultirideParams {
            tolerance_m: properties.parametres.tolerance_m,
            longueur_min_m: properties.parametres.longueur_min_m,
            pas_echantillonnage_m: properties.parametres.pas_echantillonnage_m,
            fusion_references_m: properties.parametres.fusion_references_m,
        },
        trace_point_count: properties.trace.point_count,
        trace_length_km: properties.trace.length_km,
        pas_plafonne: properties.parametres.pas_plafonne,
        passages: file.features.into_iter().map(feature_to_passage).collect(),
    }
}

// ─── Entrées / sorties ────────────────────────────────────────────────

/// Écrit le fichier de description (écriture atomique). Le dossier de la trace
/// est créé au besoin.
pub fn save_file(path: &Path, archive: &MultirideArchive) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Création du dossier du fichier de passages multiples ({}): {}",
                parent.display(),
                e
            )
        })?;
    }
    let json = serde_json::to_string_pretty(&to_file(archive))
        .map_err(|e| format!("Sérialisation des passages multiples : {}", e))?;
    write_atomic(path, json.as_bytes())
}

/// Lit le fichier de description d'une trace.
///
/// Retourne `None` si le fichier est absent, illisible, d'une version inconnue
/// ou rattaché à une autre trace : l'appelant relance alors la détection.
pub fn load_file(path: &Path, trace_id: &str) -> Option<MultirideArchive> {
    let content = std::fs::read_to_string(path).ok()?;
    let file: MultirideFile = serde_json::from_str(&content).ok()?;
    if file.properties.version != FILE_VERSION || file.properties.trace_id != trace_id {
        return None;
    }
    Some(from_file(file))
}
