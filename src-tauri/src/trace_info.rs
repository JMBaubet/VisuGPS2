//! Informations de synthèse d'un circuit, pour le volet « Info » de la carte.
//!
//! La commande `get_trace_info` agrège, **en lecture seule**, les artefacts
//! déjà écrits par les modules :
//! - `audit.json` — compteurs d'aller-retours traités / non traités ;
//! - `multiride.json` — compteurs de segments traités / non traités ;
//! - `altimetrie.json` — montées simples / fusionnées / séparées et profil
//!   d'altitude sous-échantillonné ;
//! - `communes.json` — nombre de communes distinctes traversées.
//!
//! Elle évite à la carte de charger les archives complètes (points d'audit,
//! emprunts, profil de plusieurs milliers de points) pour n'afficher que des
//! compteurs et une sparkline. Chaque bloc est `None` quand le fichier
//! correspondant est absent ou illisible : ce n'est pas une erreur, la carte
//! affiche simplement l'information comme indisponible.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::gpx_altimetrie::types::{
    AltimetrieClimb, AltimetrieFile, AltimetrieProfilePoint,
};
use crate::gpx_audit::types::{FindingKind, FindingStatus};
use crate::gpx_communes::types::CommunePassage;
use crate::gpx_multiride::types::MultiridePassage;
use crate::import_gpx::get_mode_dir;

/// Nombre maximal de points du profil renvoyés à la carte. Le volet Info
/// n'affiche qu'une sparkline : inutile de transférer les milliers de points
/// du profil complet.
const PROFILE_SAMPLE_MAX: usize = 120;

// ─── Contrats IPC ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditInfo {
    pub ar_total: usize,
    pub ar_treated: usize,
    pub ar_pending: usize,
    pub rp_total: usize,
    pub rp_treated: usize,
    pub rp_pending: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultirideInfo {
    pub segments_total: usize,
    pub segments_treated: usize,
    pub segments_pending: usize,
    pub updated_at: String,
}

