//! Détecteur RP — boucles de giratoire (270°, 360° ou plus).
//!
//! Portage fidèle de `detectRP` du HTML de référence
//! (`docs/audit/reference/verifgpx-V3.0.html`), spécification normative
//! `docs/audit/spec/ANALYSE.md` §6 à §12.

use std::collections::{HashMap, HashSet};

use super::anchor::rp_anchor_indices;
use super::types::{
    AuditParams, AuditPoint, Finding, FindingContext, FindingContextIds, FindingKind, FindingPair,
    FindingPart, FindingStatus, PartRole,
};

// ─── Constantes ───────────────────────────────────────────────────────

/// Pas d'échantillonnage par défaut (m). **Ne pas exposer** : couplage dur
/// avec `p-close` (le pas doit rester ≪ `p-close`, sinon deux échantillons
/// consécutifs peuvent sauter le couloir de refermeture) — ANALYSE §3.1/§3.2.
pub const RP_STEP_DEFAULT: f64 = 4.0;

/// Plafond d'échantillons (garde-fou performance). Garantit également
/// `k < 10⁶`, donc la sûreté des clés de dédoublonnage `m·10⁶ + k` de l'É2
/// (ANALYSE §3.2 / §9.2).
pub const RP_MAX_SAMPLES: usize = 120_000;

/// Séparation minimale entre les deux points d'une paire, en échantillons
/// (≈ 24 m de trace) : élimine les paires quasi contiguës (ANALYSE §3.2).
pub const RP_MIN_SEP: usize = 6;

/// Portée maximale d'une boucle recherchée (m) — plafond d'écart entre deux
/// points appariés (ANALYSE §3.2).
pub const RP_MAX_GAP_M: f64 = 4000.0;

/// Borne de sécurité du nombre de paires brutes de l'É1 (ANALYSE §3.2).
pub const RP_MAX_PAIRS: usize = 40_000;

/// Borne de sécurité du nombre de tests de croisement de l'É2 (ANALYSE §3.2).
pub const RP_MAX_TESTS: usize = 1_500_000;

/// Seuil de colinéarité du contact quasi exact de `segs_cross` (m²) : une
/// superposition au centimètre, qui évite les faux contacts entre voies
/// distinctes d'un giratoire (ANALYSE §9.1).
pub const RP_EPS_CROSS: f64 = 1e-7;

/// Périmètre maximal d'un cœur de boucle (m) : exclut les méga-paires des
/// aller-retours macroscopiques (ANALYSE §3.2 / §11.2).
pub const RP_LMAX: f64 = 800.0;

/// Circularité minimale du polygone fermé (cercle parfait = 1) — test 1 de
/// l'anti-aiguille (ANALYSE §11.4).
pub const RP_CIRC_MIN: f64 = 0.10;

/// Fraction miroir maximale — test 2 de l'anti-aiguille (ANALYSE §11.4).
pub const RP_MIRROR_MAX: f64 = 0.95;

/// Bornes basse et haute d'éligibilité du coin de refermeture (°) : vrais
/// demi-tours de réengagement uniquement (ANALYSE §11.2/§11.3).
pub const RP_COIN_LO: f64 = 130.0;
pub const RP_COIN_HI: f64 = 240.0;

/// Écart minimal des caps pour déclarer une fenêtre « propre » (°)
/// (ANALYSE §11.5).
pub const RP_PROPRE_DEG: f64 = 60.0;

/// Tolérance d'alignement sur un multiple de 360° (°) : la refermeture d'un
/// tour complet se produit à l'angle vif de l'entrée, d'où une oscillation
/// 320–400° bruts (ANALYSE §11.2).
pub const RP_QUANT_TOL: f64 = 40.0;

/// Portée de cap de `rp_walk` (JS `rpWalk(R, from, dir, minM = 8, maxSeg = 4)`).
pub const RP_WALK_MIN_M: f64 = 8.0;
pub const RP_WALK_MAX_SEG: usize = 4;

/// Nombre maximal de paires restituées par finding (les plus serrées) —
/// ANALYSE §12.2.
pub const RP_PAIRS_TOP: usize = 6;

/// Largeur cible des portions approche/sortie publiées autour du cœur (m) —
/// ANALYSE §12 / §4.2.
pub const RP_ZONE_M: f64 = 80.0;

/// Garde d'entrée : trace dégénérée → aucun finding (ANALYSE §3.2).
pub const RP_MIN_SAMPLES: usize = 12;

// ─── Constantes des ancres (ANALYSE §13) ──────────────────────────────

/// δ absolu du disque élargi (m) : bruit GPS + demi-chaussée, ne croît pas
/// avec le rayon.
pub const RP_ANCHOR_DELTA_ABS: f64 = 12.0;

/// δ relatif du disque élargi : imperfection de forme, croît avec le rayon.
pub const RP_ANCHOR_DELTA_K: f64 = 0.4;

/// Marge d'extension après le plancher (m) : l'ancre marque la FRONTIÈRE de la
/// zone anormale, pas le milieu de la route franche.
pub const RP_ANCHOR_EXT_M: f64 = 15.0;

/// Portée minimale de recherche hors zone (m) ; portée effective =
/// `max(RP_ANCHOR_SEARCH_BASE, 2,5·r)`.
pub const RP_ANCHOR_SEARCH_BASE: f64 = 150.0;

/// Plafond global dur de la marche (m), toutes zones confondues (sécurité).
pub const RP_ANCHOR_HARD_M: f64 = 2000.0;

/// Points consécutifs hors zone confirmant la sortie.
pub const RP_ANCHOR_PERSIST: usize = 2;

/// Échantillonnage maximal du cœur (points) — Kåsa + tests de superposition.
pub const RP_ANCHOR_MAX_SAMPLES: usize = 200;

// ─── Structure de la trace rééchantillonnée (ANALYSE §7) ──────────────

/// Trace rééchantillonnée à pas constant (ANALYSE §7).
///
/// Le rééchantillonnage à pas constant rend les seuils métriques
/// indépendants de la densité du GPX d'entrée : un fichier à 1 point/10 m
/// et un fichier à 10 points/1 m produisent la même structure `R`.
///
/// `n` désigne le nombre d'**intervalles** : `R` porte `n + 1` échantillons
/// (indices 0..=n).
pub struct ResampledGeo {
    pub n: usize,         // nombre d'intervalles (n+1 échantillons)
    pub rx: Vec<f64>,     // coordonnées métriques X
    pub ry: Vec<f64>,     // coordonnées métriques Y
    pub cum_r: Vec<f64>,  // distance cumulée le long des échantillons
    pub orig: Vec<usize>, // R i -> index de référence (pivot du contrat)
    pub step: f64,        // pas effectif (m)
}

