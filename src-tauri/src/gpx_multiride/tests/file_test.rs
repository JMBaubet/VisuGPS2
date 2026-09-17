//! Tests du fichier de description des passages multiples
//! (`traces/{trace_id}/multiride.json`).
//!
//! Ils couvrent le **contrat de fichier** : nomenclature de la spécification
//! (§F-17 / annexe 13.4), géométrie bornée à deux points, ajouts de
//! restauration (`version`, `trace_id`, `valide`, `faux_positif`,
//! `avant_fusion`), écriture atomique, et robustesse de la lecture (fichier
//! absent, illisible, d'une version inconnue, rattaché à une autre trace, ou
//! écrit avant l'introduction d'un champ optionnel).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::gpx_multiride::file::{build_archive, file_path, load_file, save_file, FILE_VERSION};
use crate::gpx_multiride::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultirideSens,
    STATUS_NONE, STATUS_PENDING, STATUS_VALIDATED,
};

// ─── Aides de test ────────────────────────────────────────────────────

const TRACE_ID: &str = "cd9e49cb-40fb-43a6-896e-ef2fdde357de";

/// Dossier temporaire isolé (nettoyé au préalable), propre à chaque test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_multiride_file_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Paramètres de test, alignés sur les valeurs par défaut `Multiride.Detection`.
fn params() -> MultirideParams {
    MultirideParams {
        tolerance_m: 10.0,
        longueur_min_m: 100.0,
        pas_echantillonnage_m: 4.0,
        fusion_references_m: 100.0,
    }
}

/// Passage de test : bornes et métadonnées fixes, réutilisées par les
/// assertions du round-trip.
fn passage(
    segment: usize,
    passage: usize,
    sens: MultirideSens,
    faux_positif: bool,
    fusionne: bool,
) -> MultiridePassage {
    MultiridePassage {
        segment,
        passage,
        sens,
        faux_positif,
        point_entree: 240,
        point_sortie: 842,
        km_entree: 7.62,
        km_sortie: 23.06,
        longueur_km: 15.44,
        fusionne,
        valide: false,
        avant_fusion: None,
        entree: MultirideLatLon {
            lat: 43.77584,
            lon: 7.49912,
        },
        sortie: MultirideLatLon {
            lat: 43.79012,
            lon: 7.49278,
        },
    }
}

/// État de référence des tests : deux segments, dont un marqué faux positif, et
/// une fusion manuelle sur le premier.
fn archive_with_passages() -> MultirideArchive {
    build_archive(
        TRACE_ID,
        "CalpePhoto.gpx",
        params(),
        2207,
        42.5,
        false,
        vec![
            passage(1, 1, MultirideSens::Reference, false, true),
            passage(1, 2, MultirideSens::Retour, false, true),
            passage(2, 1, MultirideSens::Reference, true, false),
        ],
    )
}

/// Contenu JSON du fichier écrit par `save_file`.
fn read_json(path: &std::path::Path) -> Value {
    let content = fs::read_to_string(path).expect("fichier de description illisible");
    serde_json::from_str(&content).expect("fichier de description non JSON")
}

// ─── Chemin et round-trip ─────────────────────────────────────────────

/// Le fichier vit dans le dossier de la trace, à côté de son GPX.
#[test]
fn file_path_lives_in_the_trace_directory() {
    let path = file_path(std::path::Path::new("/mode"), TRACE_ID);
    assert!(path.ends_with(format!("traces/{}/multiride.json", TRACE_ID)));
}

