//! Tests de `gpx_multiride::adjustments` — ajustements manuels de la détection.
//!
//! Les règles de la fusion manuelle sont la partie la plus fine du module : elles
//! décident ce que l'utilisateur obtient quand il déclare que deux tronçons n'en
//! font qu'un. Le cas de référence de la spécification (documentation technique
//! §3.5) est reproduit tel quel : deux segments adjacents, chacun avec sa
//! référence et son retour, doivent donner **deux** emprunts fusionnés — la
//! référence d'un côté, le retour de l'autre.
//!
//! Deux familles de règles s'y ajoutent : l'**exclusion** du faux positif et de
//! la fusion (un segment ne porte qu'un ajustement à la fois), et
//! l'**annulation**, y compris d'une chaîne de fusions.

use std::fs;
use std::path::PathBuf;

use crate::gpx_multiride::adjustments::{
    merge_segment, sens_rank, toggle_fp, undo_segment, validate_segment,
};
use crate::gpx_multiride::commands::{
    merge_impl, reset_impl, toggle_fp_impl, undo_impl, validate_segment_impl,
};
use crate::gpx_multiride::file::{build_archive, file_path, load_file, save_file};
use crate::import_gpx::{
    get_traces_path, save_registry, Point3D, TraceMetadata, TraceStats,
};
use crate::gpx_multiride::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultirideSens,
    STATUS_PENDING,
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
        valide: false,
        avant_fusion: None,
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

/// Résumé lisible d'emprunts : `(segment, emprunt, sens, km début, km fin)`.
fn summary_of(passages: &[MultiridePassage]) -> Vec<(usize, usize, MultirideSens, f64, f64)> {
    passages
        .iter()
        .map(|p| (p.segment, p.passage, p.sens, p.km_entree, p.km_sortie))
        .collect()
}

/// Résumé lisible d'un état : `(segment, emprunt, sens, km début, km fin)`.
fn summary(archive: &MultirideArchive) -> Vec<(usize, usize, MultirideSens, f64, f64)> {
    summary_of(&archive.passages)
}