// ─── Étape 1 — rééchantillonnage ──────────────────────────────────────

/// Rééchantillonnage à pas constant (ANALYSE §7, JS `resampleGeo`).
///
/// `px`/`py`/`cum` décrivent la géométrie de la trace consolidée (`m` points)
/// et `total` vaut `cum[m−1]`. `step` est le pas demandé (m) ; le pas
/// **effectif** vaut `total / n`, ce qui garantit que le dernier échantillon
/// tombe exactement sur `total`.
///
/// Les échantillons sont produits par interpolation linéaire le long du
/// segment hôte ; `orig[i]` mémorise l'indice de référence qui héberge
/// l'échantillon `i` (dernier échantillon : `m − 1`).
pub fn resample_geo(
    px: &[f64],
    py: &[f64],
    cum: &[f64],
    total: f64,
    step: f64,
) -> ResampledGeo {
    let m = px.len();

    // Nombre d'intervalles : max(1, round(total / step)), plafonné à
    // RP_MAX_SAMPLES. La garde de finitude remplace le comportement JS sur
    // entrée dégénérée (round(NaN) propagerait NaN).
    let mut n_f = (total / step).round();
    if !n_f.is_finite() || n_f < 1.0 {
        n_f = 1.0;
    }
    let mut n = n_f as usize;
    if n > RP_MAX_SAMPLES {
        n = RP_MAX_SAMPLES;
    }
    let eff = total / n as f64;

    // Trace dégénérée (moins de deux points, ou tableaux désalignés) : aucun
    // segment hôte n'existe. Le JS produirait des échantillons NaN ; on
    // retourne une structure vide, rejetée par la garde d'entrée de
    // `detect_rp` (n < RP_MIN_SAMPLES) — comportement observable identique.
    if m < 2 || py.len() < 2 || cum.len() < 2 {
        return ResampledGeo {
            n: 0,
            rx: Vec::new(),
            ry: Vec::new(),
            cum_r: Vec::new(),
            orig: Vec::new(),
            step: eff,
        };
    }

    let mut rx = vec![0.0; n + 1];
    let mut ry = vec![0.0; n + 1];
    let mut cum_r = vec![0.0; n + 1];
    let mut orig = vec![0usize; n + 1];
    let mut k = 0usize;

    for i in 0..=n {
        let d = (i as f64 * eff).min(total);
        // Segment hôte de la cible : on avance tant que la cible dépasse la
        // fin du segment courant. `k + 2 < m` reproduit `k < m − 2` sans
        // soustraction non signée (le dernier segment n'est jamais hôte :
        // la cible 0 du premier échantillon reste sur le segment 0).
        while k + 2 < m && cum[k + 1] < d {
            k += 1;
        }
        let seg_len = cum[k + 1] - cum[k];
        let t = if seg_len > 0.0 {
            (d - cum[k]) / seg_len
        } else {
            0.0
        };
        rx[i] = px[k] + t * (px[k + 1] - px[k]);
        ry[i] = py[k] + t * (py[k + 1] - py[k]);
        cum_r[i] = if i == 0 {
            0.0
        } else {
            cum_r[i - 1] + (rx[i] - rx[i - 1]).hypot(ry[i] - ry[i - 1])
        };
        orig[i] = if i == n { m - 1 } else { k };
    }

    ResampledGeo {
        n,
        rx,
        ry,
        cum_r,
        orig,
        step: eff,
    }
}

// ─── Étape 2 — helpers angulaires ─────────────────────────────────────

/// Normalisation d'un angle dans ]−180, +180] (ANALYSE §8.1, JS `rpNorm180`).
///
/// La constante `540` recentre le reste de la division par 360 — qui, en
/// virgule flottante, conserve le signe du dividende — avant de retrancher
/// 180 ; `−180` est ramené sur `+180` pour rendre la borne supérieure
/// inclusive.
pub(super) fn rp_norm_180(a: f64) -> f64 {
    let x = ((a % 360.0) + 540.0) % 360.0 - 180.0;
    if x <= -180.0 {
        180.0
    } else {
        x
    }
}

/// Traduction humaine de la rotation cumulée (ANALYSE §12.2, JS `turnText`).
///
/// Les gabarits et les seuils sont reproduits à l'identique, y compris les
/// pluriels figés du JS (ex. `391°` → « 1 tours complets »).
pub fn turn_text(total: f64) -> String {
    let k = (total / 360.0).round();
    if k >= 1.0 && (total - 360.0 * k).abs() <= 30.0 {
        return if k == 1.0 {
            "tour complet (1×)".to_string()
        } else {
            format!("{} tours complets", k)
        };
    }
    if total < 240.0 {
        return "demi-tour dépassé".to_string();
    }
    let base = (total / 360.0).floor();
    let q = ((total - 360.0 * base) / 90.0).round();
    // Le JS indexe un tableau littéral et retombe sur `''` hors plage : le
    // `get`/`unwrap_or` reproduit ce repli (aucun accès non borné).
    let quart = ["", "1/4 de tour", "1/2 tour", "3/4 de tour", "tour complet"]
        .get(q as usize)
        .copied()
        .unwrap_or("");
    if base == 0.0 {
        return if quart.is_empty() {
            format!("{}°", total.round())
        } else {
            quart.to_string()
        };
    }
    if q == 0.0 {
        return format!("{} tours complets", base);
    }
    let s = if base > 1.0 { "s" } else { "" };
    format!("{} tour{} complet{} + {}", base, s, s, quart)
}

// ─── Étape 3 — caps lissés et cumul angulaire ────────────────────────

