//! Tests de `gpx_multiride::segments` — assemblage des runs en segments.
//!
//! Ils portent d'abord sur les prédicats et les étapes de fusion, éprouvés sur des
//! intervalles construits à la main : c'est là que se jouent les invariants qui
//! protègent un aller-retour. Vient ensuite l'assemblage complet sur des traces
//! synthétiques, dont les intervalles attendus sont calculables.

use crate::gpx_multiride::resample::Resample;
use crate::gpx_multiride::runs::find_runs;
use crate::gpx_multiride::segments::{
    boundary_between_passes, build_segments, merge_ordered_passages, merge_within_segment,
    Interval, Passage,
};

use super::sampled_trace;

/// Tolérance de superposition des tests (m) — non multiple du pas, comme dans
/// `runs_test` : les appariements sont alors franchement dedans ou dehors.
const TOL: f64 = 6.0;
/// Longueur minimale des tests (m).
const MIN_LEN: f64 = 50.0;
/// Distance de fusion des tests (m).
const FUSE: f64 = 100.0;

/// Emprunt de test, réduit à ses bornes et à son sens.
fn passage(s: usize, e: usize, rel: i8) -> Passage {
    Passage {
        s,
        e,
        rel,
        km0: 0.0,
        km1: 0.0,
        pt0: 0,
        pt1: 0,
    }
}

// ─── Étape 1 : fusion ordonnée ────────────────────────────────────────

/// Le **contact simple** (`s == e` d'un intervalle à l'autre) ne fusionne pas :
/// c'est ce qui préserve la structure aller/retour au demi-tour d'un col. Seul un
/// recouvrement interne strict fusionne.
#[test]
fn a_simple_contact_is_not_an_overlap() {
    let intervals = [
        Interval { s: 0, e: 10 },
        Interval { s: 10, e: 20 },
        Interval { s: 15, e: 25 },
    ];

    let merged = merge_ordered_passages(&intervals);

    assert_eq!(
        merged,
        vec![Interval { s: 0, e: 10 }, Interval { s: 10, e: 25 }],
        "le contact simple sépare, le recouvrement strict unit"
    );
}

// ─── Étape 3 : Passe D ────────────────────────────────────────────────

/// La frontière entre deux **sens** est intangible : même contigus et séparés
/// d'un trou très court, un aller et son retour ne fusionnent jamais.
#[test]
fn the_frontier_between_directions_is_never_bridged() {
    let passages = [passage(0, 10, 0), passage(12, 20, -1)];

    let merged = merge_within_segment(&passages, 50);

    assert_eq!(merged.len(), 2);
}

/// Deux fragments **contigus de même sens** séparés d'un trou court sont recousus
/// — c'est l'objet même de la Passe D (virages serrés, tunnels, croisements
/// rejetés à l'appariement).
#[test]
fn a_short_gap_in_the_same_direction_is_bridged() {
    let passages = [passage(0, 10, -1), passage(12, 20, -1)];

    let merged = merge_within_segment(&passages, 50);

    assert_eq!(merged.len(), 1);
    assert_eq!((merged[0].s, merged[0].e), (0, 20), "les bornes sont étendues");
}

/// Un trou plus long que le seuil n'est pas comblé : la Passe D ne ponte pas
/// deux portions réellement distinctes.
#[test]
fn a_long_gap_is_not_bridged() {
    let passages = [passage(0, 10, -1), passage(100, 120, -1)];

    assert_eq!(merge_within_segment(&passages, 50).len(), 2);
}

/// La Passe D n'opère que sur des passages **contigus** : elle ne peut donc pas
/// créer d'intervalle chevauchant un passage tiers.
#[test]
fn the_passe_d_never_creates_an_overlap() {
    let passages = [passage(0, 15, -1), passage(10, 20, -1)];

    assert_eq!(merge_within_segment(&passages, 50).len(), 2);
}

// ─── Frontière entre passes ───────────────────────────────────────────

/// Un demi-tour porte la signature d'une frontière : les caps s'opposent de part
/// et d'autre de la jonction.
#[test]
fn a_turnaround_is_a_boundary() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0)], 10.0);

    // Fin de la montée (26→30) et début de la descente (30→34).
    let is_boundary = boundary_between_passes(
        &r,
        &Interval { s: 26, e: 30 },
        &Interval { s: 30, e: 34 },
        10.0,
    );

    assert!(is_boundary, "un demi-tour n'est pas une fente à recoudre");
}

/// Une continuation ne la porte pas : les deux fragments décrivent le même
/// emprunt et doivent être recousus.
#[test]
fn a_continuation_is_not_a_boundary() {
    let (_, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0)], 10.0);

    let is_boundary = boundary_between_passes(
        &r,
        &Interval { s: 0, e: 10 },
        &Interval { s: 10, e: 20 },
        10.0,
    );

    assert!(!is_boundary, "deux fragments du même emprunt se recousent");
}

