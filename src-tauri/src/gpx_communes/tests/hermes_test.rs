//! Tests du moteur **hermes** du module Communes : conversion de la réponse du
//! serveur et contrôle de couverture.

use serde_json::json;

use crate::gpx_communes::hermes::{
    coverage_rings, is_outside_coverage, passages_from_hermes, smooth_short_passages,
};
use crate::gpx_communes::types::{CommunePassage, coverage_is_partial};
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

// ─── Lissage des passages courts ──────────────────────────────────────

/// Passage du fichier, sans ambiguïté : la numérotation sera recalculée.
fn span(commune: &str, entry: f64, exit: f64) -> CommunePassage {
    CommunePassage {
        commune: commune.to_string(),
        passage: 1,
        nb_passages_total: 1,
        km_entree: entry,
        km_sortie: exit,
        longueur_km: exit - entry,
        ambiguous: false,
    }
}

/// La séquence réellement rendue par hermes sur « Stage Émotion Bike. Menton :
/// J5 » : quatre allers-retours Nice ↔ Villefranche en 285 m, un Monaco de 40 m
/// et un Beausoleil de 38 m.
fn boundary_sequence() -> Vec<CommunePassage> {
    [
        ("Menton", 0.000, 1.877),
        ("Roquebrune-Cap-Martin", 1.877, 8.764),
        ("Monaco", 8.764, 8.804),
        ("Roquebrune-Cap-Martin", 8.804, 9.431),
        ("Monaco", 9.431, 13.255),
        ("Cap-d'Ail", 13.255, 16.139),
        ("Èze", 16.139, 20.315),
        ("Villefranche-sur-Mer", 20.315, 20.722),
        ("Beaulieu-sur-Mer", 20.722, 22.894),
        ("Villefranche-sur-Mer", 22.894, 25.996),
        ("Nice", 25.996, 26.140),
        ("Villefranche-sur-Mer", 26.140, 26.221),
        ("Nice", 26.221, 26.256),
        ("Villefranche-sur-Mer", 26.256, 26.281),
        ("Nice", 26.281, 36.657),
        ("La Trinité", 36.657, 38.229),
        ("Nice", 38.229, 39.363),
        ("Villefranche-sur-Mer", 39.363, 41.329),
        ("Èze", 41.329, 47.877),
        ("La Turbie", 47.877, 51.072),
        ("Beausoleil", 51.072, 51.110),
        ("La Turbie", 51.110, 51.195),
        ("Beausoleil", 51.195, 52.674),
        ("Roquebrune-Cap-Martin", 52.674, 60.375),
        ("Menton", 60.375, 62.249),
    ]
    .into_iter()
    .map(|(commune, entry, exit)| span(commune, entry, exit))
    .collect()
}

#[test]
fn smoothing_absorbs_the_boundary_micro_passages() {
    let smoothed = smooth_short_passages(&boundary_sequence(), 0.5);

    assert_eq!(smoothed.len(), 16, "25 passages bruts → 16 après absorption");

    let roquebrune = &smoothed[1];
    assert_eq!(roquebrune.commune, "Roquebrune-Cap-Martin");
    assert_eq!((roquebrune.km_entree, roquebrune.km_sortie), (1.877, 9.431));
    assert_eq!(roquebrune.longueur_km, 7.554, "le Monaco de 40 m est absorbé");
    assert_eq!((roquebrune.passage, roquebrune.nb_passages_total), (1, 2));

    // Les deux passages Roquebrune devenus adjacents sont recollés : le second
    // passage de la commune est celui de la fin de trace.
    let last_roquebrune = smoothed
        .iter()
        .find(|passage| passage.commune == "Roquebrune-Cap-Martin" && passage.passage == 2)
        .expect("second passage Roquebrune");
    assert_eq!(
        (last_roquebrune.km_entree, last_roquebrune.km_sortie),
        (52.674, 60.375)
    );

    let nice = &smoothed[7];
    assert_eq!(nice.commune, "Nice");
    assert_eq!((nice.km_entree, nice.km_sortie), (26.256, 36.657));
    assert_eq!(nice.longueur_km, 10.401, "les allers-retours sont absorbés");

    let la_turbie = &smoothed[12];
    assert_eq!(la_turbie.commune, "La Turbie");
    assert_eq!(
        (la_turbie.km_entree, la_turbie.km_sortie),
        (47.877, 51.195)
    );

    // 11 communes conservées, couverture totale conservée.
    let communes: std::collections::HashSet<&str> =
        smoothed.iter().map(|passage| passage.commune.as_str()).collect();
    assert_eq!(communes.len(), 11);
    let total: f64 = smoothed.iter().map(|passage| passage.longueur_km).sum();
    assert!((total - 62.249).abs() < 1e-9, "couverture : {total}");
}