/// Marque tous les emprunts d'un segment en faux positif.
fn mark_false_positive(mut archive: MultirideArchive, segment: usize) -> MultirideArchive {
    for passage in archive
        .passages
        .iter_mut()
        .filter(|p| p.segment == segment)
    {
        passage.faux_positif = true;
    }
    archive
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

/// Mode temporaire portant le GPX d'une trace **et son entrée au registre**,
/// pour les commandes qui relisent la trace (la réinitialisation rejoue la
/// détection) ou reposent son statut (tout geste).
fn mode_with_gpx(name: &str, trace_id: &str, points: usize) -> PathBuf {
    let mode = test_dir(name);
    let dir = mode.join("traces").join(trace_id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("trace.gpx"), make_gpx(points)).unwrap();
    save_registry(
        &get_traces_path(&mode),
        &[make_registry_entry(trace_id, "trace.gpx")],
    )
    .unwrap();
    mode
}

/// Entrée minimale du registre : trace auditée, sans détection de passages
/// multiples — c'est le point de départ des commandes de geste.
fn make_registry_entry(trace_id: &str, filename: &str) -> TraceMetadata {
    TraceMetadata {
        id: trace_id.to_string(),
        name: "Trace de test".to_string(),
        source: "Test".to_string(),
        source_url: None,
        activity_type: None,
        filename: filename.to_string(),
        import_date: "2026-01-01T00:00:00Z".to_string(),
        stats: TraceStats {
            start_point: Point3D {
                lat: 45.0,
                lon: 2.0,
                alt: None,
            },
            end_point: Point3D {
                lat: 45.0,
                lon: 2.1,
                alt: None,
            },
            distance_m: 1000.0,
            positive_elevation_m: 0.0,
            negative_elevation_m: 0.0,
            alt_min_m: None,
            alt_max_m: None,
            points_count: 8,
            duration_s: None,
        },
        hash: "sha256:test".to_string(),
        favorite: false,
        is_displayed: false,
        audit_status: "clean".to_string(),
        audit_archived: false,
        multiride_status: None,
    }
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

/// Un segment faux positif ne fusionne pas — ni comme segment absorbé, ni comme
/// segment absorbant : le résultat serait à la fois fusionné et écarté de
/// l'export, et aucun des deux gestes ne pourrait être annulé seul.
#[test]
fn a_false_positive_segment_never_merges() {
    // Le précédent est écarté : la fusion est refusée.
    let error = merge_segment(&mark_false_positive(reference_case(), 1), 2).unwrap_err();
    assert!(error.contains("segment 1"), "message : {error}");
    assert!(error.contains("faux positif"), "message : {error}");

    // Le segment absorbé est écarté : refus également.
    let error = merge_segment(&mark_false_positive(reference_case(), 2), 2).unwrap_err();
    assert!(error.contains("segment 2"), "message : {error}");
    assert!(error.contains("faux positif"), "message : {error}");
}

/// L'inverse est vrai aussi : un segment fusionné ne s'écarte pas — le marquage
/// est refusé, le retrait du marqueur resterait possible.
#[test]
fn a_merged_segment_cannot_be_marked_false_positive() {
    let merged = merge_segment(&reference_case(), 2).unwrap();

    let error = toggle_fp(&merged, 1).unwrap_err();

    assert!(error.contains("fusionné"), "message : {error}");
}

// ─── Approbation d'un segment ─────────────────────────────────────────

/// Approuver un segment le marque **en entier** : c'est un fait de segment.
#[test]
fn approving_a_segment_marks_the_whole_segment() {
    let approved = validate_segment(&reference_case(), 2).unwrap();

    assert!(approved.passages.iter().filter(|p| p.segment == 2).all(|p| p.valide));
    assert!(
        approved.passages.iter().filter(|p| p.segment == 1).all(|p| !p.valide),
        "le segment voisin n'est pas touché : {approved:?}"
    );
}

/// Un segment **écarté** ne s'approuve pas : un segment exclu de l'export ne
/// peut pas être dit juste, la contradiction serait dans les termes.
#[test]
fn an_excluded_segment_cannot_be_approved() {
    let error = validate_segment(&mark_false_positive(reference_case(), 1), 1).unwrap_err();
    assert!(error.contains("segment 1"), "message : {error}");
    assert!(error.contains("faux positif"), "message : {error}");
}

/// Un segment **fusionné** s'approuve, en revanche : la fusion réorganise la
/// détection, elle ne dit rien de sa justesse — et le segment fusionné est un
/// segment neuf, à juger comme les autres. Sans quoi il resterait à examiner
/// pour toujours, et l'avancement ne pourrait jamais être complet.
#[test]
fn a_merged_segment_can_be_approved() {
    let merged = merge_segment(&reference_case(), 2).unwrap();
    assert!(merged.passages.iter().all(|p| !p.valide));

    let approved = validate_segment(&merged, 1).unwrap();

    assert!(
        approved.passages.iter().all(|p| p.valide && p.fusionne),
        "approuvé sans cesser d'être fusionné : {approved:?}"
    );
}

/// Les deux ordres de gestes s'annulent séparément : le verdict d'abord, la
/// fusion ensuite.
#[test]
fn undoing_a_merged_segment_peels_the_approval_then_the_merge() {
    let base = reference_case();
    let merged = merge_segment(&base, 2).unwrap();
    let approved = validate_segment(&merged, 1).unwrap();

    // Premier retour : l'approbation, le segment restant fusionné.
    let back_to_merged = undo_segment(&approved, 1).unwrap();
    assert_eq!(summary(&back_to_merged), summary(&merged));
    assert!(back_to_merged.passages.iter().all(|p| p.fusionne && !p.valide));

    // Second retour : la fusion, la détection retrouvant ses deux segments.
    let back_to_base = undo_segment(&back_to_merged, 1).unwrap();
    assert_eq!(summary(&back_to_base), summary(&base));
}

/// Un segment introuvable est refusé comme partout ailleurs dans le module.
#[test]
fn approving_an_unknown_segment_is_refused() {
    let error = validate_segment(&reference_case(), 9).unwrap_err();
    assert!(error.contains("introuvable"), "message : {error}");
}

/// Écarter un segment approuvé prend la place de l'approbation : rien n'est
/// perdu, une approbation ne conservant aucune donnée.
#[test]
fn excluding_an_approved_segment_clears_the_approval() {
    let approved = validate_segment(&reference_case(), 2).unwrap();

    let excluded = toggle_fp(&approved, 2).unwrap();

    assert!(excluded.passages.iter().all(|p| !p.valide));
    assert!(excluded.passages.iter().filter(|p| p.segment == 2).all(|p| p.faux_positif));
}

/// Une fusion produit un segment **neuf** : l'approbation portée par l'un des
/// deux camps ne le couvre pas, et l'annulation la restitue.
#[test]
fn merging_clears_the_approval_and_undoing_restores_it() {
    let approved = validate_segment(&reference_case(), 1).unwrap();
    let approved = validate_segment(&approved, 2).unwrap();

    let merged = merge_segment(&approved, 2).unwrap();
    assert!(
        merged.passages.iter().all(|p| !p.valide),
        "le segment fusionné est à approuver : {merged:?}"
    );

    let undone = undo_segment(&merged, 1).unwrap();
    assert!(
        undone.passages.iter().all(|p| p.valide),
        "l'instantané porte l'approbation des deux segments : {undone:?}"
    );
}

/// Approuver un segment ne vaut pas **valider la détection** : deux champs
/// distincts, que la barrière de l'édition caméra ne confond pas. Une détection
/// dont tous les segments sont approuvés reste à valider.
#[test]
fn approving_a_segment_does_not_validate_the_detection() {
    let approved = validate_segment(&reference_case(), 1).unwrap();

    assert!(approved.passages.iter().any(|p| p.valide));
    assert!(!approved.valide, "la détection reste à valider");
    assert_eq!(approved.status(), STATUS_PENDING);
}

/// Tout geste invalide la validation de la détection : elle portait sur un état
/// qui n'est plus celui-ci, et la barrière de l'édition caméra se referme.
#[test]
fn every_gesture_invalidates_the_detection_validation() {
    let mut validated = reference_case();
    validated.valide = true;

    assert!(!validate_segment(&validated, 1).unwrap().valide, "approuver");
    assert!(!toggle_fp(&validated, 2).unwrap().valide, "écarter");
    assert!(!merge_segment(&validated, 2).unwrap().valide, "fusionner");

    // Annuler est un geste comme un autre : l'état d'avant n'est pas celui qui
    // avait été validé.
    let mut marked = toggle_fp(&reference_case(), 2).unwrap();
    marked.valide = true;
    assert!(!undo_segment(&marked, 2).unwrap().valide, "annuler");
}

// ─── Annulation ───────────────────────────────────────────────────────
/// La fusion enregistre les emprunts d'avant, tels quels : c'est de là que
/// l'annulation tire l'état à réinstaller.
#[test]
fn a_merge_records_the_passages_of_before() {
    let base = reference_case();

    let merged = merge_segment(&base, 2).unwrap();

    let saved = merged.passages[0]
        .avant_fusion
        .as_ref()
        .expect("enregistrement d'annulation");
    assert_eq!(
        summary_of(saved),
        summary(&base),
        "l'instantané est l'état des deux segments d'origine"
    );
    assert!(
        saved.iter().all(|p| p.avant_fusion.is_none()),
        "aucun enregistrement antérieur : il n'y a pas eu de fusion avant"
    );
}

/// Annuler une fusion rend aux deux segments leurs emprunts, leur sens et leur
/// numérotation d'origine — et ne laisse aucun enregistrement derrière elle.
#[test]
fn undoing_a_merge_restores_the_original_segments() {
    let base = reference_case();
    let merged = merge_segment(&base, 2).unwrap();

    let undone = undo_segment(&merged, 1).unwrap();

    assert_eq!(summary(&undone), summary(&base));
    assert!(
        undone.passages.iter().all(|p| !p.fusionne && p.avant_fusion.is_none()),
        "l'état restauré ne porte plus d'ajustement : {undone:?}"
    );
}

/// Les segments que la fusion avait décalés d'un rang reprennent leur numéro.
#[test]
fn undoing_a_merge_shifts_the_following_segments_back() {
    let base = archive(vec![
        passage(1, 1, MultirideSens::Reference, 0.0, 10.0),
        passage(2, 1, MultirideSens::Reference, 11.0, 20.0),
        passage(3, 1, MultirideSens::Reference, 100.0, 110.0),
        passage(3, 2, MultirideSens::Retour, 200.0, 210.0),
    ]);
    let merged = merge_segment(&base, 2).unwrap();
    assert_eq!(merged.passages.last().unwrap().segment, 2, "le 3ᵉ est devenu 2ᵉ");

    let undone = undo_segment(&merged, 1).unwrap();

    assert_eq!(summary(&undone), summary(&base));
}

/// Une chaîne de fusions s'annule **pas à pas**, de la plus récente à la plus
/// ancienne : l'instantané d'une fusion contient celui de la précédente, et
/// c'est ce qui rend l'état intermédiaire à nouveau annulable.
#[test]
fn a_chain_of_merges_is_undone_step_by_step() {
    let base = archive(vec![
        passage(1, 1, MultirideSens::Reference, 0.0, 10.0),
        passage(2, 1, MultirideSens::Reference, 10.5, 20.0),
        passage(3, 1, MultirideSens::Reference, 20.5, 30.0),
    ]);

    let two_merged = merge_segment(&base, 2).unwrap(); // S2 absorbé par S1
    let three_merged = merge_segment(&two_merged, 2).unwrap(); // S3 absorbé à son tour
    assert_eq!(
        summary(&three_merged),
        vec![(1, 1, MultirideSens::Reference, 0.0, 30.0)],
        "trois segments consécutifs recousus en un seul"
    );

    // Premier retour : l'état à deux segments, toujours annulable.
    let back_to_two = undo_segment(&three_merged, 1).unwrap();
    assert_eq!(summary(&back_to_two), summary(&two_merged));

    // Second retour : l'état d'origine.
    let back_to_base = undo_segment(&back_to_two, 1).unwrap();
    assert_eq!(summary(&back_to_base), summary(&base));
    assert!(back_to_base.passages.iter().all(|p| p.avant_fusion.is_none()));
}

/// Annuler un faux positif se réduit à retirer le marqueur.
#[test]
fn undoing_a_false_positive_clears_the_marker() {
    let marked = mark_false_positive(reference_case(), 2);

    let undone = undo_segment(&marked, 2).unwrap();

    assert!(undone.passages.iter().all(|p| !p.faux_positif));
    assert_eq!(summary(&undone), summary(&reference_case()));
}

/// Annuler une approbation retire la marque et rend le segment à examiner.
#[test]
fn undoing_an_approval_clears_the_mark() {
    let approved = validate_segment(&reference_case(), 2).unwrap();

    let undone = undo_segment(&approved, 2).unwrap();

    assert!(undone.passages.iter().all(|p| !p.valide));
    assert_eq!(summary(&undone), summary(&reference_case()));
}

/// Rien à annuler : le refus est explicite, jamais silencieux.
#[test]
fn undoing_an_untouched_segment_is_refused() {
    let error = undo_segment(&reference_case(), 1).unwrap_err();
    assert!(error.contains("Aucun ajustement"), "message : {error}");
}

/// Un segment introuvable est refusé comme partout ailleurs dans le module.
#[test]
fn undoing_an_unknown_segment_is_refused() {
    let error = undo_segment(&reference_case(), 9).unwrap_err();
    assert!(error.contains("introuvable"), "message : {error}");
}

/// Un segment fusionné **sans** enregistrement — fichier écrit avant
/// l'introduction du champ `avant_fusion` — est refusé, sans être cassé.
#[test]
fn undoing_a_merge_without_a_record_is_refused() {
    let mut merged = merge_segment(&reference_case(), 2).unwrap();
    for passage in merged.passages.iter_mut() {
        passage.avant_fusion = None;
    }

    let error = undo_segment(&merged, 1).unwrap_err();

    assert!(error.contains("Aucun ajustement"), "message : {error}");
}

// ─── Commandes ────────────────────────────────────────────────────────

/// L'ajustement est écrit sur disque : une réouverture de la vue le retrouve.
#[test]
fn an_adjustment_is_persisted() {
    let mode = mode_with_gpx("persistance", "t-1", 8);
    let base = reference_case();

    let updated = merge_impl(&mode, "t-1", base, 2).unwrap();

    let reloaded = load_file(&file_path(&mode, "t-1"), "t-1").expect("état relu");
    assert_eq!(reloaded.passages.len(), updated.passages.len());
    assert!(reloaded.passages.iter().all(|p| p.fusionne));
}

/// L'annulation est écrite sur disque, enregistrement compris : une réouverture
/// retrouve l'état d'avant l'ajustement, et non plus la fusion.
#[test]
fn an_undo_is_persisted() {
    let mode = mode_with_gpx("persistance_annulation", "t-1", 8);
    let base = reference_case();
    merge_impl(&mode, "t-1", base.clone(), 2).unwrap();
    // L'enregistrement survit à l'écriture : sans lui, l'annulation ne pourrait
    // pas être jouée après une relecture.
    let reloaded = load_file(&file_path(&mode, "t-1"), "t-1").expect("état relu");
    assert!(reloaded.passages.iter().all(|p| p.avant_fusion.is_some()));

    let undone = undo_impl(&mode, "t-1", reloaded, 1).unwrap();

    assert_eq!(summary(&undone), summary(&base));
    let persisted = load_file(&file_path(&mode, "t-1"), "t-1").expect("état relu");
    assert_eq!(
        summary(&persisted),
        summary(&base),
        "le fichier porte l'état d'avant la fusion"
    );
    assert!(persisted.passages.iter().all(|p| p.avant_fusion.is_none()));
}

/// Un faux positif s'annule aussi par la commande, et l'annulation est écrite.
#[test]
fn undoing_a_false_positive_is_persisted() {
    let mode = mode_with_gpx("persistance_fp", "t-1", 8);
    let marked = toggle_fp_impl(&mode, "t-1", reference_case(), 2).unwrap();
    assert!(marked.passages.iter().any(|p| p.faux_positif));

    let undone = undo_impl(&mode, "t-1", marked, 2).unwrap();

    assert!(undone.passages.iter().all(|p| !p.faux_positif));
    assert!(
        load_file(&file_path(&mode, "t-1"), "t-1")
            .unwrap()
            .passages
            .iter()
            .all(|p| !p.faux_positif)
    );
}

/// Approbation et annulation sont écrites comme les autres gestes : une
/// réouverture de la vue retrouve les segments approuvés.
#[test]
fn an_approval_and_its_undo_are_persisted() {
    let mode = mode_with_gpx("persistance_approbation", "t-1", 8);

    let approved = validate_segment_impl(&mode, "t-1", reference_case(), 1).unwrap();
    assert!(approved.passages.iter().any(|p| p.valide));
    assert!(
        load_file(&file_path(&mode, "t-1"), "t-1")
            .unwrap()
            .passages
            .iter()
            .filter(|p| p.segment == 1)
            .all(|p| p.valide)
    );

    let undone = undo_impl(&mode, "t-1", approved, 1).unwrap();
    assert!(undone.passages.iter().all(|p| !p.valide));
    assert!(
        load_file(&file_path(&mode, "t-1"), "t-1")
            .unwrap()
            .passages
            .iter()
            .all(|p| !p.valide)
    );
}

/// Une annulation sur une autre trace est refusée, sans rien écrire.
#[test]
fn undoing_on_a_foreign_archive_is_refused() {
    let mode = test_dir("annulation_etrangere");
    // L'état porte un ajustement annulable, mais appartient à une autre trace
    // que celle de l'appel : c'est le rattachement qui est refusé.
    let adjusted = toggle_fp(&reference_case(), 2).unwrap();

    let error = undo_impl(&mode, "autre", adjusted, 2).unwrap_err();

    assert!(error.contains("autre trace"), "message : {error}");
    assert!(!file_path(&mode, "autre").exists());
}

/// Un état appartenant à une autre trace est refusé, sans rien écrire.
#[test]
fn an_adjustment_on_a_foreign_archive_is_refused() {
    let mode = test_dir("trace_etrangere");

    let error = toggle_fp_impl(&mode, "autre", reference_case(), 1).unwrap_err();

    assert!(error.contains("autre trace"), "message : {error}");
    assert!(!file_path(&mode, "autre").exists());
}

/// La réinitialisation rejoue la détection et **invalide la validation**,
/// comme tout autre geste : la détection rejouée n'est plus celle qui avait été
/// relue et validée.
#[test]
fn resetting_replays_the_detection_and_clears_the_validation() {
    let mode = mode_with_gpx("reinitialisation", "t-1", 8);
    let mut base = archive(vec![passage(1, 1, MultirideSens::Reference, 0.0, 0.01)]);
    base.valide = true;
    save_file(&file_path(&mode, "t-1"), &base).unwrap();

    let fresh = reset_impl(&mode, "t-1", base).unwrap();

    assert!(!fresh.valide, "la validation est tombée");
    // La trace de test est rectiligne : la détection rejouée ne trouve rien.
    assert!(fresh.passages.is_empty(), "détection rejouée : {fresh:?}");
    assert_eq!(
        fresh.trace_point_count, 8,
        "l'état revient à celui de la détection, pas de l'ajustement"
    );
    assert!(!load_file(&file_path(&mode, "t-1"), "t-1").unwrap().valide);
}

/// Réinitialiser l'état d'une autre trace est refusé.
#[test]
fn resetting_a_foreign_archive_is_refused() {
    let mode = mode_with_gpx("reinit_etrangere", "t-1", 8);
    let error = reset_impl(&mode, "autre", reference_case()).unwrap_err();
    assert!(error.contains("autre trace"), "message : {error}");
}
