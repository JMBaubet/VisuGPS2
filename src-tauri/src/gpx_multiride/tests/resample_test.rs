//! Tests de `gpx_multiride::resample` — rééchantillonnage à pas quasi constant.
//!
//! Ils vérifient les invariants de la spécification — provenance non
//! décroissante (P-1), metricité (P-2), plafond (P-3) — et **l'espace d'index du
//! contrat** : les bornes des passages désignent des `<trkpt>` du GPX d'origine,
//! et non les points de la trace nettoyée.

use crate::gpx_multiride::projection::{build_geometry, MultirideGeom, Projector};
use crate::gpx_multiride::resample::{raw_point_number, resample, resample_capped, MAX_SAMPLES};
use crate::gpx_multiride::types::MultiridePoint;

// ─── Aides de test ────────────────────────────────────────────────────

const M_PER_DEG_LAT: f64 = 110_540.0;
const EPS: f64 = 1e-9;

fn point(lat: f64, lon: f64) -> MultiridePoint {
    MultiridePoint { lat, lon }
}

/// Latitude à `meters` au nord du point de départ des traces de test.
fn north(meters: f64) -> f64 {
    45.0 + meters / M_PER_DEG_LAT
}

/// Trace rectiligne vers le nord : `count` points espacés de `spacing_m`.
fn line_north(count: usize, spacing_m: f64) -> Vec<MultiridePoint> {
    (0..count)
        .map(|i| point(north((i as f64) * spacing_m), 2.0))
        .collect()
}

// ─── Pas effectif ─────────────────────────────────────────────────────

/// Le pas est calé sur la fin de trace : le dernier point échantillonné tombe
/// exactement sur le dernier point de la géométrie, sans reliquat.
#[test]
fn effective_step_lands_on_the_trace_end() {
    let geom = build_geometry(&line_north(5, 250.0)).unwrap();

    let r = resample(&geom, 4.0).unwrap();

    assert_eq!(r.n, 250);
    assert!((r.step - 4.0).abs() < EPS, "pas effectif = {}", r.step);
    assert_eq!(r.lat.len(), 251);
    assert!((r.cum[r.n] - geom.total).abs() < EPS);
    assert!((r.lat[r.n] - north(1000.0)).abs() < EPS, "dernier point");
}

/// Quand le pas ne divise pas la trace, le pas effectif est **recalé** pour que
/// le dernier échantillon tombe tout de même sur la fin.
#[test]
fn effective_step_is_recalibrated_when_the_trace_does_not_divide() {
    let geom = build_geometry(&line_north(5, 250.25)).unwrap();
    assert!((geom.total - 1001.0).abs() < 1e-6);

    let r = resample(&geom, 4.0).unwrap();

    assert_eq!(r.n, 250, "1001 / 4 arrondi à 250 intervalles");
    assert!(r.step > 4.0 && r.step < 4.01, "pas effectif = {}", r.step);
    assert!(
        (r.cum[r.n] - geom.total).abs() < EPS,
        "le dernier point doit tomber sur la fin de trace"
    );
}

/// P-2 (metricité) : la distance cumulée est exactement `i · pas`, ce qui
/// justifie d'exprimer les seuils de détection en nombre de points.
#[test]
fn cumulated_distance_is_the_distance_along_the_trace() {
    let geom = build_geometry(&line_north(5, 250.0)).unwrap();
    let r = resample(&geom, 7.0).unwrap();

    for (i, &cum) in r.cum.iter().enumerate() {
        let expected = (i as f64) * r.step;
        assert!((cum - expected).abs() < EPS, "cum[{i}] = {cum}");
    }
    assert!(r.cum.windows(2).all(|w| w[1] > w[0]), "cum non monotone");
}

// ─── Provenance (P-1) ─────────────────────────────────────────────────

/// P-1 : le curseur de provenance ne recule jamais, et chaque point
/// échantillonné s'appuie sur un point de la géométrie.
#[test]
fn provenance_never_goes_backwards() {
    let geom = build_geometry(&line_north(40, 30.0)).unwrap();

    let r = resample(&geom, 4.0).unwrap();

    assert_eq!(r.orig.len(), r.n + 1);
    for i in 1..r.orig.len() {
        assert!(r.orig[i] >= r.orig[i - 1], "orig non décroissant en {i}");
        assert!(r.orig[i] < geom.px.len(), "orig hors géométrie en {i}");
    }
    assert_eq!(r.orig[0], 0, "le départ est le premier point");
    assert_eq!(
        r.orig[r.n],
        geom.px.len() - 1,
        "l'arrivée est le dernier point"
    );
}

