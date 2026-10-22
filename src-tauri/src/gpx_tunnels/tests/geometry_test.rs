//! Tests de la géométrie pure du module Tunnels.

use crate::gpx_tunnels::geometry::{
    angle_diff_deg, barycenter, bbox, bbox_contains_point, bbox_intersects, bbox_with_margin,
    bearing_deg, cap_at, cumulative_distances, haversine_m, point_to_segment, project_local,
    segment_cap_deg,
};

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "valeur {actual} attendue ≈ {expected} (±{tolerance})"
    );
}

#[test]
fn haversine_matches_one_degree_of_latitude() {
    // 1° de latitude vaut environ 111,195 km sur le rayon moyen.
    close(haversine_m([0.0, 0.0], [0.0, 1.0]), 111_194.9, 5.0);
    close(haversine_m([2.0, 45.0], [2.0, 45.001]), 111.19, 1.0);
}

#[test]
fn bearing_points_north_and_east() {
    close(bearing_deg([0.0, 0.0], [0.0, 1.0]), 0.0, 0.01);
    close(bearing_deg([0.0, 0.0], [1.0, 0.0]), 90.0, 0.01);
    close(bearing_deg([0.0, 0.0], [0.0, -1.0]), 180.0, 0.01);
    close(bearing_deg([0.0, 0.0], [-1.0, 0.0]), 270.0, 0.01);
}

#[test]
fn cumulative_distances_start_at_zero_and_accumulate() {
    let line = [[0.0, 0.0], [0.0, 0.001], [0.0, 0.002]];
    let cumul = cumulative_distances(&line);
    assert_eq!(cumul[0], 0.0);
    close(cumul[1], 111.19, 1.0);
    close(cumul[2], 222.39, 2.0);
    assert!(cumulative_distances(&[]).is_empty());
}

#[test]
fn barycenter_is_the_mean_of_coordinates() {
    let line = [[2.0, 45.0], [4.0, 47.0]];
    let center = barycenter(&line);
    close(center[0], 3.0, 1e-12);
    close(center[1], 46.0, 1e-12);
    assert_eq!(barycenter(&[]), [0.0, 0.0]);
}

#[test]
fn bbox_and_margin_cover_the_trace() {
    let line = [[2.0, 45.0], [2.001, 45.001]];
    let bounds = bbox(&line);
    close(bounds[0], 2.0, 1e-12);
    close(bounds[1], 45.0, 1e-12);
    close(bounds[2], 2.001, 1e-12);
    close(bounds[3], 45.001, 1e-12);

    let widened = bbox_with_margin(&line, 500.0);
    assert!(widened[0] < bounds[0] && widened[1] < bounds[1]);
    assert!(widened[2] > bounds[2] && widened[3] > bounds[3]);
    // La marge de longitude est corrigée par le cosinus de la latitude.
    let lon_margin = bounds[0] - widened[0];
    let lat_margin = bounds[1] - widened[1];
    assert!(lon_margin > lat_margin);
}

#[test]
fn bbox_intersection_and_containment_include_borders() {
    let a = [0.0, 0.0, 10.0, 10.0];
    assert!(bbox_intersects(a, [5.0, 5.0, 15.0, 15.0]));
    assert!(bbox_intersects(a, [10.0, 10.0, 12.0, 12.0]));
    assert!(!bbox_intersects(a, [11.0, 11.0, 12.0, 12.0]));
    assert!(bbox_contains_point(a, [0.0, 0.0]));
    assert!(bbox_contains_point(a, [10.0, 10.0]));
    assert!(!bbox_contains_point(a, [10.1, 10.0]));
}

#[test]
fn cap_follows_a_straight_line() {
    let east = [[0.0, 0.0], [0.001, 0.0], [0.002, 0.0]];
    let east_cumul = cumulative_distances(&east);
    close(cap_at(&east, &east_cumul, 1, 30.0), 90.0, 0.1);

    let north = [[0.0, 0.0], [0.0, 0.001], [0.0, 0.002]];
    let north_cumul = cumulative_distances(&north);
    close(cap_at(&north, &north_cumul, 1, 30.0), 0.0, 0.1);
}

#[test]
fn cap_falls_back_to_zero_on_a_single_point() {
    let line = [[0.0, 0.0]];
    let cumul = cumulative_distances(&line);
    assert_eq!(cap_at(&line, &cumul, 0, 30.0), 0.0);
}

#[test]
fn local_projection_is_east_north_in_metres() {
    let reference = [2.0, 45.0];
    let (x, y) = project_local([2.0, 45.001], reference);
    close(x, 0.0, 1e-9);
    close(y, 111.19, 1.0);

    let (x, y) = project_local([2.001, 45.0], reference);
    close(x, 78.63, 1.0);
    close(y, 0.0, 1e-9);
}

#[test]
fn point_to_segment_handles_projection_and_endpoints() {
    // Projection orthogonale.
    close(point_to_segment(5.0, 5.0, 0.0, 0.0, 10.0, 0.0), 5.0, 1e-9);
    // Au-delà de l'extrémité : distance au sommet.
    close(point_to_segment(-5.0, 0.0, 0.0, 0.0, 10.0, 0.0), 5.0, 1e-9);
    // Segment dégénéré.
    close(point_to_segment(3.0, 4.0, 0.0, 0.0, 0.0, 0.0), 5.0, 1e-9);
}

#[test]
fn angle_diff_wraps_around_north() {
    close(angle_diff_deg(350.0, 10.0), 20.0, 1e-9);
    close(angle_diff_deg(10.0, 350.0), 20.0, 1e-9);
    close(angle_diff_deg(90.0, 270.0), 180.0, 1e-9);
    close(angle_diff_deg(0.0, 0.0), 0.0, 1e-9);
}

#[test]
fn segment_cap_is_north_east_south_west() {
    close(segment_cap_deg(0.0, 0.0, 0.0, 10.0), 0.0, 1e-9);
    close(segment_cap_deg(0.0, 0.0, 10.0, 0.0), 90.0, 1e-9);
    close(segment_cap_deg(0.0, 0.0, 0.0, -10.0), 180.0, 1e-9);
    close(segment_cap_deg(0.0, 0.0, -10.0, 0.0), 270.0, 1e-9);
}
