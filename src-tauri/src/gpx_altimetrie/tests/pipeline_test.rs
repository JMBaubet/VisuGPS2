//! Tests du pipeline altimétrique : profils synthétiques, helpers de
//! prétraitement et trace réelle de fixture.

use std::f64::consts::PI;
use std::fs::File;
use std::io::BufReader;

use crate::gpx_altimetrie::pipeline::{
    anchored_moving_average, despike_altitude, detect, plateau_correct, RawPoint,
};
use crate::gpx_altimetrie::types::{AltimetrieFile, AltimetrieParams, FILE_VERSION};

const LAT0: f64 = 45.0;
const LON0: f64 = 6.0;
const EARTH_RADIUS_M: f64 = 6_371_000.0;

fn point_at(x_m: f64, ele: f64) -> RawPoint {
    let meters_per_degree = EARTH_RADIUS_M * LAT0.to_radians().cos() * PI / 180.0;
    RawPoint {
        lat: LAT0,
        lon: LON0 + x_m / meters_per_degree,
        ele,
    }
}

fn track(total_m: f64, step_m: f64, elevation: impl Fn(f64) -> f64) -> Vec<RawPoint> {
    let count = (total_m / step_m).floor() as usize;
    (0..=count)
        .map(|i| {
            let x = i as f64 * step_m;
            point_at(x, elevation(x))
        })
        .collect()
}

fn detect_with(points: &[RawPoint], params: &AltimetrieParams) -> AltimetrieFile {
    detect(
        points,
        params,
        "trace-test",
        "trace.gpx",
        "2026-10-19T12:00:00Z",
    )
    .expect("détection altimétrique")
}

fn detect_default(points: &[RawPoint]) -> AltimetrieFile {
    detect_with(points, &AltimetrieParams::default())
}

/// Approche descendante, montée de 2 km à 5 %, puis descente : le sommet est
/// explicite et la montée ne se prolonge pas dans un replat.
fn single_climb_elevation(x: f64) -> f64 {
    if x < 1_000.0 {
        1_100.0 - x * 0.1
    } else if x < 3_000.0 {
        1_000.0 + (x - 1_000.0) * 0.05
    } else {
        1_100.0 - (x - 3_000.0) * 0.02
    }
}

/// Deux montées à 6 %, séparées par une descente de 1,6 km à −2 %. La seconde
/// est plus haute, ce qui permet à la fusion de produire une montée globale
/// cohérente avec deux sous-montées.
fn two_climbs_elevation(x: f64) -> f64 {
    if x < 1_000.0 {
        1_000.0
    } else if x < 2_000.0 {
        1_000.0 + (x - 1_000.0) * 0.06
    } else if x < 3_600.0 {
        1_060.0 - (x - 2_000.0) * 0.02
    } else if x < 4_600.0 {
        1_028.0 + (x - 3_600.0) * 0.06
    } else {
        1_088.0
    }
}

#[test]
fn flat_profile_has_no_climb_and_one_plateau_segment() {
    let points = track(5_000.0, 10.0, |_| 1_000.0);
    let file = detect_default(&points);

    assert!(file.climbs.is_empty(), "aucune montée attendue : {:?}", file.climbs);
    assert!(!file.segments.is_empty());
    assert!(file.segments.iter().all(|segment| segment.category_id == "pl"));
    assert!(file.segments.iter().all(|segment| segment.d_plus_m == 0.0));
    assert_eq!(file.version, FILE_VERSION);
    assert_eq!(file.trace_id, "trace-test");
    assert!(file.profile.len() >= 2);
    assert!((file.trace.total_distance_m - 5_000.0).abs() < 20.0);
}