// ─── Assemblage complet ───────────────────────────────────────────────

/// Un aller-retour franc produit **un** segment — une référence et son retour —,
/// et cela **quel que soit le réglage de fusion** : c'est l'invariant qui
/// protège un aller-retour d'un curseur de fusion trop haut (le bug v2 de la
/// spécification, corrigé par le test de frontière, insensible à ce réglage).
#[test]
fn an_out_and_back_yields_one_segment_whatever_the_fusion() {
    let (geom, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0)], 10.0);
    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    for fuse in [0.0, FUSE, 500.0] {
        let segments = build_segments(&runs, &r, &geom, TOL, fuse);

        assert_eq!(segments.len(), 1, "fusion = {fuse}");
        let passages = &segments[0].passages;
        assert_eq!(passages.len(), 2, "fusion = {fuse} : {passages:?}");
        assert_eq!(passages[0].rel, 0, "le premier emprunt est la référence");
        assert_eq!(passages[1].rel, -1, "le second repart en sens inverse");
        assert!(passages[0].s < passages[1].s);
        assert!(
            (passages[0].km1 - passages[0].km0 - 0.28).abs() < 0.02,
            "référence de 280 m attendue, obtenue {} km",
            passages[0].km1 - passages[0].km0
        );
    }
}

/// Une trace sans superposition ne produit aucun segment.
#[test]
fn a_straight_trace_yields_no_segment() {
    let (geom, r) = sampled_trace(&[(0.0, 0.0), (0.0, 1000.0)], 10.0);
    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    assert!(build_segments(&runs, &r, &geom, TOL, FUSE).is_empty());
}

/// Trois passages parallèles forment **un** segment de trois emprunts : la
/// référence, son retour, puis un second aller — les trois se recoupant le long
/// de la trace. Le résultat est stable d'une exécution à l'autre.
#[test]
fn a_serpentine_yields_one_segment_with_three_passages() {
    // Aller (0→300 m), retour (300→0), aller (0→300).
    let (geom, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0), (0.0, 300.0)], 10.0);
    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    let segments = build_segments(&runs, &r, &geom, TOL, FUSE);
    let again = build_segments(&runs, &r, &geom, TOL, FUSE);

    assert_eq!(segments, again, "deux exécutions doivent rendre le même résultat");
    assert_eq!(segments.len(), 1);
    let rels: Vec<i8> = segments[0].passages.iter().map(|p| p.rel).collect();
    assert_eq!(
        rels,
        vec![0, -1, 1],
        "référence, retour, puis aller dans le même sens qu'elle"
    );
    for passage in &segments[0].passages {
        assert!(passage.s < passage.e, "intervalle non dégénéré");
        // Les numéros de points GPX sont non décroissants. L'égalité est
        // légitime : la géométrie de ces traces de test ne compte que quelques
        // sommets, donc plusieurs points échantillonnés proviennent du même
        // point source.
        assert!(
            passage.pt0 <= passage.pt1,
            "bornes de points incohérentes : {passage:?}"
        );
    }
}

/// Les emprunts d'un segment se suivent le long de la trace : l'ordre des
/// passages est chronologique.
#[test]
fn passages_of_a_segment_are_chronological() {
    let (geom, r) = sampled_trace(&[(0.0, 0.0), (0.0, 300.0), (0.0, 0.0), (0.0, 300.0)], 10.0);
    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    for segment in build_segments(&runs, &r, &geom, TOL, FUSE) {
        let starts: Vec<usize> = segment.passages.iter().map(|p| p.s).collect();
        assert!(
            starts.windows(2).all(|w| w[0] < w[1]),
            "emprunts non chronologiques : {starts:?}"
        );
    }
}

/// Un `Resample` de test est toujours réutilisable : on vérifie ici que
/// l'assemblage d'une trace à laquelle il ne manque rien ne produit pas de
/// segment fantôme.
#[test]
fn a_closed_loop_without_repetition_yields_no_segment() {
    // Carré sans superposition : aucun emprunt n'est répété.
    let (geom, r) = sampled_trace(
        &[
            (0.0, 0.0),
            (300.0, 0.0),
            (300.0, 300.0),
            (0.0, 300.0),
            (0.0, 0.0),
        ],
        10.0,
    );
    let runs = find_runs(&r, TOL, MIN_LEN).unwrap();

    let segments = build_segments(&runs, &r, &geom, TOL, FUSE);

    assert!(
        segments.is_empty(),
        "un carré parcouru une fois ne répète aucun tronçon : {segments:?}"
    );
}
