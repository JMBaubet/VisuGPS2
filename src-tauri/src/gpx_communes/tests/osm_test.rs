//! Tests de la lecture de la réponse Overpass (`gpx_communes::osm`).

use serde_json::json;

use crate::gpx_communes::osm::parse_communes;

// ─── Aides de test ────────────────────────────────────────────────────

/// Way membre d'une relation, géométrie inline comme la renvoie `out geom`.
fn way(role: &str, coords: &[(f64, f64)]) -> serde_json::Value {
    json!({
        "type": "way",
        "ref": 1,
        "role": role,
        "geometry": coords
            .iter()
            .map(|(lat, lon)| json!({ "lat": lat, "lon": lon }))
            .collect::<Vec<_>>(),
    })
}

/// Relation de limite administrative nommée.
fn relation(name: &str, admin_level: &str, members: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "type": "relation",
        "id": 100,
        "tags": {
            "boundary": "administrative",
            "admin_level": admin_level,
            "name": name,
        },
        "members": members,
    })
}

/// Sérialise la liste d'éléments OSM, telle que le front la transmet.
fn payload(elements: Vec<serde_json::Value>) -> String {
    serde_json::to_string(&elements).expect("sérialisation du payload de test")
}

// ─── Assemblage ───────────────────────────────────────────────────────

/// Une relation dont les ways extérieurs forment un carré est assemblée en un
/// anneau fermé unique.
#[test]
fn a_relation_is_assembled_from_its_way_members() {
    let json = payload(vec![relation(
        "Carré",
        "8",
        vec![
            way("outer", &[(44.9, 2.0), (44.9, 2.1)]),
            way("outer", &[(44.9, 2.1), (45.1, 2.1)]),
            way("outer", &[(45.1, 2.1), (45.1, 2.0)]),
            way("outer", &[(45.1, 2.0), (44.9, 2.0)]),
        ],
    )]);

    let (communes, stats) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert_eq!(stats.elements, 1);
    assert_eq!(stats.relations, 1);
    assert_eq!(stats.rings, 1);
    assert_eq!(stats.forced_closures, 0);
    assert_eq!(communes.len(), 1);
    assert_eq!(communes[0].name, "Carré");
    let outer = &communes[0].polygons[0].outer;
    assert_eq!(outer.len(), 5, "quatre sommets et le retour au départ");
    assert_eq!(outer.first(), outer.last(), "l'anneau est fermé");
}

/// L'assemblage ne dépend ni de l'ordre des ways membres ni de leur orientation
/// (un way membre peut être parcouru dans les deux sens).
#[test]
fn rings_are_assembled_whatever_the_order_and_orientation() {
    let json = payload(vec![relation(
        "Carré",
        "8",
        vec![
            // Mêmes arêtes que le test précédent, désordonnées et inversées.
            way("outer", &[(45.1, 2.1), (44.9, 2.1)]),
            way("outer", &[(45.1, 2.0), (45.1, 2.1)]),
            way("outer", &[(44.9, 2.0), (44.9, 2.1)]),
            way("outer", &[(44.9, 2.0), (45.1, 2.0)]),
        ],
    )]);

    let (communes, stats) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert_eq!(stats.forced_closures, 0);
    assert_eq!(communes.len(), 1);
    assert_eq!(communes[0].polygons[0].outer.len(), 5);
}

/// Un membre en rôle vide ou `outer` est traité comme un extérieur — comme le
/// fait `osmtogeojson`.
#[test]
fn an_empty_role_is_treated_as_an_outer_ring() {
    let json = payload(vec![relation(
        "Carré",
        "8",
        vec![
            way("", &[(44.9, 2.0), (44.9, 2.1)]),
            way("outer", &[(44.9, 2.1), (45.1, 2.1)]),
            way("outer", &[(45.1, 2.1), (45.1, 2.0)]),
            way("outer", &[(45.1, 2.0), (44.9, 2.0)]),
        ],
    )]);

    let (communes, _) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert_eq!(communes[0].polygons.len(), 1);
    assert_eq!(communes[0].polygons[0].outer.len(), 5);
}

/// Frontière incomplète : plutôt que de perdre la commune, l'anneau est refermé
/// de force et le diagnostic est reporté.
#[test]
fn an_unclosed_ring_is_closed_by_force_and_reported() {
    let json = payload(vec![relation(
        "Ouverte",
        "8",
        vec![
            way("outer", &[(44.9, 2.0), (44.9, 2.1)]),
            way("outer", &[(44.9, 2.1), (45.1, 2.1)]),
            way("outer", &[(45.1, 2.1), (45.1, 2.0)]),
        ],
    )]);

    let (communes, stats) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert_eq!(stats.forced_closures, 1);
    assert_eq!(communes.len(), 1, "la commune est conservée");
    let outer = &communes[0].polygons[0].outer;
    assert_eq!(outer.first(), outer.last(), "l'anneau est refermé");
}

/// Un membre `inner` devient un trou de l'anneau extérieur qui le contient.
#[test]
fn an_inner_member_becomes_a_hole() {
    let json = payload(vec![relation(
        "Trouée",
        "8",
        vec![
            way("outer", &[(44.9, 2.0), (44.9, 2.1)]),
            way("outer", &[(44.9, 2.1), (45.1, 2.1)]),
            way("outer", &[(45.1, 2.1), (45.1, 2.0)]),
            way("outer", &[(45.1, 2.0), (44.9, 2.0)]),
            way("inner", &[(45.0, 2.04), (45.0, 2.06)]),
            way("inner", &[(45.0, 2.06), (45.02, 2.06)]),
            way("inner", &[(45.02, 2.06), (45.02, 2.04)]),
            way("inner", &[(45.02, 2.04), (45.0, 2.04)]),
        ],
    )]);

    let (communes, stats) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert_eq!(stats.orphan_rings, 0);
    assert_eq!(communes[0].polygons.len(), 1);
    assert_eq!(communes[0].polygons[0].inners.len(), 1, "le trou est rattaché");
    assert!(
        communes[0].polygons[0].inners[0].len() >= 4,
        "le trou est un anneau fermé"
    );
}

/// Seules les relations de limite administrative du niveau visé, nommées et
/// pourvues de géométrie, sont retenues.
#[test]
fn elements_that_are_not_named_communes_are_ignored() {
    let json = payload(vec![
        json!({ "type": "way", "id": 1, "tags": { "name": "Rue de la Paix" } }),
        json!({
            "type": "relation", "id": 2,
            "tags": { "boundary": "administrative", "admin_level": "6", "name": "Département" },
            "members": [],
        }),
        json!({
            "type": "relation", "id": 3,
            "tags": { "boundary": "administrative", "admin_level": "8" },
            "members": [],
        }),
        json!({
            "type": "relation", "id": 4,
            "tags": { "boundary": "administrative", "admin_level": "8", "name": "   " },
            "members": [],
        }),
        json!({
            "type": "relation", "id": 5,
            "tags": { "boundary": "administrative", "admin_level": "8", "name": "Sans géométrie" },
            "members": [],
        }),
    ]);

    let (communes, stats) = parse_communes(&json, "8").expect("lecture de la réponse");
    assert!(communes.is_empty());
    assert_eq!(stats.elements, 5);
    assert_eq!(stats.relations, 0);
}

/// Une réponse illisible est une erreur explicite (le front la remonte).
#[test]
fn an_unreadable_payload_is_an_error() {
    let err = parse_communes("pas du JSON", "8").expect_err("la lecture doit échouer");
    assert!(err.contains("Overpass"), "message inattendu : {err}");
}