#[test]
fn single_climb_is_detected_with_coherent_metrics() {
    let points = track(5_000.0, 10.0, single_climb_elevation);
    let file = detect_default(&points);

    assert_eq!(file.climbs.len(), 1, "une montée attendue : {:?}", file.climbs);
    let climb = &file.climbs[0];
    let length = climb.end_km - climb.start_km;

    assert!(length > 1.5 && length < 2.5, "longueur {length}");
    assert!(
        climb.d_plus_m > 80.0 && climb.d_plus_m < 120.0,
        "D+ {}",
        climb.d_plus_m
    );
    assert!(
        climb.avg_slope_pct > 3.5 && climb.avg_slope_pct < 6.5,
        "pente {}",
        climb.avg_slope_pct
    );
    assert!(!climb.is_merged);
    assert!(!climb.is_separated);
    assert!(file
        .segments
        .iter()
        .any(|segment| segment.category_id == "up1" || segment.category_id == "up2"));
}

/// Un sommet étroit à l'arrivée d'une montée ne doit pas produire un pic
/// d'un point dans le profil affiché : les bornes ne sont plus ancrées sur
/// l'altitude brute.
#[test]
fn climb_boundaries_are_not_anchored_on_raw_altitude() {
    fn narrow_summit(x: f64) -> f64 {
        if x < 1_000.0 {
            900.0
        } else if x < 3_000.0 {
            900.0 + (x - 1_000.0) * 0.05
        } else if x < 3_150.0 {
            1_000.0 + (x - 3_000.0) * 0.4
        } else if x < 3_300.0 {
            1_060.0 - (x - 3_150.0) * 0.4
        } else {
            1_000.0 - (x - 3_300.0) * 0.05
        }
    }

    let points = track(6_000.0, 10.0, narrow_summit);
    let file = detect_default(&points);

    assert!(!file.climbs.is_empty(), "une montée attendue");
    for climb in &file.climbs {
        let start_m = climb.start_km * 1_000.0;
        let end_m = climb.end_km * 1_000.0;
        for pair in file.profile.windows(2) {
            if pair[1].d < start_m || pair[0].d > end_m + 50.0 {
                continue;
            }
            let jump = (pair[1].ele - pair[0].ele).abs();
            assert!(
                jump < 10.0,
                "saut de {jump:.1} m entre {} et {} m",
                pair[0].d,
                pair[1].d
            );
        }
    }
}

#[test]
fn separated_climbs_are_merged_when_the_gap_is_below_the_threshold() {
    let points = track(6_000.0, 10.0, two_climbs_elevation);
    let params = AltimetrieParams {
        ecart_fusion: 2_000.0,
        ..AltimetrieParams::default()
    };
    let file = detect_with(&points, &params);

    let merged = file
        .climbs
        .iter()
        .find(|climb| climb.is_merged)
        .expect("une montée fusionnée attendue");
    let parts = merged
        .merged_parts
        .as_ref()
        .expect("les sous-montées doivent être conservées");

    assert!(parts.len() >= 2, "sous-montées : {parts:?}");
    assert_eq!(merged.merged_count as usize, parts.len() - 1);
    assert!(parts.iter().all(|part| part.end_km > part.start_km));
}

#[test]
fn segments_cover_the_whole_sampled_trace() {
    let points = track(5_000.0, 10.0, single_climb_elevation);
    let file = detect_default(&points);

    assert!(!file.segments.is_empty());
    assert_eq!(file.segments[0].start_km, 0.0);

    let last = file.segments.last().expect("au moins un segment");
    let sampled_km = file.trace.total_distance_sampled_m / 1_000.0;
    assert!(
        (last.end_km - sampled_km).abs() < 1e-3,
        "fin {} vs {}",
        last.end_km,
        sampled_km
    );

    for pair in file.segments.windows(2) {
        assert!(
            (pair[0].end_km - pair[1].start_km).abs() < 1e-6,
            "trou entre {:?} et {:?}",
            pair[0],
            pair[1]
        );
        assert!(pair[1].end_km >= pair[0].end_km);
    }
    assert!(file
        .segments
        .iter()
        .all(|segment| segment.end_km >= segment.start_km));
}

