//! Tests du moteur **hermes** du module Communes : conversion de la réponse du
//! serveur et contrôle de couverture.

use serde_json::json;

use crate::gpx_communes::hermes::{coverage_rings, is_outside_coverage, passages_from_hermes};
use crate::hermes::types::{HermesCommunePassage, HermesDataset, HermesPolygon, HermesRegion};

// ─── Aides de test ────────────────────────────────────────────────────

/// Dataset minimal : seul le mode de composition (`zones` XOR `regions`) et le
/// jeu d'identifiants importe au contrôle de couverture.
fn dataset(zones: Option<&[&str]>, regions: Option<&[&str]>) -> HermesDataset {
    HermesDataset {
        id: "ds-test".to_string(),
        name: None,
        regions: regions.map(|list| list.iter().map(|id| id.to_string()).collect()),
        zones: zones.map(|list| list.iter().map(|id| id.to_string()).collect()),
        created_at: "2026-11-01T00:00:00Z".to_string(),
        status: "ready".to_string(),
        state: Some("ready".to_string()),
        state_label: None,
        phase: None,
        artifacts: None,
        run_id: None,
        size_bytes: 0,
        ors_version: String::new(),
        stale: false,
        last_log_tail: None,
    }
}

fn polygon(id: &str, geometry: serde_json::Value) -> HermesPolygon {
    HermesPolygon {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        color: None,
        geometry,
        bbox: [0.0, 0.0, 10.0, 10.0],
        created_at: "2026-11-01T00:00:00Z".to_string(),
        updated_at: "2026-11-01T00:00:00Z".to_string(),
        revision: 1,
    }
}

fn region(id: &str, bbox: [f64; 4], geojson: Option<serde_json::Value>) -> HermesRegion {
    HermesRegion {
        id: id.to_string(),
        bbox,
        has_poly: geojson.is_some(),
        geojson,
        geofabrik_id: None,
        updated_at: None,
    }
}

/// Passage serveur (camelCase côté JSON, snake_case côté struct).
fn passage(
    commune: &str,
    km_entree: f64,
    km_sortie: f64,
    longueur_km: f64,
    ambiguous: bool,
) -> HermesCommunePassage {
    HermesCommunePassage {
        commune: commune.to_string(),
        passage: 1,
        nb_passages_total: 1,
        km_entree,
        km_sortie,
        longueur_km,
        ambiguous,
    }
}

// ─── Conversion ───────────────────────────────────────────────────────

#[test]
fn conversion_rounds_to_metre_and_recomputes_length() {
    let converted = passages_from_hermes(&[passage("Nice", 0.012_34, 6.684_56, 6.672_22, false)]);

    assert_eq!(converted.len(), 1);
    let passage = &converted[0];
    assert_eq!(passage.km_entree, 0.012);
    assert_eq!(passage.km_sortie, 6.685);
    // La longueur est recalculée sur les bornes arrondies (6.685 − 0.012), et
    // non reprise du serveur (6.672_22) : l'invariant du fichier reste exact.
    assert_eq!(passage.longueur_km, 6.673);
    assert!(!passage.ambiguous);
}

#[test]
fn conversion_preserves_ambiguous_and_multiple_passages() {
    let converted = passages_from_hermes(&[
        passage("Monaco", 1.0, 1.4, 0.4, true),
        passage("Nice", 1.4, 8.05, 6.65, false),
    ]);

    assert_eq!(converted.len(), 2);
    assert_eq!(converted[0].commune, "Monaco");
    assert!(converted[0].ambiguous);
    assert_eq!(converted[1].commune, "Nice");
    assert!(!converted[1].ambiguous);
}

#[test]
fn conversion_of_an_empty_list_is_an_empty_file_content() {
    assert!(passages_from_hermes(&[]).is_empty());
}

// ─── Couverture ───────────────────────────────────────────────────────

