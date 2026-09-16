//! Tests de `gpx_multiride::adjustments` — ajustements manuels de la détection.
//!
//! Les règles de la fusion manuelle sont la partie la plus fine du module : elles
//! décident ce que l'utilisateur obtient quand il déclare que deux tronçons n'en
//! font qu'un. Le cas de référence de la spécification (documentation technique
//! §3.5) est reproduit tel quel : deux segments adjacents, chacun avec sa
//! référence et son retour, doivent donner **deux** emprunts fusionnés — la
//! référence d'un côté, le retour de l'autre.

use std::fs;
use std::path::PathBuf;

use crate::gpx_multiride::adjustments::{merge_segment, sens_rank, toggle_fp};
use crate::gpx_multiride::commands::{merge_impl, reset_impl, toggle_fp_impl};
use crate::gpx_multiride::file::{build_archive, file_path, load_file, save_file};
use crate::gpx_multiride::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultirideSens,
};

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé (nettoyé au préalable), propre à chaque test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_multiride_adj_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn params() -> MultirideParams {
    MultirideParams {
        tolerance_m: 10.0,
        longueur_min_m: 100.0,
        pas_echantillonnage_m: 4.0,
        fusion_references_m: 100.0,
    }
}

/// Emprunt de test : bornes kilométriques et sens, le reste à des valeurs fixes.
fn passage(
    segment: usize,
    passage: usize,
    sens: MultirideSens,
    km_entree: f64,
    km_sortie: f64,
) -> MultiridePassage {
    MultiridePassage {
        segment,
        passage,
        sens,
        faux_positif: false,
        point_entree: 1,
        point_sortie: 2,
        km_entree,
        km_sortie,
        longueur_km: km_sortie - km_entree,
        fusionne: false,
        entree: MultirideLatLon {
            lat: 45.0,
            lon: 2.0,
        },
        sortie: MultirideLatLon {
            lat: 45.1,
            lon: 2.1,
        },
    }
}

/// État de détection à partir d'une liste d'emprunts.
fn archive(passages: Vec<MultiridePassage>) -> MultirideArchive {
    build_archive("t-1", "trace.gpx", params(), 100, 100.0, false, passages)
}

/// Les deux segments de la spécification : une référence et un retour chacun.
///
/// Bornes reprises du cas de référence de la documentation technique §3.5.
fn reference_case() -> MultirideArchive {
    archive(vec![
        passage(1, 1, MultirideSens::Reference, 7.62, 11.86),
        passage(1, 2, MultirideSens::Retour, 62.80, 67.05),
        passage(2, 1, MultirideSens::Reference, 11.99, 23.06),
        passage(2, 2, MultirideSens::Retour, 51.37, 62.54),
    ])
}

/// Résumé lisible d'un état : `(segment, emprunt, sens, km début, km fin)`.
fn summary(archive: &MultirideArchive) -> Vec<(usize, usize, MultirideSens, f64, f64)> {
    archive
        .passages
        .iter()
        .map(|p| (p.segment, p.passage, p.sens, p.km_entree, p.km_sortie))
        .collect()
}