/// Étape 3 — caps lissés (±1 échantillon) et cumul angulaire signé
/// (ANALYSE §8.1, JS `detectRP` / crochet `angleBetween`).
///
/// Le cap de l'échantillon `k` est mesuré sur la **fenêtre centrale**
/// `R(k−1) → R(k+1)` (lissage qui écrête le bruit GPS) ; le cumul `pre` est la
/// somme **signée** des variations de cap normalisées dans ]−180, +180], si
/// bien que `angle_between(pre, i, j) = pre[j] − pre[i]` donne la rotation
/// signée cumulée en O(1). C'est ce signe qui distingue une boucle (+360°)
/// d'un enchaînement de virages en S (somme nulle).
///
/// Les échantillons 0 et `n` restent sans cap (comme dans le JS) et l'écart
/// entre deux caps indéfinis vaut 0 ; le dernier intervalle n'est pas cumulé
/// (`pre[n] = pre[n−1]`).
pub(super) fn compute_heading_prefix(r: &ResampledGeo) -> Vec<f64> {
    let n = r.n;
    // Entrée dégénérée : préfixe nul (aucun accès hors tableau).
    if n < 2 || r.rx.len() < n + 1 || r.ry.len() < n + 1 {
        return vec![0.0; n + 1];
    }

    // Caps lissés — les indices 0 et n restent indéfinis (NaN).
    let mut heads = vec![f64::NAN; n + 1];
    for k in 1..=n - 1 {
        let dx = r.rx[k + 1] - r.rx[k - 1];
        let dy = r.ry[k + 1] - r.ry[k - 1];
        if dx.hypot(dy) > 1e-9 {
            heads[k] = dy.atan2(dx) * 180.0 / std::f64::consts::PI;
        }
    }

    // pre[0] = pre[1] = 0 (valeur initiale du vecteur).
    let mut pre = vec![0.0; n + 1];
    for k in 2..=n - 1 {
        let mut d = heads[k] - heads[k - 1];
        if !d.is_finite() {
            // Cap indéfini d'un côté ou de l'autre : écart neutre.
            d = 0.0;
        } else if d > 180.0 {
            d -= 360.0;
        } else if d < -180.0 {
            d += 360.0;
        }
        pre[k] = pre[k - 1] + d;
    }
    pre[n] = pre[n - 1];

    pre
}

/// Rotation signée cumulée entre deux échantillons (ANALYSE §8.1, JS
/// `angleBetween`). Hors bornes, le JS propagerait `NaN` : on retourne 0,0.
pub(super) fn angle_between(pre: &[f64], i: usize, j: usize) -> f64 {
    if i >= pre.len() || j >= pre.len() {
        return 0.0;
    }
    pre[j] - pre[i]
}

// ─── Étape 4 — É1 : paires par proximité spatiale ────────────────────

/// É1 — paires par proximité spatiale (ANALYSE §8.2).
///
/// Une boucle candidate est un couple `(j, i)`, `j < i`, distant de moins de
/// `close_thr` avec `RP_MIN_SEP ≤ i − j ≤ maxGap` (portée ≈ `RP_MAX_GAP_M`).
/// La recherche s'appuie sur une grille de hachage de cellule
/// `max(close_thr, 1)` m avec **insertion différée** : le point `i` n'est
/// indexé qu'après sa comparaison aux points antérieurs, si bien que chaque
/// paire `(j, i)` n'est examinée qu'une fois — coût nominal O(n) au lieu de
/// O(n²).
///
/// Les candidats sont retournés dans l'ordre de découverte, tronqués à
/// `RP_MAX_PAIRS`. Les paires brutes ne portent **pas** leur distance
/// (mesurée au raffinage de l'É4a — invariant « `d` toujours défini »).
pub(super) fn find_proximity_candidates(r: &ResampledGeo, close_thr: f64) -> Vec<Candidate> {
    let n = r.n;
    let mut cands: Vec<Candidate> = Vec::new();
    if n == 0 || r.rx.len() < n + 1 || r.ry.len() < n + 1 {
        return cands;
    }

    let cell = close_thr.max(1.0);
    // Portée maximale d'une boucle recherchée (~4 km), exprimée en échantillons.
    let gap_f = (RP_MAX_GAP_M / r.step).ceil() + 8.0;
    let max_gap = if gap_f.is_finite() && gap_f > 0.0 {
        (gap_f as usize).min(n)
    } else {
        n
    };

    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

    'outer1: for i in 0..=n {
        let cx = (r.rx[i] / cell).floor() as i32;
        let cy = (r.ry[i] / cell).floor() as i32;
        for gx in -1..=1 {
            for gy in -1..=1 {
                let Some(bucket) = grid.get(&(cx + gx, cy + gy)) else {
                    continue;
                };
                for &j in bucket {
                    // j < i garanti par l'insertion différée ; `saturating_sub`
                    // écarte toute paire dans le mauvais sens sans dépassement.
                    let sep = i.saturating_sub(j);
                    if sep < RP_MIN_SEP || sep > max_gap {
                        continue;
                    }
                    let dx = r.rx[i] - r.rx[j];
                    let dy = r.ry[i] - r.ry[j];
                    if dx * dx + dy * dy < close_thr * close_thr {
                        cands.push(Candidate {
                            s: j,
                            e: i,
                            paire: Some((j, i)),
                            croisement: None,
                        });
                        if cands.len() >= RP_MAX_PAIRS {
                            break 'outer1;
                        }
                    }
                }
            }
        }
        // Insertion différée : APRÈS la comparaison (sinon la paire (i, i) et
        // les doublons symétriques seraient examinés).
        grid.entry((cx, cy)).or_default().push(i);
    }

    cands
}

// ─── Étape 5 — É2 : croisements de segments ──────────────────────────

/// Test de croisement de deux segments `[a, a+1]` et `[b, b+1]`
/// (ANALYSE §9.1, JS `segsCross`).
///
/// Niveau 1 — **croisement strict** : orientations opposées de part et d'autre
/// de chacun des deux segments (`o1·o2 < 0` et `o3·o4 < 0`).
/// Niveau 2 — **contact quasi exact** : colinéarité au seuil `RP_EPS_CROSS`
/// (m²) avec boîte élargie de 1e-6 m — attrape les boucles posées à plat l'une
/// sur l'autre sans croisement franc. Le seuil très strict (superposition au
/// centimètre) évite les faux contacts entre voies distinctes d'un giratoire.
///
/// Indices hors bornes : `false` (aucun accès hors tableau).
pub(super) fn segs_cross(r: &ResampledGeo, a: usize, b: usize) -> bool {
    let n = r.n;
    if a >= n || b >= n || r.rx.len() < n + 1 || r.ry.len() < n + 1 {
        return false;
    }

    let (ax, ay) = (r.rx[a], r.ry[a]);
    let (bx, by) = (r.rx[a + 1], r.ry[a + 1]);
    let (cx, cy) = (r.rx[b], r.ry[b]);
    let (dx, dy) = (r.rx[b + 1], r.ry[b + 1]);

    let o1 = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
    let o2 = (bx - ax) * (dy - ay) - (by - ay) * (dx - ax);
    let o3 = (dx - cx) * (ay - cy) - (dy - cy) * (ax - cx);
    let o4 = (dx - cx) * (by - cy) - (dy - cy) * (bx - cx);
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return true;
    }

    // Niveau 2 — contact quasi exact (colinéarité).
    let eps = RP_EPS_CROSS;
    let bxs = 1e-6;
    let on_seg = |px: f64, py: f64, ux: f64, uy: f64, vx: f64, vy: f64| -> bool {
        let cr = (vx - ux) * (py - uy) - (vy - uy) * (px - ux);
        if cr.abs() > eps {
            return false;
        }
        px >= ux.min(vx) - bxs
            && px <= ux.max(vx) + bxs
            && py >= uy.min(vy) - bxs
            && py <= uy.max(vy) + bxs
    };
    on_seg(cx, cy, ax, ay, bx, by)
        || on_seg(dx, dy, ax, ay, bx, by)
        || on_seg(ax, ay, cx, cy, dx, dy)
        || on_seg(bx, by, cx, cy, dx, dy)
}

