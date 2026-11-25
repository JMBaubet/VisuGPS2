//! Tests du calcul de surface utilisé par la validation des polygones.

use serde_json::{json, Value};

use crate::hermes::commands::{dataset_surface_km2, polygon_outer_area_km2};
use crate::hermes::types::HermesPolygon;

fn square() -> Value {
    json!({
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0]
        ]]
    })
}

fn polygon(id: &str, geometry: Value) -> HermesPolygon {
    HermesPolygon {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        color: None,
        geometry,
        bbox: [0.0, 0.0, 1.0, 1.0],
        created_at: String::new(),
        updated_at: String::new(),
        revision: 1,
    }
}

#[test]
fn square_of_one_degree_at_equator_is_about_twelve_thousand_km2() {
    let geometry = json!({
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0]
        ]]
    });

    let area = polygon_outer_area_km2(&geometry).expect("surface calculable");
    assert!(
        (12_000.0..=12_600.0).contains(&area),
        "surface inattendue : {area}"
    );
}

#[test]
fn open_ring_is_closed_before_computing_area() {
    let open = json!({
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0]
        ]]
    });
    let closed = json!({
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0]
        ]]
    });

    let open_area = polygon_outer_area_km2(&open).expect("anneau refermé");
    let closed_area = polygon_outer_area_km2(&closed).expect("anneau fermé");
    assert!((open_area - closed_area).abs() < 1e-6);
}

#[test]
fn invalid_geometries_are_rejected_with_invalid_code() {
    let not_a_polygon = json!({
        "type": "LineString",
        "coordinates": [[0.0, 0.0], [1.0, 1.0]]
    });
    let error = polygon_outer_area_km2(&not_a_polygon).expect_err("pas de coordonnées");
    assert!(error.starts_with("HERMES_INVALID: "), "erreur : {error}");

    let too_few_vertices = json!({
        "type": "Polygon",
        "coordinates": [[[0.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]
    });
    let error = polygon_outer_area_km2(&too_few_vertices).expect_err("trop peu de sommets");
    assert!(error.starts_with("HERMES_INVALID: "), "erreur : {error}");

    let out_of_range = json!({
        "type": "Polygon",
        "coordinates": [[
            [0.0, 0.0],
            [200.0, 0.0],
            [200.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0]
        ]]
    });
    let error = polygon_outer_area_km2(&out_of_range).expect_err("longitude hors bornes");
    assert!(error.starts_with("HERMES_INVALID: "), "erreur : {error}");
}

#[test]
fn dataset_surface_sums_selected_zones_only() {
    let unit_area = polygon_outer_area_km2(&square()).expect("surface unitaire");

    let total = dataset_surface_km2(
        &["a".to_string(), "b".to_string()],
        &[polygon("a", square()), polygon("b", square())],
    )
    .expect("somme des zones");
    assert!((total - 2.0 * unit_area).abs() < 1e-6);

    let partial = dataset_surface_km2(
        &["a".to_string(), "inconnu".to_string()],
        &[polygon("a", square())],
    )
    .expect("zone inconnue ignorée");
    assert!((partial - unit_area).abs() < 1e-6);
}