/// Un état écrit puis relu est identique : le fichier est un état
/// **restaurable**, pas seulement un export.
#[test]
fn round_trip_preserves_the_state() {
    let dir = test_dir("round_trip");
    let path = file_path(&dir, TRACE_ID);
    let archive = archive_with_passages();

    save_file(&path, &archive).unwrap();
    let loaded = load_file(&path, TRACE_ID).expect("fichier écrit mais non relu");

    assert_eq!(loaded.version, FILE_VERSION);
    assert_eq!(loaded.trace_id, TRACE_ID);
    assert_eq!(loaded.source, "CalpePhoto.gpx");
    assert_eq!(loaded.updated_at, archive.updated_at);
    assert!(!loaded.valide);
    assert_eq!(loaded.trace_point_count, 2207);
    assert_eq!(loaded.trace_length_km, 42.5);
    assert!(!loaded.pas_plafonne);
    assert_eq!(loaded.params.tolerance_m, 10.0);
    assert_eq!(loaded.params.fusion_references_m, 100.0);
    assert_eq!(loaded.passages.len(), 3);

    let first = &loaded.passages[0];
    assert_eq!(first.segment, 1);
    assert_eq!(first.passage, 1);
    assert_eq!(first.sens, MultirideSens::Reference);
    assert_eq!(first.point_entree, 240);
    assert_eq!(first.point_sortie, 842);
    assert_eq!(first.km_entree, 7.62);
    assert_eq!(first.longueur_km, 15.44);
    assert_eq!(first.entree.lat, 43.77584);
    assert_eq!(first.sortie.lon, 7.49278);
    assert_eq!(loaded.passages[1].sens, MultirideSens::Retour);
}

/// Le fichier suit la nomenclature de la spécification, et non celle du
/// projet : c'est un contrat de sortie.
#[test]
fn file_follows_the_specification_nomenclature() {
    let dir = test_dir("nomenclature");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();
    let json = read_json(&path);

    assert_eq!(json["type"], "FeatureCollection");
    assert_eq!(json["properties"]["version"], 1);
    assert_eq!(json["properties"]["trace_id"], TRACE_ID);
    assert_eq!(json["properties"]["valide"], false);
    assert_eq!(json["properties"]["source"], "CalpePhoto.gpx");
    assert_eq!(json["properties"]["trace"]["point_count"], 2207);
    assert_eq!(json["properties"]["trace"]["length_km"], 42.5);
    assert_eq!(json["properties"]["parametres"]["tolerance_m"], 10.0);
    assert_eq!(json["properties"]["parametres"]["longueur_min_m"], 100.0);
    assert_eq!(json["properties"]["parametres"]["pas_echantillonnage_m"], 4.0);
    assert_eq!(json["properties"]["parametres"]["fusion_references_m"], 100.0);
    assert_eq!(json["properties"]["parametres"]["pas_plafonne"], false);
    assert!(json["properties"]["note"].is_string());

    let features = json["features"].as_array().unwrap();
    assert_eq!(features.len(), 3);
    let feature = &features[1];
    assert_eq!(feature["type"], "Feature");
    assert_eq!(feature["properties"]["segment"], 1);
    assert_eq!(feature["properties"]["passage"], 2);
    assert_eq!(feature["properties"]["sens"], "retour");
    assert_eq!(feature["properties"]["point_entree"], 240);
    assert_eq!(feature["properties"]["km_sortie"], 23.06);
    assert_eq!(feature["properties"]["longueur_km"], 15.44);
    assert_eq!(feature["properties"]["fusionne"], true);
}

/// Chaque Feature porte une géométrie bornée à **deux** points, en `[lon, lat]`
/// (§CA-15 : une Feature par passage, la portion complète se reconstituant par
/// jointure sur `point_entree` / `point_sortie`).
#[test]
fn features_have_two_point_geometries() {
    let dir = test_dir("geometry");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();
    let json = read_json(&path);

    for feature in json["features"].as_array().unwrap() {
        assert_eq!(feature["geometry"]["type"], "LineString");
        let coordinates = feature["geometry"]["coordinates"].as_array().unwrap();
        assert_eq!(coordinates.len(), 2, "la géométrie doit être bornée à 2 points");
        // Ordre GeoJSON : longitude puis latitude.
        assert_eq!(coordinates[0][0], 7.49912);
        assert_eq!(coordinates[0][1], 43.77584);
    }
}