/// É2 — candidats par croisement de segments (ANALYSE §9.2).
///
/// Chaque segment `k = [k, k+1]` est comparé aux segments **antérieurs non
/// adjacents** `m` (`k − m ≥ 3`) ; un croisement produit le candidat
/// `{ s: m, e: k+1, croisement: (m, k+1) }`. L'indexation utilise une grille de
/// segments de cellule `max(close_thr, 3·step)`, chaque segment occupant toutes
/// les cellules de sa boîte englobante (insertion différée là aussi).
///
/// Les couples sont dédoublonnés par la clé `m·10⁶ + k` (sûre : `RP_MAX_SAMPLES`
/// garantit `k < 10⁶`) et le nombre de tests est borné par `RP_MAX_TESTS`.
pub(super) fn find_crossing_candidates(r: &ResampledGeo, close_thr: f64) -> Vec<Candidate> {
    let n = r.n;
    let mut cands: Vec<Candidate> = Vec::new();
    if n == 0 || r.rx.len() < n + 1 || r.ry.len() < n + 1 {
        return cands;
    }

    let cell2 = close_thr.max(3.0 * r.step);
    let mut seg_grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut seen_cross: HashSet<u64> = HashSet::new();
    let mut tests: usize = 0;

    'outer2: for k in 0..=n - 1 {
        let (ax, ay) = (r.rx[k], r.ry[k]);
        let (bx, by) = (r.rx[k + 1], r.ry[k + 1]);
        let x0 = (ax.min(bx) / cell2).floor() as i32;
        let x1 = (ax.max(bx) / cell2).floor() as i32;
        let y0 = (ay.min(by) / cell2).floor() as i32;
        let y1 = (ay.max(by) / cell2).floor() as i32;

        for gx in x0..=x1 {
            for gy in y0..=y1 {
                let Some(bucket) = seg_grid.get(&(gx, gy)) else {
                    continue;
                };
                for &m in bucket {
                    // Segments adjacents (un point en commun) exclus.
                    if k.saturating_sub(m) < 3 {
                        continue;
                    }
                    let key = (m as u64) * 1_000_000 + k as u64;
                    if !seen_cross.insert(key) {
                        continue;
                    }
                    tests += 1;
                    if tests > RP_MAX_TESTS {
                        break 'outer2;
                    }
                    if segs_cross(r, m, k) {
                        cands.push(Candidate {
                            s: m,
                            e: k + 1,
                            paire: None,
                            croisement: Some((m, k + 1)),
                        });
                    }
                }
            }
        }

        // Indexation du segment [k, k+1] dans toutes les cellules de sa boîte.
        for gx in x0..=x1 {
            for gy in y0..=y1 {
                seg_grid.entry((gx, gy)).or_default().push(k);
            }
        }
    }

    cands
}

// ─── Étape 6 — É3 : fusion des candidats ─────────────────────────────

/// É3 — fusion des candidats chevauchants (ANALYSE §10).
///
/// Les candidats sont triés par `(s, e)` croissants puis fusionnés par
/// recouvrement glissant : tant que le candidat courant commence avant la fin
/// du groupe ouvert, il l'étend (`e = max(e, c.e)`) et ses descripteurs
/// s'ajoutent aux listes du groupe ; sinon un nouveau groupe est ouvert.
///
/// Propriété produite : les groupes sont **disjoints et ordonnés** — condition
/// requise par l'É5 pour borner mutuellement les emprises voisines.
///
/// **Attention** (leçon de cas réel) : un même groupe peut fusionner des
/// centaines de candidats hétérogènes en un méga-intervalle quasi égal à la
/// trace entière ; l'intervalle fusionné n'est qu'un **conteneur**, jamais une
/// anomalie en soi — c'est l'É4 (fenêtres bornées) qui démêle.
pub(super) fn merge_candidates(cands: &[Candidate]) -> Vec<MergedGroup> {
    let mut sorted: Vec<Candidate> = cands.to_vec();
    // Tri stable par (s, e) : à clés égales, l'ordre de découverte est conservé
    // (le JS s'appuie sur la stabilité de `Array.sort`).
    sorted.sort_by(|u, v| u.s.cmp(&v.s).then(u.e.cmp(&v.e)));

    let mut merged: Vec<MergedGroup> = Vec::new();
    for c in sorted {
        if let Some(last) = merged.last_mut() {
            if c.s <= last.e {
                if c.e > last.e {
                    last.e = c.e;
                }
                if let Some(p) = c.paire {
                    last.paires.push(p);
                }
                if let Some(cr) = c.croisement {
                    last.croisements.push(cr);
                }
                continue;
            }
        }
        merged.push(MergedGroup {
            s: c.s,
            e: c.e,
            paires: if let Some(p) = c.paire { vec![p] } else { Vec::new() },
            croisements: if let Some(cr) = c.croisement {
                vec![cr]
            } else {
                Vec::new()
            },
        });
    }

    merged
}

// ─── Étape 7 — É4a : raffinage des paires ────────────────────────────