/// Point allégé du profil : la carte n'a besoin que de la distance et de
/// l'altitude pour tracer la sparkline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfilePoint {
    pub d: f64,
    pub ele: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClimbInfo {
    pub index: usize,
    pub start_km: f64,
    pub end_km: f64,
    pub avg_slope_pct: f64,
    pub is_merged: bool,
    pub is_separated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieInfo {
    pub simple_climbs: usize,
    pub merged_climbs: usize,
    pub separated_climbs: usize,
    /// L'opérateur a validé l'analyse (sauvegarde depuis la vue Altimétrie).
    pub validated: bool,
    pub updated_at: String,
    pub profile: Vec<ProfilePoint>,
    pub climbs: Vec<ClimbInfo>,
    pub alt_min_m: f64,
    pub alt_max_m: f64,
    pub total_distance_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunesInfo {
    pub commune_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceInfo {
    pub audit: Option<AuditInfo>,
    pub multiride: Option<MultirideInfo>,
    pub altimetrie: Option<AltimetrieInfo>,
    pub communes: Option<CommunesInfo>,
}

// ─── Lecture allégée de l'archive d'audit ─────────────────────────────

/// Vue partielle d'un finding : seuls son type et son statut comptent.
#[derive(Debug, Serialize, Deserialize)]
struct FindingSummary {
    kind: FindingKind,
    status: FindingStatus,
}

/// Vue partielle de `audit.json` : le volet Info n'a besoin que des findings
/// et de l'horodatage. Serde ignore les champs non déclarés, en particulier
/// `points` et les enregistrements d'annulation — la trace de travail peut
/// peser plusieurs mégaoctets et n'a aucune raison d'être désérialisée pour
/// compter des anomalies.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditSummaryFile {
    version: u32,
    trace_id: String,
    updated_at: String,
    findings: Vec<FindingSummary>,
}

fn load_audit_summary(mode_dir: &Path, trace_id: &str) -> Option<AuditInfo> {
    let path = crate::gpx_audit::archive::archive_path(mode_dir, trace_id);
    let content = std::fs::read_to_string(path).ok()?;
    let file: AuditSummaryFile = serde_json::from_str(&content).ok()?;
    // Même tolérance que `archive::load_archive` : version inconnue ou archive
    // rattachée à une autre trace → pas d'information.
    if file.version != crate::gpx_audit::archive::ARCHIVE_VERSION
        || file.trace_id != trace_id
    {
        return None;
    }
    Some(audit_info(&file.findings, &file.updated_at))
}

// ─── Agrégations pures ────────────────────────────────────────────────

/// Compteurs d'une famille de findings (AR ou RP).
///
/// Un finding est traité dès qu'il n'est plus `pending` : corrigé (suppression
/// ou routage) ou marqué faux positif.
fn count_findings(
    findings: &[FindingSummary],
    kind: FindingKind,
) -> (usize, usize, usize) {
    let total = findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count();
    let treated = findings
        .iter()
        .filter(|finding| {
            finding.kind == kind && finding.status != FindingStatus::Pending
        })
        .count();
    (total, treated, total - treated)
}

/// Compteurs des **aller-retours** et des **rond-points** d'un audit.
fn audit_info(findings: &[FindingSummary], updated_at: &str) -> AuditInfo {
    let (ar_total, ar_treated, ar_pending) =
        count_findings(findings, FindingKind::Ar);
    let (rp_total, rp_treated, rp_pending) =
        count_findings(findings, FindingKind::Rp);

    AuditInfo {
        ar_total,
        ar_treated,
        ar_pending,
        rp_total,
        rp_treated,
        rp_pending,
        updated_at: updated_at.to_string(),
    }
}

/// Compteurs des **segments** de passages multiples.
///
/// Un segment est traité quand un de ses emprunts porte un verdict —
/// approuvé (`valide`) ou écarté (`faux_positif`). Un segment fusionné sans
/// verdict reste à examiner, comme dans la vue Multiride.
fn multiride_info(passages: &[MultiridePassage], updated_at: &str) -> MultirideInfo {
    let mut segments: BTreeMap<usize, bool> = BTreeMap::new();
    for passage in passages {
        let treated = segments.entry(passage.segment).or_insert(false);
        *treated = *treated || passage.valide || passage.faux_positif;
    }

    let total = segments.len();
    let treated = segments.values().filter(|value| **value).count();

    MultirideInfo {
        segments_total: total,
        segments_treated: treated,
        segments_pending: total - treated,
        updated_at: updated_at.to_string(),
    }
}

/// Montées, profil sous-échantillonné et bornes d'altitude.
fn altimetrie_info(
    profile: &[AltimetrieProfilePoint],
    climbs: &[AltimetrieClimb],
    validated: bool,
    updated_at: &str,
    fallback_total_distance_m: f64,
) -> AltimetrieInfo {
    let simple = climbs
        .iter()
        .filter(|climb| !climb.is_merged && !climb.is_separated)
        .count();
    let merged = climbs.iter().filter(|climb| climb.is_merged).count();
    let separated = climbs.iter().filter(|climb| climb.is_separated).count();

    let (alt_min, alt_max) = profile.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min, max), point| (min.min(point.ele), max.max(point.ele)),
    );
    let has_profile = !profile.is_empty();

    AltimetrieInfo {
        simple_climbs: simple,
        merged_climbs: merged,
        separated_climbs: separated,
        validated,
        updated_at: updated_at.to_string(),
        profile: sample_profile(profile),
        climbs: climbs
            .iter()
            .map(|climb| ClimbInfo {
                index: climb.index,
                start_km: climb.start_km,
                end_km: climb.end_km,
                avg_slope_pct: climb.avg_slope_pct,
                is_merged: climb.is_merged,
                is_separated: climb.is_separated,
            })
            .collect(),
        alt_min_m: if has_profile { alt_min } else { 0.0 },
        alt_max_m: if has_profile { alt_max } else { 0.0 },
        total_distance_m: profile
            .last()
            .map(|point| point.d)
            .unwrap_or(fallback_total_distance_m),
    }
}

/// Sous-échantillonne le profil à `PROFILE_SAMPLE_MAX` points, en conservant
/// toujours le dernier point (la fin de la trace).
fn sample_profile(profile: &[AltimetrieProfilePoint]) -> Vec<ProfilePoint> {
    if profile.is_empty() {
        return Vec::new();
    }

    let step = profile.len().div_ceil(PROFILE_SAMPLE_MAX).max(1);
    let mut sampled: Vec<ProfilePoint> = profile
        .iter()
        .step_by(step)
        .map(|point| ProfilePoint {
            d: point.d,
            ele: point.ele,
        })
        .collect();

    if let Some(last) = profile.last() {
        if sampled.last().map(|point| point.d) != Some(last.d) {
            sampled.push(ProfilePoint {
                d: last.d,
                ele: last.ele,
            });
        }
    }
    sampled
}