/// Les passages d'un segment marqué faux positif **restent** dans le fichier
/// (l'export les exclut) : sans eux, une réouverture de la vue ne pourrait plus
/// distinguer un segment écarté d'un segment ordinaire.
#[test]
fn false_positives_stay_in_the_file_and_are_counted() {
    let dir = test_dir("faux_positifs");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();
    let json = read_json(&path);

    let features = json["features"].as_array().unwrap();
    assert_eq!(features.len(), 3);
    assert_eq!(features[2]["properties"]["faux_positif"], true);
    assert_eq!(features[0]["properties"]["faux_positif"], false);
    assert_eq!(
        json["properties"]["ajustements"]["faux_positifs_exclus"], 1,
        "un segment faux positif"
    );
    assert_eq!(
        json["properties"]["ajustements"]["fusions_manuelles"], 1,
        "une fusion manuelle, comptée par segment"
    );

    // Le marquage survit à la relecture.
    let loaded = load_file(&path, TRACE_ID).unwrap();
    assert!(loaded.passages[2].faux_positif);
}

// ─── Enregistrement d'annulation (`avant_fusion`) ─────────────────────

/// Les emprunts d'avant une fusion sont écrits avec la Feature du segment
/// fusionné, à la forme du contrat : ce sont des Features complètes.
#[test]
fn a_merge_record_is_written_and_read_back() {
    let dir = test_dir("avant_fusion");
    let path = file_path(&dir, TRACE_ID);
    let saved = passage(1, 2, MultirideSens::Retour, false, false);
    let mut archive = archive_with_passages();
    archive.passages[0].avant_fusion = Some(vec![saved]);

    save_file(&path, &archive).unwrap();
    let json = read_json(&path);

    let record = json["features"][0]["properties"]["avant_fusion"]
        .as_array()
        .expect("enregistrement écrit");
    assert_eq!(record.len(), 1);
    assert_eq!(record[0]["type"], "Feature");
    assert_eq!(record[0]["properties"]["passage"], 2);
    assert_eq!(record[0]["properties"]["sens"], "retour");
    assert_eq!(record[0]["geometry"]["coordinates"][0][0], 7.49912);

    let loaded = load_file(&path, TRACE_ID).expect("fichier relu");
    let reloaded_record = loaded.passages[0]
        .avant_fusion
        .as_ref()
        .expect("enregistrement relu");
    assert_eq!(reloaded_record.len(), 1);
    assert_eq!(reloaded_record[0].passage, 2);
    assert_eq!(reloaded_record[0].sens, MultirideSens::Retour);
    assert_eq!(reloaded_record[0].km_sortie, 23.06);
    assert!(loaded.passages[1].avant_fusion.is_none());
}

/// Le champ est **absent** du fichier quand il n'y a rien à restaurer : le
/// contrat de sortie n'est pas alourdi par un `null` systématique.
#[test]
fn the_undo_record_is_absent_without_a_merge() {
    let dir = test_dir("avant_fusion_absente");
    let path = file_path(&dir, TRACE_ID);

    save_file(&path, &archive_with_passages()).unwrap();
    let json = read_json(&path);

    for feature in json["features"].as_array().unwrap() {
        assert!(
            feature["properties"].get("avant_fusion").is_none(),
            "aucun enregistrement à écrire : {feature}"
        );
    }
}

/// Un fichier écrit **avant** l'introduction du champ se lit encore : la
/// version du format n'a donc pas à changer, le segment fusionné n'est
/// simplement plus annulable.
#[test]
fn a_file_written_before_the_field_still_loads() {
    let dir = test_dir("avant_le_champ");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();

    // Ce que produisait la version précédente : le champ en moins.
    let mut json = read_json(&path);
    for feature in json["features"].as_array_mut().unwrap() {
        feature["properties"]
            .as_object_mut()
            .unwrap()
            .remove("avant_fusion");
    }
    fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

    let loaded = load_file(&path, TRACE_ID).expect("fichier antérieur toujours lisible");

    assert_eq!(loaded.version, FILE_VERSION);
    assert_eq!(loaded.passages.len(), 3);
    assert!(loaded.passages.iter().all(|p| p.avant_fusion.is_none()));
    assert!(loaded.passages[0].fusionne, "la fusion reste lisible");
}

// ─── Approbation d'un segment (`valide`) ──────────────────────────────

