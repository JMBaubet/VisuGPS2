//! Tests du pipeline de détection (`gpx_communes::pipeline`).
//!
//! Les communes sont construites en rectangles : la géométrie attendue des
//! passages s'en déduit exactement, sans dépendre d'un extrait OSM réel.

use crate::gpx_communes::osm::{OsmCommune, OsmPolygon};
use crate::gpx_communes::pipeline::{detect_passages, fill_gaps};
use crate::gpx_communes::types::{CommuneParams, CommunePassage};

// ─── Aides de test ────────────────────────────────────────────────────

/// Commune rectangulaire : `[lon_min, lon_max] × [lat_min, lat_max]`.
fn rect(name: &str, lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64) -> OsmCommune {
    OsmCommune {
        name: name.to_string(),
        polygons: vec![ring(lon_min, lon_max, lat_min, lat_max, Vec::new())],
    }
}

/// Commune rectangulaire percée d'un trou (enclave d'une autre commune).
fn rect_with_hole(
    name: &str,
    outer: [f64; 4],
    hole: [f64; 4],
) -> OsmCommune {
    OsmCommune {
        name: name.to_string(),
        polygons: vec![ring(
            outer[0],
            outer[1],
            outer[2],
            outer[3],
            vec![ring_ring(hole[0], hole[1], hole[2], hole[3])],
        )],
    }
}

/// Polygone `[lon_min, lon_max] × [lat_min, lat_max]`, avec ses trous.
fn ring(
    lon_min: f64,
    lon_max: f64,
    lat_min: f64,
    lat_max: f64,
    inners: Vec<Vec<[f64; 2]>>,
) -> OsmPolygon {
    OsmPolygon {
        outer: ring_ring(lon_min, lon_max, lat_min, lat_max),
        inners,
    }
}

/// Anneau fermé (sens horaire, quatre coins).
fn ring_ring(lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64) -> Vec<[f64; 2]> {
    vec![
        [lon_min, lat_min],
        [lon_max, lat_min],
        [lon_max, lat_max],
        [lon_min, lat_max],
        [lon_min, lat_min],
    ]
}

/// Trace rectiligne ouest → est à latitude constante, `count` points.
fn straight(lon_start: f64, lon_end: f64, lat: f64, count: usize) -> Vec<[f64; 2]> {
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64;
            [lon_start + (lon_end - lon_start) * t, lat]
        })
        .collect()
}

/// Noms des passages, dans l'ordre chronologique.
fn names(passages: &[CommunePassage]) -> Vec<&str> {
    passages.iter().map(|p| p.commune.as_str()).collect()
}

/// Longueur attendue d'un segment `[lon_a, lon_b]` à latitude constante (km).
fn expected_km(lon_a: f64, lon_b: f64, lat: f64) -> f64 {
    crate::gpx_communes::geometry::haversine_m([lon_a, lat], [lon_b, lat]) / 1000.0
}

// ─── Passages ─────────────────────────────────────────────────────────

/// Trois communes contiguës traversées de bout en bout : trois passages
/// contigus, la sortie de l'une étant l'entrée de la suivante.
#[test]
fn three_communes_in_a_row_yield_three_contiguous_passages() {
    let trace = straight(2.001, 2.029, 45.0, 200);
    let communes = vec![
        rect("A", 2.000, 2.010, 44.9, 45.1),
        rect("B", 2.010, 2.020, 44.9, 45.1),
        rect("C", 2.020, 2.030, 44.9, 45.1),
    ];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert_eq!(names(&passages), ["A", "B", "C"]);
    assert!(passages.iter().all(|p| p.nb_passages_total == 1));
    assert_eq!(passages[0].km_entree, 0.0, "la trace démarre dans A");

    // Bornes contiguës : la sortie d'une commune est l'entrée de la suivante
    // (à l'arrondi au mètre près).
    for pair in passages.windows(2) {
        assert!(
            (pair[0].km_sortie - pair[1].km_entree).abs() <= 0.001,
            "discontinuité entre {} et {} : {} ≠ {}",
            pair[0].commune,
            pair[1].commune,
            pair[0].km_sortie,
            pair[1].km_entree
        );
    }

    // La chaîne couvre la trace entière : la dernière sortie est l'arrivée.
    let total = expected_km(2.001, 2.029, 45.0);
    let last = passages.last().expect("au moins un passage").km_sortie;
    assert!(
        (last - total).abs() < 0.01,
        "la couverture s'arrête à {last} km pour une trace de {total} km"
    );

    // Longueurs conformes à la géométrie des rectangles.
    assert!((passages[0].longueur_km - expected_km(2.001, 2.010, 45.0)).abs() < 0.01);
    assert!((passages[1].longueur_km - expected_km(2.010, 2.020, 45.0)).abs() < 0.01);
}

/// Aller-retour : une commune traversée deux fois donne **deux** passages
/// numérotés, sans recouvrement, et la commune intermédiaire reste d'un tenant.
#[test]
fn an_out_and_back_yields_two_numbered_passages() {
    let out = straight(2.001, 2.025, 45.0, 150);
    let mut trace = out.clone();
    // Retour par le même chemin, sans dupliquer le point de rebroussement.
    trace.extend(out.iter().rev().skip(1).copied());

    let communes = vec![
        rect("A", 1.995, 2.010, 44.9, 45.1),
        rect("B", 2.010, 2.030, 44.9, 45.1),
    ];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert_eq!(names(&passages), ["A", "B", "A"]);
    assert_eq!(passages[0].passage, 1);
    assert_eq!(passages[0].nb_passages_total, 2);
    assert_eq!(passages[1].nb_passages_total, 1);
    assert_eq!(passages[2].passage, 2);
    assert!(
        passages[0].km_sortie <= passages[1].km_entree,
        "les passages de A ne se recouvrent pas"
    );

    // Invariants du fichier : km d'entrée croissants, longueur = sortie − entrée.
    assert!(passages.windows(2).all(|w| w[0].km_entree < w[1].km_entree));
    for passage in &passages {
        assert!((passage.longueur_km - (passage.km_sortie - passage.km_entree)).abs() < 1e-9);
        assert!(passage.longueur_km > 0.0);
        assert!(passage.passage >= 1 && passage.passage <= passage.nb_passages_total);
    }
}

