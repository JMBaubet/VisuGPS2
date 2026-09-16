//! Détection des correspondances et chaînage en runs.
//!
//! Troisième étape du pipeline (spécification §3) : trouver les couples
//! d'intervalles de trace qui décrivent le **même tronçon géométrique**, à la
//! tolérance près — le premier avant le second, dans le même sens ou en sens
//! opposé. Un index spatial en grille évite le test naïf en O(N²), trois filtres
//! ponctuels écartent les fausses pistes, et un chaînage directionnel assemble
//! les correspondances isolées en runs **maximaux** : l'unité de travail de
//! l'assemblage en segments.
//!
//! Une correspondance ponctuelle n'est pas une détection — il faut des suites
//! contiguës pour délimiter des intervalles. Le chaînage étend un couple en
//! parallèle, à direction relative constante, ce qui traite uniformément l'aller
//! futur `(i, j) → (i+1, j+1)` et le retour `(i, j) → (i+1, j−1)`.

use std::collections::HashMap;

use super::resample::Resample;

/// Fenêtre symétrique de calcul des tangentes locales : `T[i] = P[i+2] − P[i−2]`.
const TANGENT_SPAN: isize = 2;
/// Nombre de correspondances manquantes consécutives tolérées dans le chaînage.
/// Trois points couvrent les micro-ruptures d'appariement en courbe serrée, sans
/// risquer de ponter deux tronçons distincts.
const MAXGAP: usize = 3;
/// Seuil de rejet des croisements : seules les tangentes quasi perpendiculaires
/// (`|cos| ≤ 0,12`, soit un angle entre ~83° et ~97°) sont écartées. Ces
/// croisements de la trace avec elle-même sont hors périmètre.
const PERPENDICULAR_COS: f64 = 0.12;
/// Séparation temporelle minimale : la moitié de la longueur minimale…
const MIN_SEP_FACTOR: f64 = 0.5;
/// …avec un plancher de quatre pas d'échantillonnage.
const MIN_SEP_STEPS: f64 = 4.0;

/// Couple d'intervalles superposés, chaîné de façon maximale.
///
/// Les bornes sont des **indices du rééchantillonnage**. Le côté B est normalisé
/// (`b0 ≤ b1`) quel que soit le sens du parcours : les étapes en aval ne
/// raisonnent qu'en intervalles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// Intervalle « première occurrence » (côté A).
    pub a0: usize,
    pub a1: usize,
    /// Intervalle « seconde occurrence » (côté B).
    pub b0: usize,
    pub b1: usize,
    /// Sens relatif local : `+1` (même sens) ou `−1` (sens opposé).
    pub dir: i8,
}

/// Correspondance ponctuelle : sens relatif local et marque de consommation.
#[derive(Debug, Clone, Copy)]
struct Match {
    dir: i8,
    visited: bool,
}

/// Cellule de la grille spatiale d'un point échantillonné.
fn cell_of(r: &Resample, index: usize, cell: f64) -> (i64, i64) {
    (
        (r.px[index] / cell).floor() as i64,
        (r.py[index] / cell).floor() as i64,
    )
}

/// Tangentes locales précalculées, bornées aux extrémités.
///
/// Le produit scalaire de deux tangentes **non normalisé en valeur absolue**
/// accepte deux configurations — tangentes de même sens (aller futur) et de sens
/// opposés (retour) — et ne rejette que les croisements.
fn tangents(r: &Resample) -> Vec<(f64, f64)> {
    let last = (r.px.len() - 1) as isize;
    (0..r.px.len())
        .map(|i| {
            let hi = (i as isize + TANGENT_SPAN).clamp(0, last) as usize;
            let lo = (i as isize - TANGENT_SPAN).clamp(0, last) as usize;
            (r.px[hi] - r.px[lo], r.py[hi] - r.py[lo])
        })
        .collect()
}

/// Successeur d'une correspondance : `(i + d, j + d·dir)`.
fn advance(i: usize, j: usize, d: usize, dir: i8, count: usize) -> Option<(usize, usize)> {
    let ni = i + d;
    let nj = j as isize + (d as isize) * (dir as isize);
    if ni < count && (0..count as isize).contains(&nj) {
        Some((ni, nj as usize))
    } else {
        None
    }
}

/// Prédécesseur d'une correspondance : `(i − d, j − d·dir)`.
fn retreat(i: usize, j: usize, d: usize, dir: i8, count: usize) -> Option<(usize, usize)> {
    let ni = i.checked_sub(d)?;
    let nj = j as isize - (d as isize) * (dir as isize);
    if (0..count as isize).contains(&nj) {
        Some((ni, nj as usize))
    } else {
        None
    }
}