#[test]
fn anchored_smoothing_preserves_anchor_values() {
    let raw: Vec<f64> = (0..100)
        .map(|i| {
            let x = i as f64;
            1_000.0 + (x * 0.5).sin() * 10.0 + x * 0.2
        })
        .collect();
    let anchors = vec![10usize, 50, 90];
    let out = anchored_moving_average(&raw, &anchors, 10.0, 100.0, 20.0, true);

    for &anchor in &[0usize, 10, 50, 90, 99] {
        assert!(
            (out[anchor] - raw[anchor]).abs() < 1e-9,
            "ancre {anchor} : {} vs {}",
            out[anchor],
            raw[anchor]
        );
    }
}

#[test]
fn plateau_correction_interpolates_an_internal_flat_run() {
    let ele = vec![
        1_000.0, 1_002.0, 1_002.0, 1_002.0, 1_002.0, 1_002.0, 1_002.0, 1_002.0, 1_004.0,
    ];
    let (out, count) = plateau_correct(&ele, 6, 1.0);

    assert_eq!(count, 1);
    assert!((out[1] - 1_000.5).abs() < 1e-9);
    assert!((out[7] - 1_003.5).abs() < 1e-9);
}

#[test]
fn despike_replaces_an_isolated_altitude_pic() {
    let mut ele = vec![1_000.0; 20];
    ele[10] = 1_050.0;

    let (out, count) = despike_altitude(&ele, 5, 15.0);

    assert!(count >= 1);
    assert!((out[10] - 1_000.0).abs() < 1e-9);
    assert!(out.iter().all(|value| (value - 1_000.0).abs() < 1e-9));
}

#[test]
fn poggio_fixture_produces_a_coherent_file() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/gpx_multiride/tests/fixtures/poggio.gpx"
    );
    let file = File::open(path).expect("fixture poggio.gpx");
    let gpx = gpx::read(BufReader::new(file)).expect("GPX poggio");

    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for waypoint in &segment.points {
                if let Some(ele) = waypoint.elevation {
                    let point = waypoint.point();
                    points.push(RawPoint {
                        lat: point.y(),
                        lon: point.x(),
                        ele,
                    });
                }
            }
        }
    }
    assert!(points.len() > 100, "fixture trop courte");

    let file = detect(
        &points,
        &AltimetrieParams::default(),
        "poggio",
        "poggio.gpx",
        "2026-10-19T12:00:00Z",
    )
    .expect("détection poggio");

    assert_eq!(file.version, FILE_VERSION);
    assert!(file.profile.len() >= 2);
    assert!(!file.segments.is_empty());
    assert_eq!(file.trace.total_points, file.profile.len());
    assert!(file.profile.windows(2).all(|pair| pair[1].d >= pair[0].d));

    for climb in &file.climbs {
        assert!(climb.end_km > climb.start_km);
        assert!(climb.d_plus_m >= 0.0);
        assert!(climb.avg_slope_pct.is_finite());
    }
    for pair in file.climbs.windows(2) {
        assert!(
            pair[1].start_km >= pair[0].end_km - 1e-6,
            "montées chevauchantes : {:?} et {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn invalid_inputs_are_rejected() {
    let params = AltimetrieParams::default();

    assert!(detect(&[], &params, "t", "t.gpx", "now").is_err());

    let one = vec![RawPoint {
        lat: LAT0,
        lon: LON0,
        ele: 1_000.0,
    }];
    assert!(detect(&one, &params, "t", "t.gpx", "now").is_err());

    let same = vec![
        RawPoint {
            lat: LAT0,
            lon: LON0,
            ele: 1_000.0,
        };
        3
    ];
    assert!(detect(&same, &params, "t", "t.gpx", "now").is_err());

    let nan = vec![
        RawPoint {
            lat: LAT0,
            lon: LON0,
            ele: f64::NAN,
        },
        RawPoint {
            lat: LAT0,
            lon: LON0 + 0.001,
            ele: 1_000.0,
        },
    ];
    assert!(detect(&nan, &params, "t", "t.gpx", "now").is_err());
}