/// É4a — raffinage des paires de l'É1 sur les points de **référence**
/// (ANALYSE §11.1).
///
/// Les paires brutes sont en indices d'**échantillons** : leur position dépend
/// de la phase du pas d'échantillonnage, alors que la refermeture réelle se
/// produit entre points de référence (parfois en superposition exacte, 0,0 m).
/// Pour chaque paire `(a, b)`, les 25 combinaisons `da, db ∈ [−2, 2]` sont
/// explorées et l'on retient celle qui minimise la distance entre les points de
/// référence hébergés (`orig[ia]`, `orig[ib]`). Les paires raffinées sont
/// dédoublonnées par la clé `best.a·10⁶ + best.b` (sûre : `n < 10⁶`).
///
/// Seule cette liste peut être restituée dans un finding : **`d` est ainsi
/// garanti défini** (invariant « `pairs[].d` toujours défini »).
pub(super) fn refine_pairs(
    merged: &[MergedGroup],
    orig: &[usize],
    ref_x: &[f64],
    ref_y: &[f64],
    n: usize,
) -> Vec<RefinedPair> {
    let mut refined: Vec<RefinedPair> = Vec::new();
    // Borne défensive : `orig` doit héberger les n+1 échantillons.
    if orig.len() < n + 1 {
        return refined;
    }
    let mut seen_pair: HashSet<u64> = HashSet::new();

    for g in merged {
        for &(a, b) in &g.paires {
            let mut best: Option<RefinedPair> = None;
            for da in -2i64..=2 {
                for db in -2i64..=2 {
                    let ia = a as i64 + da;
                    let ib = b as i64 + db;
                    // ia < 0 : le JS écarte la combinaison ; ib > n et
                    // ib − ia < RP_MIN_SEP sont les deux autres rejets.
                    if ia < 0 || ib > n as i64 || ib - ia < RP_MIN_SEP as i64 {
                        continue;
                    }
                    let (ia, ib) = (ia as usize, ib as usize);
                    let (oa, ob) = (orig[ia], orig[ib]);
                    if oa >= ref_x.len() || ob >= ref_x.len() || oa >= ref_y.len() || ob >= ref_y.len()
                    {
                        continue;
                    }
                    let d = (ref_x[oa] - ref_x[ob]).hypot(ref_y[oa] - ref_y[ob]);
                    // Minimum STRICT : à égalité, la première combinaison
                    // rencontrée dans l'ordre (da, db) est conservée.
                    if best.is_none_or(|cur| d < cur.d) {
                        best = Some(RefinedPair { a: ia, b: ib, d });
                    }
                }
            }
            let Some(best) = best else { continue };
            let key = (best.a as u64) * 1_000_000 + best.b as u64;
            if !seen_pair.insert(key) {
                continue;
            }
            refined.push(best);
        }
    }

    refined
}

/// `dMin` de l'ANALYSE §11.1 : plus petite distance raffinée (0 si aucune).
///
/// C'est le seuil d'éligibilité du coin de refermeture en É4b (« paires
/// serrées » uniquement) : `d ≤ dMin + max(2 ; 0,25·closeThr)`.
pub(super) fn min_pair_distance(refined: &[RefinedPair]) -> f64 {
    if refined.is_empty() {
        return 0.0;
    }
    refined.iter().fold(f64::INFINITY, |m, p| m.min(p.d))
}

// ─── Étape 8 — marches de cap et mesures de refermeture ──────────────

/// Marche de cap sur les échantillons **bruts** (ANALYSE §11.3, JS `rpWalk`).
///
/// Avance de `dir` en accumulant les distances jusqu'à atteindre `min_m` m
/// (retour de l'échantillon atteint) ou épuiser `max_seg` segments. Les
/// micro-segments (< 0,05 m) sont sautés **hors budget** : ils n'incrémentent
/// pas le compte de segments. Si la portée n'est jamais atteinte, retourne le
/// dernier échantillon visité — mais seulement si une distance a été parcourue
/// (sinon `None` : borne de trace inexplorable).
pub(super) fn rp_walk(
    r: &ResampledGeo,
    from: usize,
    dir: i32,
    min_m: f64,
    max_seg: usize,
) -> Option<usize> {
    let n = r.n;
    if r.rx.len() < n + 1 || r.ry.len() < n + 1 || from > n {
        return None;
    }

    let mut i = from as i64;
    let mut acc = 0.0;
    let mut segs = 0usize;
    while segs < max_seg {
        let ni = i + dir as i64;
        if ni < 0 || ni > n as i64 {
            break;
        }
        let ni_u = ni as usize;
        let d = (r.rx[ni_u] - r.rx[i as usize]).hypot(r.ry[ni_u] - r.ry[i as usize]);
        i = ni;
        if d < 0.05 {
            continue; // micro-segment : sauté, hors budget
        }
        segs += 1;
        acc += d;
        if acc >= min_m {
            return Some(ni_u);
        }
    }

    if acc > 0.0 {
        Some(i as usize)
    } else {
        None
    }
}

/// Coin de refermeture au point `rb` (ANALYSE §11.3, JS `closingTurn`).
///
/// Cap entrant `ja → rb` contre cap sortant `rb → ka`, mesurés sur les
/// échantillons **bruts** (le lissage ±1 échantillon écrase le demi-tour de
/// réengagement), normalisés dans ]−180, +180]. `None` si un côté n'est pas
/// explorable.
pub(super) fn closing_turn(r: &ResampledGeo, rb: usize) -> Option<f64> {
    let ja = rp_walk(r, rb, -1, RP_WALK_MIN_M, RP_WALK_MAX_SEG)?;
    let ka = rp_walk(r, rb, 1, RP_WALK_MIN_M, RP_WALK_MAX_SEG)?;
    let h_in = (r.ry[rb] - r.ry[ja]).atan2(r.rx[rb] - r.rx[ja]);
    let h_out = (r.ry[ka] - r.ry[rb]).atan2(r.rx[ka] - r.rx[rb]);
    Some(rp_norm_180(
        (h_out - h_in) * 180.0 / std::f64::consts::PI,
    ))
}

/// Delta de cap entrée/sortie d'une fenêtre (ANALYSE §11.5, JS `closingDelta`).
///
/// Cap sortant au point `rb` (~8 m vers l'aval) contre cap entrant au point
/// `ra` (~8 m vers l'amont), normalisé dans ]−180, +180].
pub(super) fn closing_delta(r: &ResampledGeo, ra: usize, rb: usize) -> Option<f64> {
    let ja = rp_walk(r, ra, -1, RP_WALK_MIN_M, RP_WALK_MAX_SEG)?;
    let ka = rp_walk(r, rb, 1, RP_WALK_MIN_M, RP_WALK_MAX_SEG)?;
    let h_in = (r.ry[ra] - r.ry[ja]).atan2(r.rx[ra] - r.rx[ja]);
    let h_out = (r.ry[ka] - r.ry[rb]).atan2(r.rx[ka] - r.rx[rb]);
    Some(rp_norm_180(
        (h_out - h_in) * 180.0 / std::f64::consts::PI,
    ))
}

