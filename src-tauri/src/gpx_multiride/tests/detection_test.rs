//! Tests de bout en bout : la détection complète sur les traces de référence.
//!
//! ## Pourquoi ces tests ne reprennent pas les attendus des documents
//!
//! Les documents de spécification donnent des résultats attendus pour des traces
//! de référence — « Menton-Poggio » à 123 km, `CalpePhoto.gpx` à « 10 km dont 5
//! en aller-retour » —, mais les deux fichiers fournis avec la spécification ne
//! sont pas ceux-là : mesurés sur leurs points, `poggio.gpx` fait **75,69 km** et
//! `CalpePhoto.gpx` **50,54 km**. Les tableaux d'attendus (§13.3 du PRD, §5.5 de
//! l'`Algorithme.md`) décrivent donc d'autres fichiers, et les leur opposer
//! reviendrait à tester autre chose que ce que le module produit.
//!
//! Ces tests reposent donc sur des vérifications **indépendantes** du pipeline :
//!
//! - la longueur de trace est recalculée ici, en orthodromie sur les points du
//!   GPX, et comparée à celle que le module mesure dans sa projection locale —
//!   c'est ce qui valide la géométrie, et donc l'échelle de tous les seuils ;
//! - les **invariants de structure** de l'assemblage sont vérifiés sur chaque
//!   segment (une seule référence, emprunts chronologiques et disjoints, bornes
//!   cohérentes) ;
//! - la **superposition géométrique mutuelle** des emprunts d'un segment est
//!   vérifiée point par point : chaque emprunt doit passer à moins d'une
//!   tolérance d'un **autre** emprunt du même segment — la détection est ainsi
//!   validée sur ce qu'elle prétend avoir trouvé, sans dépendre d'aucun chiffre
//!   documentaire ;
//! - le **nombre d'emprunts** est figé comme verrou de non-régression. C'est une
//!   valeur relevée sur cette implémentation, pas un attendu de la
//!   spécification : elle détecte un changement de comportement, elle ne
//!   l'authentifie pas. Ce qui authentifie le portage, ce sont les attentes
//!   exactes des tests synthétiques — calculables à la main — et les invariants
//!   ci-dessus sur les traces réelles.

use std::path::{Path, PathBuf};

use crate::gpx_multiride::detection::{detect, load_points};
use crate::gpx_multiride::projection::{build_geometry, MultirideGeom};
use crate::gpx_multiride::resample::{resample, Resample};
use crate::gpx_multiride::types::{MultirideArchive, MultirideParams, MultiridePassage, MultirideSens};

/// Paramètres par défaut de la spécification (§6) — miroir de
/// `settings.default.toml`.
fn params() -> MultirideParams {
    MultirideParams {
        tolerance_m: 10.0,
        longueur_min_m: 100.0,
        pas_echantillonnage_m: 4.0,
        fusion_references_m: 100.0,
    }
}

/// Chemin d'une trace de référence.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/gpx_multiride/tests/fixtures")
        .join(name)
}

// ─── Vérifications indépendantes ──────────────────────────────────────

/// Longueur de la trace (km) calculée en orthodromie sur les points bruts du
/// GPX — la mesure de contrôle, indépendante de la projection du module.
fn geodesic_length_km(points: &[crate::gpx_multiride::types::MultiridePoint]) -> f64 {
    let mut meters = 0.0_f64;
    for pair in points.windows(2) {
        meters += crate::import_gpx::haversine(pair[0].lat, pair[0].lon, pair[1].lat, pair[1].lon);
    }
    meters / 1000.0
}

/// Reconstitue les étapes intermédiaires du pipeline, pour vérifier la détection
/// sur la trace elle-même.
fn intermediates(gpx_path: &Path) -> (MultirideGeom, Resample) {
    let points = load_points(gpx_path).expect("lecture du GPX de référence");
    let geom = build_geometry(&points).expect("géométrie de la trace");
    let sampling = resample(&geom, params().pas_echantillonnage_m).expect("rééchantillonnage");
    (geom, sampling)
}

/// Indices de rééchantillonnage d'un emprunt, retrouvés depuis ses bornes
/// kilométriques : la distance cumulée vaut exactement `indice × pas`.
fn index_range(passage: &MultiridePassage, step: f64) -> (usize, usize, f64) {
    let entry = passage.km_entree * 1000.0 / step;
    let exit = passage.km_sortie * 1000.0 / step;
    (
        entry.round() as usize,
        exit.round() as usize,
        // Écart à la grille : une borne doit tomber sur un point échantillonné.
        (entry - entry.round()).abs().max((exit - exit.round()).abs()),
    )
}