/// Détecte les runs de la trace échantillonnée.
///
/// `tol_m` est la tolérance de superposition et `min_len_m` la longueur minimale
/// d'une portion répétée signalée.
pub fn find_runs(r: &Resample, tol_m: f64, min_len_m: f64) -> Result<Vec<Run>, String> {
    if !(tol_m > 0.0) {
        return Err(format!("Tolérance de superposition invalide : {} m.", tol_m));
    }
    if !(min_len_m > 0.0) {
        return Err(format!("Longueur minimale invalide : {} m.", min_len_m));
    }
    let count = r.px.len();
    if count < 2 {
        return Err("Trace échantillonnée vide.".to_string());
    }

    let tangents = tangents(r);
    let min_sep = (min_len_m * MIN_SEP_FACTOR).max(MIN_SEP_STEPS * r.step);
    let tol2 = tol_m * tol_m;
    let cell = tol_m.max(1.5 * r.step);

    // Index spatial : la cellule vaut au moins la tolérance, si bien que deux
    // points distants de `tol` au plus tombent nécessairement dans des cellules
    // voisines — examiner les 9 cellules autour du point courant suffit donc à
    // ne manquer aucune correspondance.
    let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
    for i in 0..count {
        grid.entry(cell_of(r, i, cell)).or_default().push(i as u32);
    }

    // Correspondances ponctuelles, du filtre le moins cher au plus cher.
    let mut matches: HashMap<(u32, u32), Match> = HashMap::new();
    for i in 0..count {
        let (cx, cy) = cell_of(r, i, cell);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bucket) = grid.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &j in bucket {
                    let j = j as usize;
                    if j <= i {
                        continue;
                    }
                    // F1 — séparation temporelle. Sans elle, chaque point
                    // s'apparierait à ses voisins immédiats : la trace est
                    // trivialement proche d'elle-même à quelques points près.
                    if r.cum[j] - r.cum[i] < min_sep {
                        continue;
                    }
                    // F2 — proximité spatiale.
                    let ddx = r.px[j] - r.px[i];
                    let ddy = r.py[j] - r.py[i];
                    if ddx * ddx + ddy * ddy > tol2 {
                        continue;
                    }
                    // F3 — alignement local des tangentes.
                    let (tix, tiy) = tangents[i];
                    let (tjx, tjy) = tangents[j];
                    let dot = tix * tjx + tiy * tjy;
                    let scale = tix.hypot(tiy) * tjx.hypot(tjy);
                    if dot.abs() <= PERPENDICULAR_COS * scale {
                        continue;
                    }
                    matches.insert(
                        (i as u32, j as u32),
                        Match {
                            dir: if dot > 0.0 { 1 } else { -1 },
                            visited: false,
                        },
                    );
                }
            }
        }
    }

    // Ordre de parcours déterministe : la table est un `HashMap`, dont l'ordre
    // d'itération ne l'est pas, et deux exécutions sur la même trace doivent
    // produire exactement les mêmes runs.
    let mut keys: Vec<(u32, u32)> = matches.keys().copied().collect();
    keys.sort_unstable();

    let mut runs: Vec<Run> = Vec::new();
    for key in keys {
        let start_dir = match matches.get(&key) {
            Some(m) if !m.visited => m.dir,
            _ => continue,
        };
        let (mut ci, mut cj) = (key.0 as usize, key.1 as usize);

        // Recul : rejoindre l'origine du run, quel que soit le point d'entrée
        // dans la table.
        loop {
            let mut moved = false;
            for d in 1..=MAXGAP {
                if let Some((ni, nj)) = retreat(ci, cj, d, start_dir, count) {
                    if matches
                        .get(&(ni as u32, nj as u32))
                        .is_some_and(|m| !m.visited && m.dir == start_dir)
                    {
                        ci = ni;
                        cj = nj;
                        moved = true;
                        break;
                    }
                }
            }
            if !moved {
                break;
            }
        }

        // Extension : consommer le run et le marquer visité — une correspondance
        // n'appartient qu'à un seul run.
        let (ai0, bj0) = (ci, cj);
        let (mut ai1, mut bj1) = (ci, cj);
        loop {
            if let Some(m) = matches.get_mut(&(ai1 as u32, bj1 as u32)) {
                m.visited = true;
            }
            let mut moved = false;
            for d in 1..=MAXGAP {
                if let Some((ni, nj)) = advance(ai1, bj1, d, start_dir, count) {
                    if matches
                        .get(&(ni as u32, nj as u32))
                        .is_some_and(|m| !m.visited && m.dir == start_dir)
                    {
                        ai1 = ni;
                        bj1 = nj;
                        moved = true;
                        break;
                    }
                }
            }
            if !moved {
                break;
            }
        }

        // Longueurs mesurées le long de la trace (mètres réels, pas d'indices).
        let len_a = r.cum[ai1] - r.cum[ai0];
        let (b_min, b_max) = (bj0.min(bj1), bj0.max(bj1));
        let len_b = r.cum[b_max] - r.cum[b_min];
        if len_a.min(len_b) >= min_len_m {
            runs.push(Run {
                a0: ai0,
                a1: ai1,
                b0: b_min,
                b1: b_max,
                dir: start_dir,
            });
        }
    }

    // Ordre stable (côté A, puis côté B) : l'assemblage trie déjà ses entrées,
    // mais deux exécutions doivent rendre le même vecteur.
    runs.sort_unstable_by_key(|run| (run.a0, run.b0, run.a1, run.b1));
    Ok(runs)
}
