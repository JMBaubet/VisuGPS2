//! Moteur **hermes** de la détection des tunnels.
//!
//! Depuis l'intégration du serveur, la recherche ne lit plus les PBF de la clé
//! USB : les points de la trace partent vers `POST /tunnels/detect`, qui renvoie
//! les tunnels déjà appariés par `map_matching_2`. Ce module porte les deux
//! traitements locaux qui entourent cet appel :
//!
//! - la **conversion** de la réponse `camelCase` du serveur vers le contrat de
//!   `tunnels.json` (bornes arrondies au mètre, longueur recalculée,
//!   identifiants normalisés, `groupId` serveur ignoré) ;
//! - le **commit** : garde d'empreinte, écriture atomique et statut du registre.
//!
//! Le contrôle de couverture est fait par la commande, avant le commit, et
//! seulement sur un résultat vide : une liste non vide prouve à elle seule que
//! la trace est dans le dataset.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::gpx_audit::export::iso_now;
use crate::hermes::types::{
    HermesTunnel, HermesTunnelParams, HermesTunnelsDetectRequest, HermesTunnelsDetectResponse,
};
use crate::import_gpx::{get_traces_path, update_registry};

use super::file;
use super::types::{
    group_count, Tunnel, TunnelDetection, TunnelFile, TunnelParams, FILE_VERSION,
    STATUS_OUTSIDE_COVERAGE, STATUS_SUCCESS, STATUS_WAITING_VALIDATE,
};

/// Contrôle altimétrique désactivé : le serveur ne fait qu'annoter un tunnel
/// déjà apparié, et `trace.geojson` ne porte pas d'altitude. Les deux valeurs
/// sont transmises explicitement pour documenter la requête.
const ALTIMETRIC_CHECK: bool = false;
const ALTIMETRIC_TOLERANCE_M: f64 = 30.0;

/// Construit la requête de détection : identifiants, points 2D et réglages qui
/// gardent un sens côté client. Seule `minLengthM` provient des paramètres ;
/// `distanceTolerance`, `angleTolerance`, etc. ne pilotent que le moteur local
/// dormant.
pub fn build_detection_request(
    trace_id: &str,
    dataset_id: &str,
    points: Vec<[f64; 2]>,
    params: &TunnelParams,
) -> HermesTunnelsDetectRequest {
    HermesTunnelsDetectRequest {
        trace_id: trace_id.to_string(),
        dataset_id: dataset_id.to_string(),
        points,
        params: HermesTunnelParams {
            min_length_m: params.min_length_m,
            altimetric_check: ALTIMETRIC_CHECK,
            altimetric_tolerance_m: ALTIMETRIC_TOLERANCE_M,
        },
    }
}

/// Convertit les tunnels d'une réponse hermes au format de `tunnels.json`.
///
/// Le serveur calcule ses bornes sur la ligne complète ; on les arrondit au
/// mètre et on **recalcule** `lengthM` à partir des bornes arrondies, pour que
/// l'invariant du fichier — longueur = sortie − entrée — reste exact.
///
/// Normalisations :
/// - tunnels aux bornes non finies ou vides : ignorés ;
/// - identifiants vides ou dupliqués : remplacés par un `t-###` déterministe ;
/// - `groupId` du serveur **ignoré** : il identifie le tunnel physique et se
///   répète à chaque passage (aller et retour). L'importer comme groupe de
///   fusion local collerait les deux passages dans une seule carte et ferait
///   tracer une bande continue sur tout le trajet entre les deux. Le `groupId`
///   local reste réservé aux fusions **manuelles** de tronçons adjacents ;
/// - `wayId` = première way du tunnel logique, `wayIds` conserve la liste
///   complète.
pub fn tunnels_from_hermes(source: &[HermesTunnel]) -> Vec<Tunnel> {
    let mut ordered: Vec<&HermesTunnel> = source.iter().collect();
    ordered.sort_by(|a, b| a.start_km.total_cmp(&b.start_km));

    let mut candidates: Vec<(&HermesTunnel, f64, f64)> = Vec::new();
    for tunnel in ordered {
        if !tunnel.start_km.is_finite() || !tunnel.end_km.is_finite() {
            eprintln!(
                "[tunnels] Tunnel {} ignoré : bornes non finies ({}, {}).",
                tunnel.id, tunnel.start_km, tunnel.end_km
            );
            continue;
        }
        let start_km = round_metre(tunnel.start_km);
        let end_km = round_metre(tunnel.end_km);
        if end_km <= start_km {
            eprintln!(
                "[tunnels] Tunnel {} ignoré : bornes invalides ({:.3} → {:.3} km).",
                tunnel.id, start_km, end_km
            );
            continue;
        }
        candidates.push((tunnel, start_km, end_km));
    }

    // Les identifiants uniques du serveur sont réservés d'emblée : les
    // remplacements générés ne peuvent pas les voler. Un identifiant dupliqué
    // garde sa première occurrence, les suivantes sont remplacées.
    let mut id_counts: HashMap<&str, usize> = HashMap::new();
    for (tunnel, _, _) in &candidates {
        let id = tunnel.id.trim();
        if !id.is_empty() {
            *id_counts.entry(id).or_insert(0) += 1;
        }
    }
    let reserved: HashSet<String> = id_counts
        .iter()
        .filter(|(_, count)| **count == 1)
        .map(|(id, _)| (*id).to_string())
        .collect();

    let mut emitted: HashSet<String> = HashSet::new();
    let mut generated = 0_u32;

    candidates
        .into_iter()
        .map(|(tunnel, start_km, end_km)| {
            let raw_id = tunnel.id.trim();
            let id = if !raw_id.is_empty() && !emitted.contains(raw_id) {
                emitted.insert(raw_id.to_string());
                raw_id.to_string()
            } else {
                loop {
                    generated += 1;
                    let candidate = format!("t-{generated:03}");
                    if !reserved.contains(&candidate) && emitted.insert(candidate.clone()) {
                        break candidate;
                    }
                }
            };

            Tunnel {
                id,
                way_id: tunnel.way_ids.first().cloned(),
                way_ids: tunnel.way_ids.clone(),
                name: tunnel
                    .name
                    .clone()
                    .filter(|name| !name.trim().is_empty()),
                // Le serveur ne publie ni `highway` ni `tunnelType` : ces tags
                // restaient informatifs dans le fichier du moteur local.
                highway: None,
                tunnel_type: None,
                start_km,
                end_km,
                length_m: round_dec((end_km - start_km) * 1000.0, 1),
                detection_method: "osm".to_string(),
                max_distance_m: None,
                max_angle_diff_deg: None,
                // Le `groupId` serveur est une identité **physique** répétée à
                // chaque passage : il ne doit jamais devenir une fusion locale.
                group_id: None,
            }
        })
        .collect()
}