/// Garde anti-aiguille (ANALYSE §11.4, JS `loopDegenerate`).
///
/// Un anneau *balise une surface*, une aiguille non — deux tests indépendants,
/// **rejet si l'un OU l'autre déclenche** :
/// - **test 1 — circularité** du polygone fermé `[a..b]` (formule du lacet,
///   corde de fermeture `b → a` incluse) : rejet si `< RP_CIRC_MIN`. Un
///   aller-retour a une aire algébrique nulle — les deux brins se compensent,
///   même bruités ;
/// - **test 2 — recouvrement miroir** : fraction des échantillons dont le
///   symétrique par longueur depuis l'extrémité (`b − (i − a)`) est à moins de
///   `close_thr` ; rejet si `≥ RP_MIRROR_MAX`. Dans un aller-retour, chaque
///   point a son miroir sur le brin retour (fraction ≈ 1) ; sur un anneau, les
///   miroirs s'écartent en `2r·sin(θ/2)` dès qu'on quitte la jonction.
///
/// Entrée incohérente (`a ≥ b` ou borne franchie) : traitée comme dégénérée.
pub(super) fn loop_degenerate(r: &ResampledGeo, a: usize, b: usize, close_thr: f64) -> bool {
    let n = r.n;
    if a >= b || b > n || r.rx.len() < n + 1 || r.ry.len() < n + 1 {
        return true;
    }

    // Test 1 — circularité du polygone fermé.
    let mut a2 = 0.0;
    let mut l = 0.0;
    for i in a..b {
        a2 += r.rx[i] * r.ry[i + 1] - r.rx[i + 1] * r.ry[i];
        l += (r.rx[i + 1] - r.rx[i]).hypot(r.ry[i + 1] - r.ry[i]);
    }
    a2 += r.rx[b] * r.ry[a] - r.rx[a] * r.ry[b];
    l += (r.rx[a] - r.rx[b]).hypot(r.ry[a] - r.ry[b]);
    if l > 0.0 && 4.0 * std::f64::consts::PI * (a2 / 2.0).abs() / (l * l) < RP_CIRC_MIN {
        return true;
    }

    // Test 2 — recouvrement miroir.
    let total = b - a + 1;
    let thr2 = close_thr * close_thr;
    let mut near = 0usize;
    for i in a..=b {
        let mi = b - (i - a);
        let dx = r.rx[i] - r.rx[mi];
        let dy = r.ry[i] - r.ry[mi];
        if dx * dx + dy * dy < thr2 {
            near += 1;
        }
    }
    near as f64 / total as f64 >= RP_MIRROR_MAX
}

// ─── Étape 10 — É4b/c/d : fenêtres refermées bornées ─────────────────

/// Contexte partagé de construction des fenêtres (É4b/c/d).
///
/// Représente l'environnement capturé par la closure `pushWindow` du JS :
/// la structure `R`, le cumul angulaire `pre`, et les paramètres d'analyse
/// (`close_thr`, `dMin` de l'É4a, seuil angulaire `p-angle`).
pub(super) struct WindowCtx<'a> {
    pub(super) r: &'a ResampledGeo,
    pub(super) pre: &'a [f64],
    pub(super) close_thr: f64,
    pub(super) d_min: f64,
    pub(super) ang_thr: f64,
}

impl<'a> WindowCtx<'a> {
    pub(super) fn new(
        r: &'a ResampledGeo,
        pre: &'a [f64],
        close_thr: f64,
        d_min: f64,
        ang_thr: f64,
    ) -> Self {
        Self {
            r,
            pre,
            close_thr,
            d_min,
            ang_thr,
        }
    }
}

/// É4b — construction d'une fenêtre refermée bornée (ANALYSE §11.2,
/// JS `pushWindow`).
///
/// Rejets successifs : fenêtre trop courte (`b − a < RP_MIN_SEP`), périmètre
/// supérieur à `RP_LMAX` (**fenêtre bornée** : l'intervalle fusionné d'un
/// aller-retour macroscopique est un conteneur, pas une anomalie), rotation
/// insuffisante (`total < ang_thr`) et motif dégénéré (`loop_degenerate`).
///
/// Le **coin de refermeture** n'est mesuré que pour les paires **serrées**
/// (`with_coin` et `d ≤ dMin + max(2 ; 0,25·closeThr)`) et n'est retenu que
/// dans la plage des vrais demi-tours de réengagement, son sens étant aligné
/// sur celui de la rotation cumulée. La rotation est ensuite **quantifiée**
/// (±40° sur un multiple de 360°) : la refermeture se produit à l'angle vif de
/// l'entrée, d'où une oscillation de 320 à 400° bruts.
pub(super) fn push_window(
    ctx: &WindowCtx,
    a: usize,
    b: usize,
    d: Option<f64>,
    with_coin: bool,
    windows: &mut Vec<Window>,
) {
    let r = ctx.r;
    let n = r.n;
    if a >= b || b > n || r.rx.len() < n + 1 || r.cum_r.len() < n + 1 {
        return;
    }
    if b - a < RP_MIN_SEP {
        return;
    }
    if r.cum_r[b] - r.cum_r[a] > RP_LMAX {
        return;
    }

    let ang = angle_between(ctx.pre, a, b);
    let mut coin = 0.0;
    let coin_thr = ctx.d_min + 2.0f64.max(0.25 * ctx.close_thr);
    if with_coin && d.is_some_and(|dv| dv <= coin_thr) {
        if let Some(t0) = closing_turn(r, b) {
            if t0.abs() >= RP_COIN_LO && t0.abs() <= RP_COIN_HI {
                let mut t = t0;
                // Alignement du sens : le coin doit tourner dans le même sens
                // que la rotation cumulée, sans quoi il l'annulerait.
                if ang.abs() > 30.0 && (t < 0.0) != (ang < 0.0) {
                    t -= 360.0 * t.signum();
                }
                coin = t;
            }
        }
    }

    let mut total = (ang + coin).abs();
    let kk = (total / 360.0).round();
    if kk >= 1.0 && (total - 360.0 * kk).abs() <= RP_QUANT_TOL {
        total = 360.0 * kk;
    }
    if total < ctx.ang_thr {
        return;
    }
    if loop_degenerate(r, a, b, ctx.close_thr) {
        return;
    }

    let dc = if with_coin {
        closing_delta(r, a, b)
    } else {
        None
    };
    let propre = if with_coin {
        dc.is_none_or(|v| v.abs() >= RP_PROPRE_DEG)
    } else {
        true
    };

    windows.push(Window {
        a,
        b,
        d,
        total,
        propre,
        pairs_in: Vec::new(),
    });
}

/// É4d — paires raffinées contenues dans chaque fenêtre (ANALYSE §11.6).
///
/// Le balayage suppose les paires triées par `(a, b)` ; le tri est fait sur une
/// copie locale — le JS trie la liste en place mais ne la réutilise plus.
pub(super) fn fill_pairs_in(windows: &mut [Window], refined: &[RefinedPair]) {
    let mut sorted: Vec<RefinedPair> = refined.to_vec();
    sorted.sort_by(|u, v| u.a.cmp(&v.a).then(u.b.cmp(&v.b)));

    for w in windows.iter_mut() {
        for q in &sorted {
            if q.a < w.a {
                continue;
            }
            if q.a > w.b {
                break;
            }
            if q.b <= w.b {
                w.pairs_in.push(*q);
            }
        }
    }
}