// ─── Index du contrat ─────────────────────────────────────────────────

/// Les bornes d'un passage désignent des **numéros de `<trkpt>` du GPX
/// d'origine** : dès qu'un point est écarté au dédoublonnage, la trace nettoyée
/// et le GPX ne se comptent plus de la même façon, et c'est le GPX qui fait foi
/// pour la jointure décrite en annexe 13.6.
///
/// Le pas est choisi pour que chaque échantillon tombe **exactement** sur un
/// sommet de la géométrie — cas limite où la comparaison des distances cumulées
/// doit désigner le sommet lui-même, et non le point qui le précède.
#[test]
fn raw_point_number_designates_the_original_trkpt() {
    // Le point brut 1 est un doublon du point 0 : la géométrie nettoyée ne
    // compte que trois points, là où le GPX en compte quatre.
    let points = vec![
        point(45.0, 2.0),                 // trkpt 1 — conservé
        point(north(0.01), 2.0),          // trkpt 2 — écarté (1 cm)
        point(north(100.0), 2.0),         // trkpt 3 — conservé
        point(north(200.0), 2.0),         // trkpt 4 — conservé
    ];
    let geom = build_geometry(&points).unwrap();
    assert_eq!(geom.px.len(), 3, "géométrie nettoyée");

    let r = resample(&geom, 100.0).unwrap();
    assert_eq!(r.n, 2);

    let numbers: Vec<usize> = (0..=r.n).map(|i| raw_point_number(&geom, &r, i)).collect();
    assert_eq!(
        numbers,
        vec![1, 3, 4],
        "les index doivent suivre le GPX d'origine, pas la trace nettoyée"
    );
}

// ─── Plafond (P-3) ────────────────────────────────────────────────────

/// Le plafond de points relève le pas et le signale : c'est ce drapeau que le
/// fichier de description consigne (`pas_plafonne`).
#[test]
fn cap_raises_the_step_and_flags_it() {
    let geom = build_geometry(&line_north(11, 100.0)).unwrap();

    let capped = resample_capped(&geom, 0.5, 100).unwrap();

    assert!(capped.capped, "le pas doit être signalé comme relevé");
    assert_eq!(capped.n, 100);
    assert!((capped.step - 10.0).abs() < EPS, "pas effectif = {}", capped.step);
    assert!((capped.cum[capped.n] - geom.total).abs() < EPS);

    // Sous le plafond, le pas demandé est conservé et rien n'est signalé.
    let plain = resample(&geom, 10.0).unwrap();
    assert!(!plain.capped);
    assert_eq!(plain.n, 100);
}

/// Le plafond de la spécification est de 120 000 points.
#[test]
fn max_samples_follows_the_specification() {
    assert_eq!(MAX_SAMPLES, 120_000);
    let geom = build_geometry(&line_north(5, 250.0)).unwrap();
    assert!(resample(&geom, 2.0).unwrap().n <= MAX_SAMPLES);
}

// ─── Entrées invalides ────────────────────────────────────────────────

/// Un pas nul ou négatif est refusé : il rendrait le nombre d'intervalles
/// infini. La trace de longueur nulle est refusée de même — branche défensive,
/// la géométrie garantissant déjà deux points distincts.
#[test]
fn resampling_rejects_invalid_input() {
    let geom = build_geometry(&line_north(5, 250.0)).unwrap();

    for step in [0.0, -4.0] {
        let error = resample(&geom, step).unwrap_err();
        assert!(error.contains("invalide"), "message : {error}");
    }

    let zero_cap = resample_capped(&geom, 4.0, 0).unwrap_err();
    assert!(zero_cap.contains("Plafond"), "message : {zero_cap}");

    let degenerate = MultirideGeom {
        proj: Projector::new(&point(45.0, 2.0)),
        px: vec![0.0, 0.0],
        py: vec![0.0, 0.0],
        cum: vec![0.0, 0.0],
        total: 0.0,
        raw_index: vec![0, 1],
    };
    let error = resample(&degenerate, 4.0).unwrap_err();
    assert!(error.contains("longueur nulle"), "message : {error}");
}
