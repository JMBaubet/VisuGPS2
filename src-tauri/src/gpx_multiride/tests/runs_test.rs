//! Tests de `gpx_multiride::runs` — correspondances ponctuelles et chaînage.
//!
//! Les traces de test sont décrites par leurs sommets en mètres (cf.
//! `sampled_trace`), ce qui permet d'affirmer des intervalles **exacts** plutôt
//! que des ordres de grandeur : c'est la condition pour qu'une régression du
//! chaînage soit visible.

use crate::gpx_multiride::runs::{find_runs, Run};

use super::sampled_trace;

/// Tolérance de superposition des tests (m).
///
/// Volontairement **non multiple du pas d'échantillonnage** (10 m) : deux
/// échantillons d'un aller-retour séparés d'un pas se trouveraient sinon
/// exactement à la distance `tol`, et leur appariement dépendrait d'un arrondi
/// flottant — ils sont ici franchement dedans (0 m) ou franchement dehors (10 m
/// contre 6 m), sans zone d'incertitude.
const TOL: f64 = 6.0;
/// Longueur minimale des tests (m).
const MIN_LEN: f64 = 50.0;

/// Une trace sans superposition ne produit aucun run.
#[test]
fn a_straight_trace_has_no_run() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 1000.0)], 10.0);

    assert!(find_runs(&r, TOL, MIN_LEN).unwrap().is_empty());
}

/// Un aller-retour franc produit **un** run de sens opposé, dont les deux
/// intervalles sont séparés par la moitié de la longueur minimale (filtre de
/// séparation temporelle).
#[test]
fn an_out_and_back_produces_one_opposite_run() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0)], 10.0);

    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    assert_eq!(
        runs,
        vec![Run {
            a0: 0,
            a1: 28,
            b0: 32,
            b1: 60,
            dir: -1,
        }],
        "l'aller et son retour se font face, à la séparation temporelle près"
    );
}

/// Le recoupement d'une trace avec elle-même à angle droit n'est **pas** un
/// passage multiple : le filtre d'alignement des tangentes l'écarte.
#[test]
fn a_perpendicular_crossing_is_not_a_run() {
    // Le seul recoupement de cette trace est le croisement à angle droit de la
    // dernière branche avec le point de départ de la première.
    let (_, r) = sampled_trace(
        &[
            (-100.0, 0.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (-100.0, 100.0),
            (-100.0, -100.0),
        ],
        10.0,
    );

    assert!(
        find_runs(&r, TOL, MIN_LEN).unwrap().is_empty(),
        "un croisement perpendiculaire n'est pas un passage multiple"
    );
}

/// Une répétition plus courte que la longueur minimale n'est pas signalée : la
/// séparation temporelle exigée dépasse alors la trace elle-même (cas d'usage
/// UC-4 — le réglage de la longueur minimale fait disparaître les micro-segments).
#[test]
fn a_repetition_shorter_than_the_minimum_length_is_not_reported() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 60.0), (0.0, 0.0)], 10.0);

    assert!(
        find_runs(&r, TOL, 500.0).unwrap().is_empty(),
        "60 m de répétition sous un seuil de 500 m : rien à signaler"
    );

    let runs = find_runs(&r, TOL, 20.0).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].dir, -1);
    assert!(
        runs[0].a1 < runs[0].b0,
        "les deux passages d'un aller-retour sont disjoints"
    );
}

/// Trois passages parallèles produisent des runs des **deux sens**, dans un ordre
/// stable : le parcours de la table de correspondances ne doit pas dépendre de
/// l'ordre d'itération d'une table de hachage.
#[test]
fn a_serpentine_yields_both_directions_with_a_stable_order() {
    // Aller (0→300 m), retour (300→0), aller (0→300).
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0), (0.0, 300.0)], 10.0);

    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();
    let again = find_runs(&r, TOL, MIN_LEN).unwrap();

    assert_eq!(runs, again, "deux exécutions doivent rendre le même résultat");
    assert!(
        runs.iter().any(|run| run.dir == 1),
        "le double passage dans le même sens doit produire un run de même sens"
    );
    assert!(
        runs.iter().any(|run| run.dir == -1),
        "les passages en sens inverse produisent des runs opposés"
    );
    assert!(runs.iter().all(|run| run.a0 <= run.a1 && run.b0 < run.b1));
    assert!(
        runs.windows(2)
            .all(|w| (w[0].a0, w[0].b0) <= (w[1].a0, w[1].b0)),
        "les runs sont rendus triés : {runs:?}"
    );
}

/// Une tolérance ou une longueur minimale nulle est refusée : l'une rendrait la
/// grille inutilisable, l'autre autoriserait n'importe quelle correspondance.
#[test]
fn invalid_parameters_are_rejected() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0)], 10.0);

    let tolerance = find_runs(&r, 0.0, MIN_LEN).unwrap_err();
    assert!(tolerance.contains("Tolérance"), "message : {tolerance}");

    let length = find_runs(&r, TOL, 0.0).unwrap_err();
    assert!(length.contains("Longueur minimale"), "message : {length}");
}