/// É4e — sélection gloutonne des fenêtres disjointes (ANALYSE §11.6).
///
/// Ordre de préférence : **propres d'abord** (refermeture ≠ cap d'entrée),
/// puis rotation la plus forte, puis refermeture la plus serrée (`None` = ∞),
/// puis fenêtre la plus compacte. Une fenêtre est retenue si elle ne chevauche
/// aucune fenêtre déjà retenue ; les retenues sont re-triées par `a` croissant.
pub(super) fn select_windows(mut windows: Vec<Window>) -> Vec<Window> {
    windows.sort_by(|u, v| {
        // 1. propres d'abord
        let ord = (v.propre as u8).cmp(&(u.propre as u8));
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        // 2. rotation la plus forte
        let ord = v
            .total
            .partial_cmp(&u.total)
            .unwrap_or(std::cmp::Ordering::Equal);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        // 3. refermeture la plus serrée (None = ∞)
        let du = u.d.unwrap_or(f64::INFINITY);
        let dv = v.d.unwrap_or(f64::INFINITY);
        let ord = du.partial_cmp(&dv).unwrap_or(std::cmp::Ordering::Equal);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        // 4. fenêtre la plus compacte
        u.b.saturating_sub(u.a).cmp(&v.b.saturating_sub(v.a))
    });

    let mut taken: Vec<Window> = Vec::new();
    for w in windows {
        // Chevauchement ⟺ NON (w.b < t.a || w.a > t.b).
        let overlaps = taken.iter().any(|t| !(w.b < t.a || w.a > t.b));
        if !overlaps {
            taken.push(w);
        }
    }
    taken.sort_by(|u, v| u.a.cmp(&v.a));

    taken
}

// ─── Étape 11 — É5 : qualification et publication ────────────────────

/// É5 — publication des fenêtres retenues en indices de **référence**
/// (ANALYSE §12, JS `detectRP` — boucle finale).
///
/// `W = max(2, round(RP_ZONE_M / step))` est la largeur des portions latérales
/// (≈ 80 m de trace, exprimée en échantillons). Les emprises sont **bornées
/// mutuellement** : `sRef` ne remonte jamais avant `prevERef + 1`, `eRef` ne
/// descend jamais au-delà de `nextCsRef − 1`. Les ancres radiales (§13)
/// élargissent ensuite l'emprise — l'emprise englobe donc toujours la zone de
/// routage par défaut.
///
/// Toute grandeur publiée a transité par `R.orig` : c'est **le pivot du
/// contrat** d'intégration (un index d'échantillon publié tel quel produirait
/// des lectures hors tableau côté hôte).
pub(super) fn publish_windows(
    taken: &[Window],
    r: &ResampledGeo,
    px: &[f64],
    py: &[f64],
    ids: &[u32],
    close_thr: f64,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let n = r.n;
    let m_ref = ids.len();
    if m_ref == 0 || r.orig.len() < n + 1 {
        return out;
    }
    // Garde d'intégrité : toute valeur de `orig` doit désigner un point de
    // référence existant (sinon aucune publication n'est sûre).
    if r.orig.iter().any(|&o| o >= m_ref) {
        return out;
    }

    let w_zone = (RP_ZONE_M / r.step).round().max(2.0) as usize;
    let mut prev_e_ref: i64 = -1;

    for wi in 0..taken.len() {
        let w = &taken[wi];
        if w.a > n || w.b > n || w.a > w.b {
            continue;
        }
        let cs_ref = r.orig[w.a];
        let ce_ref = r.orig[w.b];
        // Fenêtre dont le cœur retombe dans l'emprise précédente : absorbée.
        if ce_ref as i64 <= prev_e_ref {
            continue;
        }
        let next_cs_ref = if wi + 1 < taken.len() {
            r.orig[taken[wi + 1].a]
        } else {
            m_ref
        };

        // Emprise par défaut : bornée par l'emprise précédente, la fenêtre
        // suivante et la largeur latérale W.
        let mut s_ref = ((prev_e_ref + 1).max(0) as usize)
            .max(r.orig[w.a.saturating_sub(w_zone)])
            .min(cs_ref);
        let mut e_ref = (m_ref - 1)
            .min(next_cs_ref.saturating_sub(1))
            .min(r.orig[w.b.saturating_add(w_zone).min(n)])
            .max(ce_ref);

        // Élargissement D2 aux ancres radiales (§13).
        let an0 = rp_anchor_indices(px, py, cs_ref, ce_ref, close_thr);
        if let Some(up) = an0.up {
            if up < s_ref {
                s_ref = ((prev_e_ref + 1).max(0) as usize).max(up);
            }
        }
        if let Some(dn) = an0.dn {
            if dn > e_ref {
                e_ref = next_cs_ref.saturating_sub(1).min(dn);
            }
        }
        // Re-bornage : l'emprise contient toujours le cœur.
        if s_ref > cs_ref {
            s_ref = cs_ref;
        }
        if e_ref < ce_ref {
            e_ref = ce_ref;
        }
        prev_e_ref = e_ref as i64;

        // Jonction = DÉBUT de boucle (et non le milieu de la paire de
        // refermeture, qui tomberait au centre du giratoire quand les voies
        // d'entrée et de sortie sont superposées).
        let junc_ref = cs_ref;
        let mut pairs_sorted: Vec<RefinedPair> = w.pairs_in.clone();
        pairs_sorted.sort_by(|p, q| p.d.partial_cmp(&q.d).unwrap_or(std::cmp::Ordering::Equal));
        pairs_sorted.truncate(RP_PAIRS_TOP);
        let pairs: Vec<FindingPair> = pairs_sorted
            .iter()
            .map(|p| FindingPair {
                aid: ids[r.orig[p.a]],
                bid: ids[r.orig[p.b]],
                a: r.orig[p.a],
                b: r.orig[p.b],
                d: p.d,
            })
            .collect();

        let pair_txt = pairs
            .iter()
            .map(|p| {
                let dist = if p.d < 10.0 {
                    format!("{:.1}", p.d)
                } else {
                    format!("{}", p.d.round())
                };
                format!("pts {}↔{} ({} m)", p.a + 1, p.b + 1, dist)
            })
            .collect::<Vec<_>>()
            .join(" · ");

        let tt = turn_text(w.total);
        let total_angle = w.total.round() as i32;
        let ctx_up = if s_ref > 0 { Some(ids[s_ref - 1]) } else { None };
        let ctx_dn = if e_ref + 1 < m_ref {
            Some(ids[e_ref + 1])
        } else {
            None
        };

        let num = out.len() + 1; // numérotation par famille (précédent AR)
        let label = if w.total > 340.0 {
            "Tour de rond-point"
        } else {
            "Boucle giratoire"
        };
        let pair_idx: Vec<usize> = pairs.iter().flat_map(|p| [p.a, p.b]).collect();
        let parts = vec![
            FindingPart {
                s: s_ref,
                e: e_ref,
                role: PartRole::Warn,
                text: if pairs.is_empty() {
                    "refermeture par croisement de trace".to_string()
                } else {
                    format!("superpositions : {}", pair_txt)
                },
            },
            FindingPart {
                s: cs_ref,
                e: ce_ref,
                role: PartRole::Warn,
                text: format!("cœur : angle cumulé {}° ({})", total_angle, tt),
            },
        ];

        out.push(Finding {
            id: format!("rp-{}", num),
            kind: FindingKind::Rp,
            label: format!("{} : {}", label, num),
            summary: format!(
                "jonction pt {} · {}° ({}) · {} paire{}",
                junc_ref + 1,
                total_angle,
                tt,
                pairs.len(),
                if pairs.len() > 1 { "s" } else { "" }
            ),
            peak: junc_ref,
            peak_id: ids[junc_ref],
            pairs,
            pair_idx,
            ecart: None,
            d1: None,
            d2: None,
            total_angle: Some(total_angle),
            turn_text: Some(tt),
            core_ids: ids[cs_ref..=ce_ref].to_vec(),
            zone_ids: ids[s_ref..=e_ref].to_vec(),
            ctx_ids: FindingContextIds {
                up: ctx_up,
                dn: ctx_dn,
            },
            ctx: FindingContext {
                up: if ctx_up.is_some() { Some(s_ref - 1) } else { None },
                dn: if ctx_dn.is_some() { Some(e_ref + 1) } else { None },
            },
            parts,
            status: FindingStatus::Pending,
            correction: None,
            undo: None,
        });
    }

    out
}