/// GPX minimal de `n` points alignés (~11 m entre deux points).
fn make_gpx(n: usize) -> String {
    let mut body = String::new();
    for i in 0..n {
        body.push_str(&format!(
            "<trkpt lat=\"{:.6}\" lon=\"{:.6}\"><ele>120.0</ele></trkpt>",
            45.0 + (i as f64) * 0.0001,
            2.0 + (i as f64) * 0.0001
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <gpx version=\"1.1\" creator=\"test\" xmlns=\"http://www.topografix.com/GPX/1/1\">\
         <trk><name>Trace de test</name><trkseg>{}</trkseg></trk></gpx>",
        body
    )
}

/// Mode temporaire portant le GPX d'une trace, pour les commandes qui relisent
/// la trace (la réinitialisation rejoue la détection).
fn mode_with_gpx(name: &str, trace_id: &str, points: usize) -> PathBuf {
    let mode = test_dir(name);
    let dir = mode.join("traces").join(trace_id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("trace.gpx"), make_gpx(points)).unwrap();
    mode
}

// ─── Rangs de sens ────────────────────────────────────────────────────

/// Le rang de sens est celui de la spécification : `0` référence, `+1` aller,
/// `−1` retour. C'est lui que compare la fusion manuelle.
#[test]
fn sens_ranks_follow_the_specification() {
    assert_eq!(sens_rank(MultirideSens::Reference), 0);
    assert_eq!(sens_rank(MultirideSens::Aller), 1);
    assert_eq!(sens_rank(MultirideSens::Retour), -1);
}

// ─── Faux positif ─────────────────────────────────────────────────────

/// Le marquage porte sur **tout** le segment, et se retire de même.
#[test]
fn a_false_positive_is_marked_on_the_whole_segment() {
    let mut base = reference_case();
    base.passages[2].faux_positif = true; // marquage partiel : le segment l'est
    let marked = toggle_fp(&base, 2).unwrap();
    assert!(marked.passages.iter().filter(|p| p.segment == 2).all(|p| p.faux_positif));

    let unmarked = toggle_fp(&marked, 2).unwrap();
    assert!(unmarked.passages.iter().filter(|p| p.segment == 2).all(|p| !p.faux_positif));
    // Le segment voisin n'a pas bougé.
    assert!(unmarked.passages.iter().filter(|p| p.segment == 1).all(|p| !p.faux_positif));
}

#[test]
fn a_false_positive_on_an_unknown_segment_is_refused() {
    let error = toggle_fp(&reference_case(), 9).unwrap_err();
    assert!(error.contains("introuvable"), "message : {error}");
}

// ─── Fusion ───────────────────────────────────────────────────────────

/// Le cas de référence de la spécification : deux segments adjacents donnent
/// deux emprunts fusionnés, la référence d'un côté et le retour de l'autre.
#[test]
fn the_specification_reference_case_merges_into_two_passages() {
    let merged = merge_segment(&reference_case(), 2).unwrap();

    assert_eq!(
        summary(&merged),
        vec![
            (1, 1, MultirideSens::Reference, 7.62, 23.06),
            (1, 2, MultirideSens::Retour, 51.37, 67.05),
        ]
    );
    assert!(
        merged.passages.iter().all(|p| p.fusionne),
        "le segment fusionné est signalé comme tel"
    );
}

/// Le premier segment n'a pas de précédent : la fusion est refusée.
#[test]
fn the_first_segment_cannot_be_merged() {
    let error = merge_segment(&reference_case(), 1).unwrap_err();
    assert!(error.contains("précédent"), "message : {error}");
}

/// La frontière entre deux **sens** est intangible : une référence et un retour
/// voisins ne fusionnent jamais, même très proches.
#[test]
fn the_frontier_between_directions_is_never_crossed() {
    let base = archive(vec![
        passage(1, 1, MultirideSens::Reference, 0.0, 10.0),
        passage(2, 1, MultirideSens::Retour, 10.1, 20.0),
    ]);

    let merged = merge_segment(&base, 2).unwrap();

    assert_eq!(merged.passages.len(), 2, "les deux sens restent distincts");
}

/// Un trou plus grand que la tolérance de fusion manuelle (1 km) n'est pas
/// comblé : au-delà, les deux portions sont réellement distinctes.
#[test]
fn a_gap_beyond_the_manual_tolerance_is_not_bridged() {
    let base = archive(vec![
        passage(1, 1, MultirideSens::Reference, 0.0, 10.0),
        passage(2, 1, MultirideSens::Reference, 11.5, 20.0),
    ]);

    let merged = merge_segment(&base, 2).unwrap();

    assert_eq!(merged.passages.len(), 2, "1,5 km d'écart : rien à recoudre");
}

/// Une ancienne référence qui n'est plus en tête bascule en **aller** : le sens
/// reste relatif à la référence du segment fusionné.
#[test]
fn an_old_reference_becomes_an_aller() {
    let merged = merge_segment(&reference_case(), 2).unwrap();

    // Seule la première référence subsiste, les autres passages sont qualifiés
    // par rapport à elle.
    assert_eq!(
        merged
            .passages
            .iter()
            .filter(|p| p.sens == MultirideSens::Reference)
            .count(),
        1
    );
    assert_eq!(merged.passages[1].sens, MultirideSens::Retour);
}

/// Les segments suivants se décalent d'un rang, et leur numérotation interne
/// reste intacte.
#[test]
fn the_following_segments_shift_down() {
    let base = archive(vec![
        passage(1, 1, MultirideSens::Reference, 0.0, 10.0),
        passage(2, 1, MultirideSens::Reference, 11.0, 20.0),
        passage(3, 1, MultirideSens::Reference, 100.0, 110.0),
        passage(3, 2, MultirideSens::Retour, 200.0, 210.0),
    ]);

    let merged = merge_segment(&base, 2).unwrap();

    assert_eq!(
        summary(&merged),
        vec![
            (1, 1, MultirideSens::Reference, 0.0, 20.0),
            (2, 1, MultirideSens::Reference, 100.0, 110.0),
            (2, 2, MultirideSens::Retour, 200.0, 210.0),
        ],
        "le troisième segment devient le deuxième, sans renumérotation interne"
    );
}

/// Fusionner un segment écarté garde le résultat **écarté** : mieux vaut
/// continuer d'exclure de l'export ce que l'utilisateur avait rejeté que de le
/// réintroduire à son insu.
#[test]
fn merging_keeps_the_segment_excluded_when_either_side_was() {
    let mut base = reference_case();
    for passage in base.passages.iter_mut().filter(|p| p.segment == 2) {
        passage.faux_positif = true;
    }

    let merged = merge_segment(&base, 2).unwrap();

    assert!(merged.passages.iter().all(|p| p.faux_positif));
}

// ─── Commandes ────────────────────────────────────────────────────────

/// L'ajustement est écrit sur disque : une réouverture de la vue le retrouve.
#[test]
fn an_adjustment_is_persisted() {
    let mode = test_dir("persistance");
    let base = reference_case();

    let updated = merge_impl(&mode, "t-1", base, 2).unwrap();

    let reloaded = load_file(&file_path(&mode, "t-1"), "t-1").expect("état relu");
    assert_eq!(reloaded.passages.len(), updated.passages.len());
    assert!(reloaded.passages.iter().all(|p| p.fusionne));
}

/// Un état appartenant à une autre trace est refusé, sans rien écrire.
#[test]
fn an_adjustment_on_a_foreign_archive_is_refused() {
    let mode = test_dir("trace_etrangere");

    let error = toggle_fp_impl(&mode, "autre", reference_case(), 1).unwrap_err();

    assert!(error.contains("autre trace"), "message : {error}");
    assert!(!file_path(&mode, "autre").exists());
}

/// La réinitialisation rejoue la détection et **conserve le statut de
/// validation** : un ajustement n'a pas d'incidence sur l'édition caméra.
#[test]
fn resetting_replays_the_detection_and_keeps_the_validation() {
    let mode = mode_with_gpx("reinitialisation", "t-1", 8);
    let mut base = archive(vec![passage(1, 1, MultirideSens::Reference, 0.0, 0.01)]);
    base.valide = true;
    save_file(&file_path(&mode, "t-1"), &base).unwrap();

    let fresh = reset_impl(&mode, "t-1", base).unwrap();

    assert!(fresh.valide, "le statut de validation est conservé");
    // La trace de test est rectiligne : la détection rejouée ne trouve rien.
    assert!(fresh.passages.is_empty(), "détection rejouée : {fresh:?}");
    assert_eq!(
        fresh.trace_point_count, 8,
        "l'état revient à celui de la détection, pas de l'ajustement"
    );
    assert_eq!(load_file(&file_path(&mode, "t-1"), "t-1").unwrap().valide, true);
}

/// Réinitialiser l'état d'une autre trace est refusé.
#[test]
fn resetting_a_foreign_archive_is_refused() {
    let mode = mode_with_gpx("reinit_etrangere", "t-1", 8);
    let error = reset_impl(&mode, "autre", reference_case()).unwrap_err();
    assert!(error.contains("autre trace"), "message : {error}");
}