/// Arrondi au mètre : précision des bornes du fichier, qui absorbe les artefacts
/// binaires du calcul serveur.
fn round_metre(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn round_dec(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

/// Écrit le résultat de la détection et pose son statut, sous le verrou du
/// registre.
///
/// Retourne une erreur — sans rien écrire — quand un résultat **vide hors
/// couverture** ferait perdre un fichier existant : l'opérateur garde son
/// analyse et peut rejouer avec un dataset qui couvre la trace. Une première
/// détection, elle, enregistre le vide et le statut `outside_coverage`.
///
/// La garde d'empreinte est celle des communes : si le GPX a été réécrit pendant
/// l'analyse (audit appliqué), la détection est abandonnée et la trace sera
/// rejouée.
pub fn commit_hermes_detection(
    mode_dir: &Path,
    trace_id: &str,
    response: &HermesTunnelsDetectResponse,
    outside_coverage: bool,
    reference_hash: Option<&str>,
    params: &TunnelParams,
) -> Result<TunnelDetection, String> {
    let tunnels = tunnels_from_hermes(&response.tunnels);

    if outside_coverage && tunnels.is_empty() && file::file_path(mode_dir, trace_id).exists() {
        return Err(format!(
            "Trace hors de la couverture du dataset « {} » : le fichier de tunnels existant est conservé. Sélectionnez un dataset qui couvre la trace, puis relancez la détection.",
            response.dataset_id
        ));
    }

    let now = iso_now();
    let detected_at = if response.detected_at.trim().is_empty() {
        now.clone()
    } else {
        response.detected_at.clone()
    };

    let status = if tunnels.is_empty() {
        if outside_coverage {
            STATUS_OUTSIDE_COVERAGE
        } else {
            STATUS_SUCCESS
        }
    } else {
        STATUS_WAITING_VALIDATE
    };
    let count = group_count(&tunnels);

    let file = TunnelFile {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        detected_at: Some(detected_at),
        updated_at: now,
        // Aucun PBF local n'a participé : le dataset servi est la seule source.
        pbf_files: Vec::new(),
        params: params.clone(),
        tunnels,
    };

    let path = file::file_path(mode_dir, trace_id);
    let abandoned = update_registry(&get_traces_path(mode_dir), |registry| {
        let entry = registry
            .iter_mut()
            .find(|trace| trace.id == trace_id)
            .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;

        if reference_hash != Some(entry.hash.as_str()) {
            return Ok(true);
        }

        file::save_file(&path, &file)?;
        entry.tunnels_status = Some(status.to_string());
        entry.tunnels_count = Some(count);
        Ok(false)
    })?;

    if abandoned {
        return Err(
            "La trace a été modifiée pendant la détection des tunnels ; détection abandonnée."
                .to_string(),
        );
    }

    println!(
        "[tunnels] {} : {} tunnel(s) via hermes (dataset {}, statut {}).",
        trace_id,
        file.tunnels.len(),
        response.dataset_id,
        status
    );

    Ok(TunnelDetection {
        file,
        status: status.to_string(),
        dataset_id: response.dataset_id.clone(),
        outside_coverage,
    })
}