/// Détecteur de boucles giratoires (ANALYSE §6 à §12, JS `detectRP`).
///
/// Pipeline : rééchantillonnage à pas constant → É1 (paires par proximité) +
/// É2 (croisements de segments) → É3 (fusion) → É4 (raffinage, fenêtres
/// bornées, anti-aiguille, glouton disjoint) → É5 (qualification et
/// publication en indices de référence).
///
/// # Préconditions
/// - `px`/`py`/`cum`/`ids` : tableaux parallèles de la trace **consolidée**,
///   `total` valant `cum[m−1]` ;
/// - `params.close_m` = `p-close` (défaut 15), `params.angle_deg` = `p-angle`
///   (défaut 270).
///
/// # Postconditions
/// - Findings triés par position croissante, emprises disjointes et bornées
///   mutuellement, `d` défini pour toutes les paires, `status = pending`.
///
/// `points` n'est pas consommé par l'algorithme (le JS `detectRP` n'utilise que
/// la géométrie) — conservé pour la symétrie de contrat avec `detect_ar`.
pub fn detect_rp(
    _points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    cum: &[f64],
    ids: &[u32],
    total: f64,
    params: &AuditParams,
) -> Vec<Finding> {
    // Prétraitement : rééchantillonnage à pas constant.
    let r = resample_geo(px, py, cum, total, RP_STEP_DEFAULT);
    let n = r.n;
    if n < RP_MIN_SAMPLES || r.orig.len() < n + 1 {
        return Vec::new();
    }

    let close_thr = params.close_m;
    let ang_thr = params.angle_deg as f64;

    // Précalcul des caps lissés et du cumul angulaire signé.
    let pre = compute_heading_prefix(&r);

    // É1 + É2 — candidats géométriques, dans l'ordre du JS (proximité puis
    // croisements : l'ordre d'insertion est conservé par le tri stable de l'É3).
    let mut cands = find_proximity_candidates(&r, close_thr);
    cands.extend(find_crossing_candidates(&r, close_thr));

    // É3 — fusion, puis É4a — raffinage sur les points de référence.
    let merged = merge_candidates(&cands);
    let refined = refine_pairs(&merged, &r.orig, px, py, n);
    let d_min = min_pair_distance(&refined);

    // É4b/c — fenêtres : une par paire raffinée (avec coin), une par
    // croisement (sans coin).
    let ctx = WindowCtx::new(&r, &pre, close_thr, d_min, ang_thr);
    let mut windows: Vec<Window> = Vec::new();
    for p in &refined {
        push_window(&ctx, p.a, p.b, Some(p.d), true, &mut windows);
    }
    for g in &merged {
        for &(ca, cb) in &g.croisements {
            push_window(&ctx, ca, cb, None, false, &mut windows);
        }
    }

    // É4d — paires contenues, puis É4e — sélection gloutonne disjointe.
    fill_pairs_in(&mut windows, &refined);
    let taken = select_windows(windows);

    // É5 — qualification et publication.
    publish_windows(&taken, &r, px, py, ids, close_thr)
}

// ─── Types internes ──────────────────────────────────────────────────

/// Candidat issu de l'É1 (proximité) ou de l'É2 (croisement).
///
/// `s`/`e` bornent l'intervalle en indices d'échantillons ; l'un des deux
/// descripteurs est renseigné selon la phase productrice (l'É3 fusionne les
/// candidats de même nature).
#[derive(Debug, Clone)]
pub(super) struct Candidate {
    pub(super) s: usize,
    pub(super) e: usize,
    pub(super) paire: Option<(usize, usize)>,      // É1
    pub(super) croisement: Option<(usize, usize)>, // É2
}

/// Groupe de candidats fusionnés par l'É3 (conteneur, jamais une anomalie).
#[derive(Debug, Clone)]
pub(super) struct MergedGroup {
    pub(super) s: usize,
    pub(super) e: usize,
    pub(super) paires: Vec<(usize, usize)>,
    pub(super) croisements: Vec<(usize, usize)>,
}

/// Paire raffinée sur les points de référence (É4a) — seule forme publiée.
#[derive(Debug, Clone, Copy)]
pub(super) struct RefinedPair {
    pub(super) a: usize,
    pub(super) b: usize,
    pub(super) d: f64,
}

/// Fenêtre refermée bornée (É4b) — candidate finding RP.
#[derive(Debug, Clone)]
pub(super) struct Window {
    pub(super) a: usize,
    pub(super) b: usize,
    pub(super) d: Option<f64>,
    pub(super) total: f64,
    pub(super) propre: bool,
    pub(super) pairs_in: Vec<RefinedPair>,
}
