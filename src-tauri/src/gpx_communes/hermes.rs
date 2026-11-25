//! Moteur **hermes** de la détection des communes.
//!
//! Depuis l'intégration du serveur, la détection ne croise plus la trace avec
//! une réponse Overpass : la géométrie part vers `POST /communes/detect`, qui
//! renvoie les passages déjà calculés. Ce module porte les deux traitements
//! locaux qui entourent cet appel :
//!
//! - la **conversion** de la réponse `camelCase` du serveur vers le contrat du
//!   fichier `communes.json` (arrondi au mètre, invariant
//!   `longueur = sortie − entrée`) ;
//! - le **contrôle de couverture**, qui distingue « aucune commune traversée »
//!   de « trace hors du dataset servi » — le serveur répond `passages: []` dans
//!   les deux cas, sans erreur.
//!
//! Le calcul de couverture porte sur les **polygones** du dataset (secteurs
//! assemblés, ou contours GeoJSON des régions) et non sur sa boîte englobante :
//! un dataset en forme de T dont la trace passe sous la barre horizontale, hors
//! de la jambe, n'est pas couvert. Quand le serveur ne publie qu'un rectangle
//! (région sans `.poly`), le contrôle hérite de cette imprécision — il ne
//! produit alors aucune fausse alerte, mais peut manquer un trou.

use std::collections::HashSet;

use crate::hermes::types::{HermesCommunePassage, HermesDataset, HermesPolygon, HermesRegion};

use super::geometry::point_in_ring;
use super::types::CommunePassage;

/// Convertit les passages d'une réponse hermes au format du fichier
/// `communes.json`.
///
/// Le serveur calcule ses bornes sur la ligne complète ; on les arrondit au
/// mètre et on **recalcule** `longueur_km` à partir des bornes arrondies, pour
/// que l'invariant du fichier — déjà tenu par le moteur Overpass — reste exact
/// au millimètre près.
pub fn passages_from_hermes(source: &[HermesCommunePassage]) -> Vec<CommunePassage> {
    source
        .iter()
        .map(|passage| {
            let km_entree = round_metre(passage.km_entree);
            let km_sortie = round_metre(passage.km_sortie);
            CommunePassage {
                commune: passage.commune.clone(),
                passage: passage.passage,
                nb_passages_total: passage.nb_passages_total,
                km_entree,
                km_sortie,
                longueur_km: round_metre(km_sortie - km_entree),
                ambiguous: passage.ambiguous,
            }
        })
        .collect()
}

/// Arrondi au mètre : c'est la précision du fichier historique, et elle absorbe
/// les artefacts binaires du calcul serveur.
fn round_metre(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Anneaux extérieurs de la couverture d'un dataset.
///
/// En mode `zones`, la géométrie exacte des secteurs assemblés : c'est le cas
/// où le contrôle est le plus fidèle. En mode `regions`, le contour GeoJSON
/// publié par le serveur — repli sur un rectangle quand la région n'a pas de
/// `.poly`. Un dataset dont aucune géométrie n'est retrouvée laisse la liste
/// vide : le contrôle ne conclut alors rien.
pub fn coverage_rings(
    dataset: &HermesDataset,
    regions: &[HermesRegion],
    polygons: &[HermesPolygon],
) -> Vec<Vec<[f64; 2]>> {
    let mut rings = Vec::new();

    if let Some(zones) = dataset.zones.as_deref().filter(|zones| !zones.is_empty()) {
        let wanted: HashSet<&str> = zones.iter().map(String::as_str).collect();
        for polygon in polygons {
            if wanted.contains(polygon.id.as_str()) {
                rings.extend(outer_rings(&polygon.geometry));
            }
        }
        return rings;
    }

    if let Some(region_ids) = dataset
        .regions
        .as_deref()
        .filter(|regions| !regions.is_empty())
    {
        let wanted: HashSet<&str> = region_ids.iter().map(String::as_str).collect();
        for region in regions {
            if !wanted.contains(region.id.as_str()) {
                continue;
            }
            match region.geojson.as_ref() {
                Some(geometry) => {
                    let contour = outer_rings(geometry);
                    if contour.is_empty() {
                        rings.push(bbox_ring(region.bbox));
                    } else {
                        rings.extend(contour);
                    }
                }
                None => rings.push(bbox_ring(region.bbox)),
            }
        }
    }

    rings
}

/// Vrai quand **aucun** point de la trace ne tombe dans la couverture.
///
/// Liste d'anneaux ou trace vide → `false` : faute de pouvoir conclure, mieux
/// vaut ne pas alerter. Le contrôle s'arrête au premier point couvert, ce qui
/// couvre les traces longues dont seule une extrémité est indexée.
pub fn is_outside_coverage(points: &[[f64; 2]], rings: &[Vec<[f64; 2]>]) -> bool {
    if rings.is_empty() || points.is_empty() {
        return false;
    }
    !points
        .iter()
        .any(|point| rings.iter().any(|ring| point_in_ring(ring, *point)))
}

/// Anneaux extérieurs d'une géométrie GeoJSON `Polygon` ou `MultiPolygon`.
///
/// Les trous sont ignorés : un point dans un trou est compté comme couvert
/// (`point_in_ring` porte sur l'anneau extérieur). C'est le choix conservateur —
/// le risque est de manquer une alerte, jamais d'en émettre une fausse.
fn outer_rings(geometry: &serde_json::Value) -> Vec<Vec<[f64; 2]>> {
    let kind = geometry.get("type").and_then(serde_json::Value::as_str);
    let Some(coordinates) = geometry.get("coordinates").and_then(|c| c.as_array()) else {
        return Vec::new();
    };

    match kind {
        Some("Polygon") => coordinates
            .first()
            .and_then(ring_of_value)
            .into_iter()
            .collect(),
        Some("MultiPolygon") => coordinates
            .iter()
            .filter_map(|polygon| polygon.as_array().and_then(|rings| rings.first()))
            .filter_map(ring_of_value)
            .collect(),
        // `LineString`, `Point`… : une couverture n'a pas de sens, on ignore.
        _ => Vec::new(),
    }
}

/// Convertit un anneau GeoJSON (`[[lon, lat], …]`) en liste de points.
fn ring_of_value(value: &serde_json::Value) -> Option<Vec<[f64; 2]>> {
    let points = value.as_array()?;
    let ring: Vec<[f64; 2]> = points
        .iter()
        .filter_map(|point| {
            let pair = point.as_array()?;
            Some([pair.first()?.as_f64()?, pair.get(1)?.as_f64()?])
        })
        .collect();
    // `point_in_ring` exige au moins quatre sommets (anneau fermé).
    (ring.len() >= 4).then_some(ring)
}

/// Anneau rectangulaire d'une bbox `[minLon, minLat, maxLon, maxLat]`.
fn bbox_ring(bbox: [f64; 4]) -> Vec<[f64; 2]> {
    let [min_lon, min_lat, max_lon, max_lat] = bbox;
    vec![
        [min_lon, min_lat],
        [max_lon, min_lat],
        [max_lon, max_lat],
        [min_lon, max_lat],
        [min_lon, min_lat],
    ]
}