/// Distance entre deux points échantillonnés.
fn distance(r: &Resample, a: usize, b: usize) -> f64 {
    ((r.px[a] - r.px[b]).powi(2) + (r.py[a] - r.py[b]).powi(2)).sqrt()
}

/// Invariants de structure de l'assemblage, valables sur toute trace.
fn assert_structurally_sound(archive: &MultirideArchive, step: f64) {
    let mut segments_seen: Vec<usize> = Vec::new();
    for passage in &archive.passages {
        assert!(
            passage.point_entree >= 1 && passage.point_sortie <= archive.trace_point_count,
            "points hors de la trace : {passage:?}"
        );
        assert!(
            passage.point_entree <= passage.point_sortie,
            "bornes de points inversées : {passage:?}"
        );
        assert!(
            passage.km_entree <= passage.km_sortie,
            "bornes kilométriques inversées : {passage:?}"
        );
        assert!(
            (passage.km_sortie - passage.km_entree - passage.longueur_km).abs() < 1e-9,
            "longueur incohérente : {passage:?}"
        );
        assert!(
            passage.km_sortie <= archive.trace_length_km + 1e-6,
            "emprunt au-delà de la fin de trace : {passage:?}"
        );
        let (_, _, off_grid) = index_range(passage, step);
        assert!(
            off_grid < 1e-6,
            "borne kilométrique hors de la grille d'échantillonnage : {passage:?}"
        );
        if !segments_seen.contains(&passage.segment) {
            segments_seen.push(passage.segment);
        }
    }

    for segment in &segments_seen {
        let passages: Vec<&MultiridePassage> = archive
            .passages
            .iter()
            .filter(|p| p.segment == *segment)
            .collect();

        // Un segment est une répétition : au moins deux emprunts, dont une seule
        // référence, en tête.
        assert!(passages.len() >= 2, "segment {segment} à un seul emprunt");
        let references = passages
            .iter()
            .filter(|p| p.sens == MultirideSens::Reference)
            .count();
        assert_eq!(references, 1, "segment {segment} : {references} références");
        assert_eq!(
            passages[0].sens,
            MultirideSens::Reference,
            "segment {segment} : la référence doit ouvrir le segment"
        );

        // Les emprunts se succèdent le long de la trace et ne se chevauchent
        // pas : l'étape de fusion ordonnée les a disjoints.
        for pair in passages.windows(2) {
            assert!(
                pair[0].km_sortie <= pair[1].km_entree + 1e-9,
                "emprunts qui se chevauchent ou désordonnés : {pair:?}"
            );
        }
    }
}

