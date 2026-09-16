//! Qualification du sens d'un passage (spécification §4).
//!
//! Les passages d'un même segment sont superposés : reste à savoir si le second
//! est parcouru dans le **même sens** que la référence (`+1`, Aller) ou en sens
//! inverse (`−1`, Retour).
//!
//! Le signe enregistré à l'appariement ne suffit pas — un passage peut être
//! assemblé depuis plusieurs runs aux directions contradictoires, artefact de
//! chaînage en courbe — d'où une requalification **globale**, sur les passages
//! définitifs, par score cumulé de `cos(Δcap)` :
//!
//! - **symétrique** : le signe ne dépend pas du sens de parcours de la référence ;
//! - **continu** : un virage commun aux deux passages pénalise sans changer le
//!   signe, là où un vote majoritaire sur le signe de `Δcap` serait fragile ;
//! - **auto-pondérant** : les appariements les mieux alignés pèsent le plus.
//!
//! Marqueur temporaire `allow(dead_code)` : la qualification du sens est appelée
//! par l'assemblage des segments, porté par la sous-étape suivante — tout le
//! module est donc encore sans appelant. Le marqueur est posé au niveau du
//! **module** parce que c'est le module entier qui est en attente, et non
//! quelques éléments au milieu d'un fichier déjà consommé.

#![allow(dead_code)]

use super::resample::Resample;

/// Nombre minimal d'échantillons de la référence.
const DIR_MIN_SAMPLES: usize = 8;
/// Nombre maximal d'échantillons de la référence.
const DIR_MAX_SAMPLES: usize = 60;
/// Un échantillon tous les `DIR_SAMPLE_PER` points de la référence.
const DIR_SAMPLE_PER: usize = 8;
/// Demi-fenêtre du pointeur glissant, en points.
const DIR_WINDOW: usize = 90;
/// Facteur du filtre de proximité : un échantillon dont le meilleur voisin est
/// au-delà de `1,2 · tol` est ignoré (bords de passage mal alignés).
const DIR_TOL_FACTOR: f64 = 1.2;
/// Nombre de points d'avance pour le cap local.
const BEARING_SPAN: usize = 3;

/// Cap local au point `i`, mesuré sur quelques points d'avance, dans le plan
/// projeté.
///
/// C'est l'équivalent local du `turf.bearing` de la référence : l'écart entre
/// les deux familles de caps est celui de la projection, négligeable à l'échelle
/// d'une trace — et c'est le même plan que celui où sont mesurées les distances.
fn local_bearing(r: &Resample, i: usize) -> f64 {
    let j = (i + BEARING_SPAN).min(r.px.len() - 1);
    (r.px[j] - r.px[i]).atan2(r.py[j] - r.py[i])
}

/// Ramène un écart de caps dans `(−π, π]` — le chemin le plus court.
fn normalize_angle(rad: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    (rad + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI
}

/// Sens du passage `other` relativement au passage de référence `first` :
/// `+1` (même sens) ou `−1` (sens inverse).
///
/// `first` et `other` sont des intervalles **d'indices du rééchantillonnage**,
/// tous deux superposés géométriquement (ils partagent un run).
///
/// Un score nul — aucun échantillon appariable — l'emporte côté « même sens »,
/// comme dans la référence : l'ambiguïté est traitée comme un aller, jamais
/// comme un retour.
pub fn relative_direction(
    r: &Resample,
    first: (usize, usize),
    other: (usize, usize),
    tol_m: f64,
) -> i8 {
    let (s1, e1) = first;
    let (s2, e2) = other;

    let samples = ((e1 - s1) / DIR_SAMPLE_PER).clamp(DIR_MIN_SAMPLES, DIR_MAX_SAMPLES);
    let max_distance2 = (DIR_TOL_FACTOR * tol_m).powi(2);

    let mut score = 0.0;
    let mut cursor = s2;
    for k in 0..samples {
        // Échantillon de la référence, réparti uniformément sur l'intervalle.
        let f = s1 + ((e1 - s1) * k) / (samples - 1);

        // Pointeur glissant : la correspondance géométrique de deux passages
        // superposés est monotone — elle progresse dans le même ordre, y compris
        // en retour, où elle descend l'intervalle `other`. La fenêtre évite
        // qu'un échantillon s'apparie à une portion erronée du passage.
        let lo = s2.max(cursor.saturating_sub(DIR_WINDOW));
        let hi = e2.min(cursor + DIR_WINDOW);
        if lo > hi {
            continue;
        }

        let mut best = lo;
        let mut best_distance2 = f64::INFINITY;
        for q in lo..=hi {
            let dx = r.px[q] - r.px[f];
            let dy = r.py[q] - r.py[f];
            let distance2 = dx * dx + dy * dy;
            if distance2 < best_distance2 {
                best_distance2 = distance2;
                best = q;
            }
        }

        if best_distance2 > max_distance2 {
            continue;
        }
        cursor = best;

        score += normalize_angle(local_bearing(r, best) - local_bearing(r, f)).cos();
    }

    if score >= 0.0 {
        1
    } else {
        -1
    }
}