#[test]
fn smoothing_keeps_the_length_invariant_and_renumbers() {
    let smoothed = smooth_short_passages(&boundary_sequence(), 0.5);

    for passage in &smoothed {
        assert!(
            (passage.longueur_km - (passage.km_sortie - passage.km_entree)).abs() < 1e-9,
            "invariant longueur = sortie − entrée pour {}",
            passage.commune
        );
    }
    for commune in ["Menton", "Roquebrune-Cap-Martin", "Nice", "Villefranche-sur-Mer"] {
        let numbers: Vec<u32> = smoothed
            .iter()
            .filter(|passage| passage.commune == commune)
            .map(|passage| passage.passage)
            .collect();
        let total = smoothed
            .iter()
            .find(|passage| passage.commune == commune)
            .map(|passage| passage.nb_passages_total)
            .expect("commune présente");
        assert_eq!(numbers, (1..=total).collect::<Vec<u32>>(), "{commune}");
    }
}

#[test]
fn smoothing_zero_returns_the_server_segmentation() {
    let raw = boundary_sequence();
    assert_eq!(smooth_short_passages(&raw, 0.0), raw);
}

// ─── Garde-fou de couverture ─────────────────────────────────────────

#[test]
fn partial_coverage_uses_a_five_percent_tolerance() {
    let covering = |km: f64| vec![span("Nice", 0.0, km)];

    assert!(
        !coverage_is_partial(100.0, &covering(96.0)),
        "4 % hors communes : complet"
    );
    assert!(
        !coverage_is_partial(100.0, &covering(95.0)),
        "exactement 5 % : toléré"
    );
    assert!(
        coverage_is_partial(100.0, &covering(94.0)),
        "6 % hors communes : partiel"
    );
    assert!(coverage_is_partial(100.0, &[]), "aucune commune : partiel");
    assert!(
        !coverage_is_partial(0.0, &[]),
        "trace de longueur nulle : jamais partielle"
    );
}

/// Un passage court isolé par un trou de couverture reste en place : le lissage
/// ne comble jamais une portion hors dataset.
#[test]
fn smoothing_never_bridges_a_coverage_hole() {
    let segments = vec![
        span("Hors zone", 0.0, 0.100),
        span("Nice", 1.0, 2.0),
        span("Nice", 2.0, 2.100),
    ];

    let smoothed = smooth_short_passages(&segments, 0.5);

    assert_eq!(smoothed.len(), 2);
    assert_eq!(smoothed[0].commune, "Hors zone", "le trou n'est pas comblé");
    assert_eq!((smoothed[0].km_entree, smoothed[0].km_sortie), (0.0, 0.100));
    assert_eq!(smoothed[1].commune, "Nice");
    assert_eq!((smoothed[1].km_entree, smoothed[1].km_sortie), (1.0, 2.100));
}

/// Un passage `ambiguous` n'est ni absorbé ni cible d'absorption.
#[test]
fn smoothing_leaves_ambiguous_passages_alone() {
    let mut short = span("Monaco", 5.0, 5.040);
    short.ambiguous = true;
    let flagged = vec![span("Roquebrune-Cap-Martin", 0.0, 5.0), short, span("Roquebrune-Cap-Martin", 5.040, 10.0)];
    assert_eq!(
        smooth_short_passages(&flagged, 0.5).len(),
        3,
        "le passage signalé reste tel quel"
    );

    let mut flagged_neighbour = span("Roquebrune-Cap-Martin", 5.040, 10.0);
    flagged_neighbour.ambiguous = true;
    let neighbour = vec![
        span("Beausoleil", 0.0, 5.0),
        span("Monaco", 5.0, 5.040),
        flagged_neighbour,
    ];
    let smoothed = smooth_short_passages(&neighbour, 0.5);
    assert_eq!(smoothed.len(), 2);
    assert_eq!(
        (smoothed[0].commune.as_str(), smoothed[0].km_sortie),
        ("Beausoleil", 5.040),
        "seul le voisin non signalé reçoit l'absorption"
    );
}