/// Nombre de **communes distinctes** traversées. Le fichier décrit chaque
/// passage : une commune empruntée deux fois ne compte qu'une fois.
fn communes_info(passages: &[CommunePassage]) -> CommunesInfo {
    let names: BTreeSet<&str> = passages
        .iter()
        .map(|passage| passage.commune.as_str())
        .collect();
    CommunesInfo {
        commune_count: names.len(),
    }
}

// ─── Construction et commande ─────────────────────────────────────────

/// Agrège les quatre artefacts d'une trace. Tolérant : un fichier absent ou
/// illisible laisse simplement son bloc à `None`.
pub fn build_trace_info(mode_dir: &Path, trace_id: &str) -> TraceInfo {
    let audit = load_audit_summary(mode_dir, trace_id);

    let multiride = crate::gpx_multiride::commands::load_impl(mode_dir, trace_id)
        .map(|archive| multiride_info(&archive.passages, &archive.updated_at));

    let altimetrie_validated = crate::import_gpx::load_registry(
        &crate::import_gpx::get_traces_path(mode_dir),
    )
    .into_iter()
    .find(|trace| trace.id == trace_id)
    .map(|trace| trace.altimetrie_validated)
    .unwrap_or(false);

    let altimetrie = crate::gpx_altimetrie::file::load_file(
        &crate::gpx_altimetrie::file::file_path(mode_dir, trace_id),
        trace_id,
    )
    .map(|file: AltimetrieFile| {
        altimetrie_info(
            &file.profile,
            &file.climbs,
            altimetrie_validated,
            &file.updated_at,
            file.trace.total_distance_m,
        )
    });

    let communes = crate::gpx_communes::file::load_file(mode_dir, trace_id)
        .map(|passages| communes_info(&passages));

    TraceInfo {
        audit,
        multiride,
        altimetrie,
        communes,
    }
}