/// Vérifie que les emprunts d'un même segment se recouvrent **mutuellement** :
/// chaque point de contrôle d'un emprunt doit passer à moins d'une tolérance d'un
/// point d'un **autre** emprunt du segment.
///
/// C'est la validation de la détection sur ce qu'elle annonce, et elle ne dépend
/// d'aucun chiffre documentaire. La référence du segment est volontairement
/// exclue de la comparaison : elle n'est que le premier emprunt détecté, et peut
/// ne couvrir qu'une partie du troncçon — chaque portion doit en revanche être
/// couverte par l'ensemble des emprunts.
///
/// Les seuils tiennent compte de ce que la détection admet **par construction** :
/// le chaînage saute jusqu'à trois correspondances (`MAXGAP`) et la Passe D recoud
/// des portions où la correspondance point-à-point se perd (virages serrés,
/// tunnels). Ce qui est exigé, c'est que la superposition tienne dans son
/// **ensemble** — distance médiane dans la tolérance, points aberrants marginaux.
fn assert_passages_mutually_superposed(archive: &MultirideArchive, r: &Resample, tol_m: f64) {
    // Un point de contrôle tous les 20 échantillons (80 m au pas par défaut).
    const CONTROL_STEP: usize = 20;
    // Au-delà de quatre fois la tolérance, un point n'est plus « superposé ».
    const ABERRANT_FACTOR: f64 = 4.0;
    // Part maximale de points aberrants admise : les trous locaux de la
    // détection, jamais une portion entière.
    const MAX_ABERRANT_RATIO: f64 = 0.05;

    let last_segment = archive.passages.iter().map(|p| p.segment).max().unwrap_or(0);
    for segment in 1..=last_segment {
        let passages: Vec<&MultiridePassage> = archive
            .passages
            .iter()
            .filter(|p| p.segment == segment)
            .collect();

        for (position, passage) in passages.iter().enumerate() {
            let others: Vec<(usize, usize)> = passages
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != position)
                .map(|(_, other)| index_range(other, r.step))
                .map(|(from, to, _)| (from, to))
                .collect();

            let (from, to, _) = index_range(passage, r.step);
            let mut distances: Vec<f64> = Vec::new();
            let mut index = from;
            while index <= to {
                let closest = others
                    .iter()
                    .flat_map(|(other_from, other_to)| *other_from..=*other_to)
                    .map(|q| distance(r, index, q))
                    .fold(f64::INFINITY, f64::min);
                distances.push(closest);
                index += CONTROL_STEP;
            }

            distances.sort_by(|a, b| a.partial_cmp(b).expect("distances finies"));
            let median = distances[distances.len() / 2];
            let aberrant = distances
                .iter()
                .filter(|d| **d > ABERRANT_FACTOR * tol_m)
                .count();
            let ratio = aberrant as f64 / distances.len() as f64;

            assert!(
                median <= tol_m,
                "segment {segment}, emprunt du km {:.2} au km {:.2} : distance médiane de {median:.1} m aux autres emprunts (tolérance {tol_m} m)",
                passage.km_entree,
                passage.km_sortie
            );
            assert!(
                ratio <= MAX_ABERRANT_RATIO,
                "segment {segment}, emprunt du km {:.2} au km {:.2} : {aberrant} points de contrôle sur {} à plus de {:.0} m des autres emprunts",
                passage.km_entree,
                passage.km_sortie,
                distances.len(),
                ABERRANT_FACTOR * tol_m
            );
        }
    }
}

// ─── Menton-Poggio ────────────────────────────────────────────────────

/// Trace longue et réelle : la détection y produit plusieurs tronçons répétés.
#[test]
fn poggio_detection_is_coherent_and_stable() {
    let archive = detect("poggio", "poggio.gpx", &fixture("poggio.gpx"), params())
        .expect("détection de poggio");
    let points = load_points(&fixture("poggio.gpx")).expect("lecture du GPX");
    let (_, sampling) = intermediates(&fixture("poggio.gpx"));

    // La longueur mesurée dans la projection locale doit coïncider avec la
    // longueur orthodromique des points bruts : c'est ce qui valide l'échelle de
    // tous les seuils, exprimés en mètres.
    let geodesic = geodesic_length_km(&points);
    assert!(
        (archive.trace_length_km - geodesic).abs() / geodesic < 0.005,
        "longueur projetée {} km contre {} km en orthodromie",
        archive.trace_length_km,
        geodesic
    );
    assert_eq!(archive.trace_point_count, points.len());

    assert_structurally_sound(&archive, sampling.step);
    assert_passages_mutually_superposed(&archive, &sampling, params().tolerance_m);

    assert_eq!(
        archive.passages.len(),
        8,
        "emprunts détectés : {:#?}",
        archive.passages
    );
}

// ─── CalpePhoto ───────────────────────────────────────────────────────

/// Trace courte : la détection y produit un aller-retour net au milieu du
/// parcours.
#[test]
fn calpe_photo_detection_is_coherent_and_stable() {
    let archive = detect(
        "calpe",
        "CalpePhoto.gpx",
        &fixture("CalpePhoto.gpx"),
        params(),
    )
    .expect("détection de CalpePhoto");
    let points = load_points(&fixture("CalpePhoto.gpx")).expect("lecture du GPX");
    let (_, sampling) = intermediates(&fixture("CalpePhoto.gpx"));

    let geodesic = geodesic_length_km(&points);
    assert!(
        (archive.trace_length_km - geodesic).abs() / geodesic < 0.005,
        "longueur projetée {} km contre {} km en orthodromie",
        archive.trace_length_km,
        geodesic
    );

    assert_structurally_sound(&archive, sampling.step);
    assert_passages_mutually_superposed(&archive, &sampling, params().tolerance_m);

    assert_eq!(
        archive.passages.len(),
        7,
        "emprunts détectés : {:#?}",
        archive.passages
    );
}