/// Un passage plus court que le seuil d'absorption est absorbé par son voisin le
/// plus long — ici C — dont l'entrée recule jusqu'à la frontière précédente.
#[test]
fn a_short_passage_is_absorbed_into_its_longest_neighbour() {
    // B est large de ~200 m (0.00254° à 45° de latitude) : sous le seuil de 500 m.
    let trace = straight(2.001, 2.029, 45.0, 300);
    let communes = vec![
        rect("A", 2.000, 2.010, 44.9, 45.1),
        rect("B", 2.010, 2.01254, 44.9, 45.1),
        rect("C", 2.01254, 2.030, 44.9, 45.1),
    ];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert_eq!(names(&passages), ["A", "C"], "B, trop courte, est absorbée");
    assert!(
        (passages[1].km_entree - passages[0].km_sortie).abs() <= 0.001,
        "C hérite de la surface de B : son entrée est la sortie de A"
    );
    // La longueur totale reste celle de la trace : la surface absorbée a été
    // réattribuée, non perdue.
    let total = expected_km(2.001, 2.029, 45.0);
    let covered: f64 = passages.iter().map(|p| p.longueur_km).sum();
    assert!((covered - total).abs() < 0.01);
}

/// Un trou étroit (moins d'un pas d'échantillonnage) ne scinde pas le passage :
/// l'échantillon isolé est comblé, la commune reste traversée d'un tenant.
#[test]
fn a_narrow_hole_does_not_split_the_passage() {
    let trace = straight(2.001, 2.029, 45.0, 300);
    // Trou de ~60 m, traversé par la trace.
    let communes = vec![rect_with_hole(
        "A",
        [2.000, 2.030, 44.9, 45.1],
        [2.0145, 2.01526, 44.99, 45.01],
    )];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert_eq!(names(&passages), ["A"]);
    assert_eq!(passages[0].nb_passages_total, 1);
    assert_eq!(passages[0].km_entree, 0.0);
}

/// Polygone enveloppant : la commune dont le centroïde est le plus proche gagne
/// le chevauchement, donc la commune locale au centre de la trace — et
/// l'enveloppe ne couvre pas toute la trace.
#[test]
fn a_wrapping_polygon_loses_to_the_local_commune() {
    let trace = straight(2.000, 2.040, 45.0, 300);
    let communes = vec![
        rect("Enveloppe", 2.000, 2.100, 44.9, 45.1),
        rect("Locale", 2.020, 2.030, 44.9, 45.1),
    ];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert_eq!(names(&passages), ["Enveloppe", "Locale", "Enveloppe"]);
    assert_eq!(passages[1].nb_passages_total, 1, "Locale n'est traversée qu'une fois");
    assert_eq!(passages[0].nb_passages_total, 2, "l'enveloppe l'est deux fois");
}

/// Aucune commune connue le long de la trace : résultat **vide**, pas une erreur.
#[test]
fn a_trace_outside_every_commune_yields_no_passage() {
    let trace = straight(2.001, 2.029, 45.0, 50);
    let communes = vec![rect("Loin", 3.000, 3.100, 44.9, 45.1)];

    let passages = detect_passages(&trace, &communes, &CommuneParams::default())
        .expect("la détection doit aboutir");

    assert!(passages.is_empty());
}

/// Trace dégénérée : refusée (aucune géométrie à analyser).
#[test]
fn a_degenerate_trace_is_rejected() {
    let err = detect_passages(&[[2.0, 45.0]], &[], &CommuneParams::default())
        .expect_err("un point unique doit être refusé");
    assert!(err.contains("dégénérée"), "message inattendu : {err}");
}

// ─── Comblement des trous ─────────────────────────────────────────────

/// Le comblement ne rattrape qu'un trou d'**un** échantillon flanqué de la même
/// commune des deux côtés.
#[test]
fn fill_gaps_bridges_only_single_holes_of_the_same_commune() {
    // Comblé : un échantillon manquant entre la même commune.
    let mut timeline = vec![Some(0), None, Some(0)];
    fill_gaps(&mut timeline, 1);
    assert_eq!(timeline, vec![Some(0), Some(0), Some(0)]);

    // Conservé : les flancs sont deux communes différentes (vraie transition).
    let mut timeline = vec![Some(0), None, Some(1)];
    fill_gaps(&mut timeline, 1);
    assert_eq!(timeline, vec![Some(0), None, Some(1)]);

    // Conservé : le trou dépasse le seuil.
    let mut timeline = vec![Some(0), None, None, Some(0)];
    fill_gaps(&mut timeline, 1);
    assert_eq!(timeline, vec![Some(0), None, None, Some(0)]);

    // Conservé : trou en bord de timeline (pas de flanc des deux côtés).
    let mut timeline = vec![None, Some(0)];
    fill_gaps(&mut timeline, 1);
    assert_eq!(timeline, vec![None, Some(0)]);

    // Seuil 0 : comblement désactivé.
    let mut timeline = vec![Some(0), None, Some(0)];
    fill_gaps(&mut timeline, 0);
    assert_eq!(timeline, vec![Some(0), None, Some(0)]);
}