/// Informations de synthèse d'un circuit (volet Info de la carte).
#[tauri::command]
pub async fn get_trace_info(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TraceInfo, String> {
    let mode_dir = get_mode_dir(&app)?;
    Ok(build_trace_info(&mode_dir, &trace_id))
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpx_altimetrie::types::AltimetrieMergedPart;
    use crate::gpx_multiride::types::{MultirideLatLon, MultirideSens};

    fn finding(kind: FindingKind, status: FindingStatus) -> FindingSummary {
        FindingSummary { kind, status }
    }

    fn passage(segment: usize, valide: bool, faux_positif: bool) -> MultiridePassage {
        MultiridePassage {
            segment,
            passage: 1,
            sens: MultirideSens::Reference,
            faux_positif,
            point_entree: 1,
            point_sortie: 2,
            km_entree: 0.0,
            km_sortie: 1.0,
            longueur_km: 1.0,
            fusionne: false,
            valide,
            avant_fusion: None,
            entree: MultirideLatLon {
                lat: 45.0,
                lon: 2.0,
            },
            sortie: MultirideLatLon {
                lat: 45.0,
                lon: 2.1,
            },
        }
    }

    fn climb(
        index: usize,
        is_merged: bool,
        is_separated: bool,
        avg_slope_pct: f64,
    ) -> AltimetrieClimb {
        AltimetrieClimb {
            id: index as u32 + 1,
            index,
            start_km: index as f64 * 2.0,
            end_km: index as f64 * 2.0 + 1.5,
            start_progress: 0.0,
            end_progress: 0.1,
            d_plus_m: 100.0,
            avg_slope_pct,
            alt_start_m: 100.0,
            alt_end_m: 200.0,
            merged_count: if is_merged { 1 } else { 0 },
            fallback: false,
            is_merged,
            is_separated,
            group_id: if is_separated { Some(1) } else { None },
            merged_parts: if is_merged {
                Some(vec![AltimetrieMergedPart {
                    part_index: 0,
                    start_km: 0.0,
                    end_km: 0.5,
                    start_progress: 0.0,
                    end_progress: 0.05,
                    d_plus_m: 50.0,
                    avg_slope_pct: 5.0,
                    alt_start_m: 100.0,
                    alt_end_m: 150.0,
                }])
            } else {
                None
            },
        }
    }

    fn profile_point(d: f64, ele: f64) -> AltimetrieProfilePoint {
        AltimetrieProfilePoint {
            d,
            ele,
            lat: 45.0,
            lon: 2.0,
        }
    }

    fn commune(name: &str) -> CommunePassage {
        CommunePassage {
            commune: name.to_string(),
            passage: 1,
            nb_passages_total: 1,
            km_entree: 0.0,
            km_sortie: 1.0,
            longueur_km: 1.0,
        }
    }

    #[test]
    fn audit_counts_aller_retours_and_roundabouts() {
        let findings = vec![
            finding(FindingKind::Ar, FindingStatus::Pending),
            finding(FindingKind::Ar, FindingStatus::Corrected),
            finding(FindingKind::Ar, FindingStatus::Fp),
            finding(FindingKind::Rp, FindingStatus::Pending),
            finding(FindingKind::Rp, FindingStatus::Corrected),
        ];

        let info = audit_info(&findings, "2026-10-20T10:00:00Z");
        assert_eq!(info.ar_total, 3);
        assert_eq!(info.ar_treated, 2);
        assert_eq!(info.ar_pending, 1);
        assert_eq!(info.rp_total, 2);
        assert_eq!(info.rp_treated, 1);
        assert_eq!(info.rp_pending, 1);
        assert_eq!(info.updated_at, "2026-10-20T10:00:00Z");
    }

    #[test]
    fn multiride_counts_segments_once() {
        let passages = vec![
            passage(1, true, false),
            passage(2, false, true),
            passage(3, false, false),
            // Un segment fusionné sans verdict reste à examiner.
            passage(4, false, false),
        ];

        let info = multiride_info(&passages, "2026-10-20T11:00:00Z");
        assert_eq!(info.segments_total, 4);
        assert_eq!(info.segments_treated, 2);
        assert_eq!(info.segments_pending, 2);
    }

    #[test]
    fn altimetrie_counts_climbs_and_samples_the_profile() {
        let climbs = vec![
            climb(0, false, false, 4.0),
            climb(1, false, false, 6.0),
            climb(2, true, false, 8.0),
            climb(3, false, true, 5.0),
        ];
        let profile: Vec<_> = (0..=1000)
            .map(|index| profile_point(index as f64 * 10.0, 100.0 + index as f64))
            .collect();

        let info = altimetrie_info(
            &profile,
            &climbs,
            true,
            "2026-10-20T12:00:00Z",
            9999.0,
        );
        assert_eq!(info.simple_climbs, 2);
        assert_eq!(info.merged_climbs, 1);
        assert_eq!(info.separated_climbs, 1);
        assert!(info.validated);
        assert_eq!(info.updated_at, "2026-10-20T12:00:00Z");
        assert!(info.profile.len() <= PROFILE_SAMPLE_MAX + 1);
        assert_eq!(info.profile.last().map(|point| point.d), Some(10_000.0));
        assert_eq!(info.alt_min_m, 100.0);
        assert_eq!(info.alt_max_m, 1100.0);
        assert_eq!(info.total_distance_m, 10_000.0);
    }

    #[test]
    fn communes_counts_distinct_names() {
        let passages = vec![commune("Toulouse"), commune("Toulouse"), commune("Blagnac")];
        let info = communes_info(&passages);
        assert_eq!(info.commune_count, 2);
    }

    #[test]
    fn audit_file_is_read_without_its_points() {
        let mode_dir = std::env::temp_dir().join(format!(
            "vg2-trace-info-{}",
            uuid::Uuid::new_v4()
        ));
        let trace_id = "t-audit";
        let trace_dir = mode_dir.join("traces").join(trace_id);
        std::fs::create_dir_all(&trace_dir).expect("dossier de trace");

        // Le fichier ne porte volontairement pas de `points` : la lecture
        // partielle doit suffire au volet Info.
        let file = AuditSummaryFile {
            version: crate::gpx_audit::archive::ARCHIVE_VERSION,
            trace_id: trace_id.to_string(),
            updated_at: "2026-10-20T10:00:00Z".to_string(),
            findings: vec![
                finding(FindingKind::Ar, FindingStatus::Corrected),
                finding(FindingKind::Ar, FindingStatus::Pending),
                finding(FindingKind::Rp, FindingStatus::Fp),
            ],
        };
        std::fs::write(
            crate::gpx_audit::archive::archive_path(&mode_dir, trace_id),
            serde_json::to_string(&file).expect("sérialisation"),
        )
        .expect("écriture de l'archive");

        let info = load_audit_summary(&mode_dir, trace_id).expect("archive lue");
        assert_eq!(info.ar_total, 2);
        assert_eq!(info.ar_treated, 1);
        assert_eq!(info.ar_pending, 1);
        assert_eq!(info.rp_total, 1);
        assert_eq!(info.rp_treated, 1);
        assert_eq!(info.rp_pending, 0);

        let _ = std::fs::remove_dir_all(&mode_dir);
    }
}