/// L'approbation est écrite à la nomenclature du contrat, sur la Feature du
/// segment, et comptée dans le bloc des ajustements.
#[test]
fn an_approval_is_written_and_read_back() {
    let dir = test_dir("segment_valide");
    let path = file_path(&dir, TRACE_ID);
    // Le segment 2 du fixture est écarté : l'approbation prend sa place, un
    // segment ne portant qu'un état à la fois.
    let mut archive = archive_with_passages();
    for passage in archive.passages.iter_mut().filter(|p| p.segment == 2) {
        passage.faux_positif = false;
        passage.valide = true;
    }

    save_file(&path, &archive).unwrap();
    let json = read_json(&path);

    let features = json["features"].as_array().unwrap();
    assert_eq!(features[2]["properties"]["valide"], true);
    assert_eq!(features[2]["properties"]["faux_positif"], false);
    assert_eq!(features[0]["properties"]["valide"], false);
    assert_eq!(
        json["properties"]["ajustements"]["segments_valides"], 1,
        "un segment approuvé"
    );

    let loaded = load_file(&path, TRACE_ID).expect("fichier relu");
    assert!(loaded.passages[2].valide);
    assert!(!loaded.passages[0].valide);
}

/// Un fichier écrit avant l'introduction du champ se relit : le segment est
/// simplement « à examiner », et la version du format n'a pas à changer.
#[test]
fn a_file_without_the_approval_field_reads_as_unapproved() {
    let dir = test_dir("avant_approbation");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();

    let mut json = read_json(&path);
    for feature in json["features"].as_array_mut().unwrap() {
        feature["properties"].as_object_mut().unwrap().remove("valide");
    }
    json["properties"]["ajustements"]
        .as_object_mut()
        .unwrap()
        .remove("segments_valides");
    fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

    let loaded = load_file(&path, TRACE_ID).expect("fichier antérieur toujours lisible");

    assert_eq!(loaded.version, FILE_VERSION);
    assert_eq!(loaded.passages.len(), 3);
    assert!(loaded.passages.iter().all(|p| !p.valide));
}

// ─── Lecture tolérante ────────────────────────────────────────────────

#[test]
fn load_returns_none_when_the_file_is_absent() {
    let dir = test_dir("absent");
    assert!(load_file(&file_path(&dir, TRACE_ID), TRACE_ID).is_none());
}

#[test]
fn load_returns_none_on_unreadable_content() {
    let dir = test_dir("illisible");
    let path = file_path(&dir, TRACE_ID);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{ pas du JSON").unwrap();
    assert!(load_file(&path, TRACE_ID).is_none());
}

#[test]
fn load_returns_none_on_unknown_version() {
    let dir = test_dir("version");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();

    let mut json = read_json(&path);
    json["properties"]["version"] = Value::from(99);
    fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

    assert!(load_file(&path, TRACE_ID).is_none());
}

#[test]
fn load_returns_none_for_another_trace() {
    let dir = test_dir("autre_trace");
    let path = file_path(&dir, TRACE_ID);
    save_file(&path, &archive_with_passages()).unwrap();
    assert!(load_file(&path, "autre-trace").is_none());
}

// ─── Statut dérivé ────────────────────────────────────────────────────

/// Le statut du registre se dérive de l'état : rien à détecter → `none`,
/// passages à valider → `pending`, validé → `validated`.
#[test]
fn status_reflects_passages_and_validation() {
    let mut archive = archive_with_passages();
    assert_eq!(archive.status(), STATUS_PENDING);

    archive.valide = true;
    assert_eq!(archive.status(), STATUS_VALIDATED);

    archive.valide = false;
    archive.passages.clear();
    assert_eq!(archive.status(), STATUS_NONE);
}

/// Une détection sans passage mais validée reste `none` : la barrière protège
/// des passages multiples, il n'y en a aucun.
#[test]
fn validated_empty_detection_has_no_barrier() {
    let mut archive = archive_with_passages();
    archive.passages.clear();
    archive.valide = true;
    assert_eq!(archive.status(), STATUS_NONE);
}