/// Couverture en T : barre horizontale (0,0)–(10,2), jambe (4,2)–(6,8).
/// Le point (1,4) est dans la boîte englobante du T mais hors de ses deux
/// polygones : c'est exactement le cas que la bbox seule ne voit pas.
fn t_shaped_rings() -> Vec<Vec<[f64; 2]>> {
    let geometry = json!({
        "type": "MultiPolygon",
        "coordinates": [
            [ [[0.0, 0.0], [10.0, 0.0], [10.0, 2.0], [0.0, 2.0], [0.0, 0.0]] ],
            [ [[4.0, 2.0], [6.0, 2.0], [6.0, 8.0], [4.0, 8.0], [4.0, 2.0]] ]
        ]
    });
    let ds = dataset(Some(&["zone-t"]), None);
    let polygons = vec![polygon("zone-t", geometry)];
    coverage_rings(&ds, &[], &polygons)
}

#[test]
fn coverage_reads_every_part_of_a_multipolygon() {
    let rings = t_shaped_rings();
    assert_eq!(rings.len(), 2);
}

#[test]
fn coverage_flags_a_trace_in_the_notch_of_a_t_shape() {
    let rings = t_shaped_rings();

    // Sous la barre horizontale, hors de la jambe : hors couverture.
    assert!(is_outside_coverage(&[[1.0, 4.0]], &rings));
    // Dans la jambe et dans la barre : couvert.
    assert!(!is_outside_coverage(&[[5.0, 5.0]], &rings));
    assert!(!is_outside_coverage(&[[1.0, 1.0]], &rings));
    // Trace à cheval : un seul point couvert suffit à ne pas alerter.
    assert!(!is_outside_coverage(&[[1.0, 4.0], [5.0, 5.0]], &rings));
}

#[test]
fn coverage_ignores_holes_conservatively() {
    let geometry = json!({
        "type": "Polygon",
        "coordinates": [
            [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
            [[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0], [4.0, 4.0]]
        ]
    });
    let ds = dataset(Some(&["zone"]), None);
    let rings = coverage_rings(&ds, &[], &[polygon("zone", geometry)]);

    assert_eq!(rings.len(), 1, "seul l'anneau extérieur est retenu");
    // Un point dans le trou est compté comme couvert : le contrôle est
    // conservateur, il ne produit pas de fausse alerte.
    assert!(!is_outside_coverage(&[[5.0, 5.0]], &rings));
}

#[test]
fn coverage_falls_back_to_the_region_bbox_without_contour() {
    let ds = dataset(None, Some(&["paca"]));
    let regions = vec![region("paca", [0.0, 0.0, 10.0, 10.0], None)];
    let rings = coverage_rings(&ds, &regions, &[]);

    assert_eq!(rings.len(), 1);
    assert!(!is_outside_coverage(&[[5.0, 5.0]], &rings));
    assert!(is_outside_coverage(&[[20.0, 20.0]], &rings));
}

#[test]
fn coverage_uses_the_region_geojson_when_published() {
    let ds = dataset(None, Some(&["paca"]));
    let contour = json!({
        "type": "Polygon",
        "coordinates": [
            [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0], [0.0, 0.0]]
        ]
    });
    let regions = vec![region("paca", [0.0, 0.0, 10.0, 10.0], Some(contour))];
    let rings = coverage_rings(&ds, &regions, &[]);

    // Le contour réel prime sur la bbox : (5,5) est dans la bbox mais dehors.
    assert!(is_outside_coverage(&[[5.0, 5.0]], &rings));
    assert!(!is_outside_coverage(&[[1.0, 1.0]], &rings));
}

#[test]
fn coverage_without_geometry_never_alerts() {
    let ds = dataset(Some(&["inconnu"]), None);
    let rings = coverage_rings(&ds, &[], &[]);

    assert!(rings.is_empty());
    assert!(!is_outside_coverage(&[[1.0, 4.0]], &rings));
    assert!(!is_outside_coverage(&[], &t_shaped_rings()));
}
