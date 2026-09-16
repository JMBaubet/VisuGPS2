//! Tests de `gpx_multiride::direction` — qualification du sens d'un passage.
//!
//! Le sens se mesure entre deux passages **superposés** : les tests s'appuient
//! sur une trace en serpentin de trois passages parallèles, dont les intervalles
//! se calculent à la main (0→30 : aller, 30→60 : retour, 60→90 : aller, à
//! 10 m de pas).

use crate::gpx_multiride::direction::relative_direction;
use crate::gpx_multiride::resample::Resample;

use super::sampled_trace;

/// Tolérance de superposition des tests (m) — non multiple du pas
/// d'échantillonnage, comme dans `runs_test` : les appariements des passages
/// superposés sont alors à distance nulle, sans ambiguïté de frontière.
const TOL: f64 = 6.0;

/// Serpentin de trois passages : aller (0→300 m), retour (300→0), aller
/// (0→300 m).
fn serpentine() -> Resample {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0), (0.0, 300.0)], 10.0);
    r
}

/// Deux passages parcourus dans le même sens sont qualifiés `+1` (Aller).
#[test]
fn passages_in_the_same_direction_are_qualified_as_aller() {
    let r = serpentine();

    assert_eq!(relative_direction(&r, (0, 30), (60, 90), TOL), 1);
}

/// Un passage parcouru en sens inverse est qualifié `−1` (Retour).
#[test]
fn an_opposite_passage_is_qualified_as_retour() {
    let r = serpentine();

    assert_eq!(relative_direction(&r, (0, 30), (30, 60), TOL), -1);
}

/// Une fenêtre resserrée **à cheval sur le demi-tour** reste qualifiée `−1` : le
/// verdict est un cumul de `cos(Δcap)` sur toute la portion, et non le signe du
/// cap local — lequel bascule, lui, de part et d'autre du rebroussement.
#[test]
fn a_window_straddling_the_turn_is_still_qualified_as_retour() {
    let r = serpentine();
    // 24→30 : fin de l'aller, dont le cap mesuré trois points plus loin a déjà
    // franchi le demi-tour ; 30→36 : le début du retour.
    let (first, other) = ((24usize, 30usize), (30usize, 36usize));

    assert_eq!(
        relative_direction(&r, first, other, TOL),
        -1,
        "la portion qui suit le demi-tour repart en sens inverse"
    );
}
