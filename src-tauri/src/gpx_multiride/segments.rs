//! Assemblage des runs en segments (spécification §5, version v5).
//!
//! Quatrième étape du pipeline. Les runs sortis de `runs` sont des **couples
//! d'intervalles bruts** ; l'état recherché est celui-ci :
//!
//! 1. chaque emprunt réel du tronçon est **un** intervalle contigu — or le
//!    chaînage fragmente (micro-ruptures, `MAXGAP`) ;
//! 2. les emprunts d'un même tronçon sont regroupés dans un même segment,
//!    ordonnés chronologiquement ;
//! 3. deux **références** de segments ne se chevauchent jamais : un segment dont
//!    la référence est incluse dans celle d'un autre est absorbé ;
//! 4. la fin d'un aller et le début de son retour ne sont **jamais** recousus,
//!    même si les deux fragments sont adjacents le long de la trace — le
//!    demi-tour est souvent plus court que le rayon de fusion.
//!
//! Le point 4 est le plus délicat : c'est la distinction entre une **fente de
//! chaînage** (à recoudre) et une **frontière entre passes** (à préserver), les
//! deux étant des intervalles quasi contigus. Elle est tranchée par
//! `boundary_between_passes`, qui **ne dépend pas** du réglage de fusion — c'est
//! ce qui rend un aller-retour indestructible quel que soit ce réglage.
//!
//! Le module porte les structures **internes** de l'algorithme (`Interval`,
//! `Passage`, `Segment`) ; leur projection sur le contrat de fichier
//! (`MultiridePassage`) est faite par `detection`.

use std::collections::{BTreeSet, HashMap};

use super::direction::relative_direction;
use super::projection::MultirideGeom;
use super::resample::{raw_point_number, Resample};
use super::runs::Run;

// ─── Constantes de la spécification ───────────────────────────────────

/// Recouvrement (en points) au-delà duquel deux items décrivent le même passage.
const SAME_PASSAGE_OVERLAP: usize = 4;
/// Nombre maximal d'itérations de l'assemblage — garde-fou contre les
/// oscillations théoriques, la boucle s'arrêtant normalement à la stabilité.
const MAX_ITERATIONS: usize = 5;
/// Seuil d'inversion de cap (T1) : en deçà, la jonction est une frontière.
const REVERSE_DOT: f64 = -0.5;
/// Fenêtre (points) de part et d'autre d'une jonction pour mesurer les caps.
const BOUNDARY_SPAN: usize = 4;
/// Facteur du test de ré-emprunt de route (T2).
const REEMPRUNT_FACTOR: f64 = 0.5;
/// Gate de proximité de T2, en multiples de la tolérance.
const REEMPRUNT_GATE_TOL: f64 = 4.0;
/// Fenêtre (points) inspectée après la fin de X par T2.
const REEMPRUNT_LOOKAHEAD: usize = 18;
/// Décalage minimal après la jonction pour que T2 soit discriminant.
const REEMPRUNT_MIN_OFFSET: usize = 6;
/// Pas de la fenêtre de T2, en points.
const REEMPRUNT_STEP: usize = 4;
/// Nombre maximal d'échantillons pour scanner X dans T2.
const REEMPRUNT_STRIDE_MAX: usize = 300;
/// Facteur de la Passe D (fusion intra-segment).
const MERGE_WITHIN_SEGMENT_FACTOR: f64 = 2.0;

// ─── Structures internes ──────────────────────────────────────────────

/// Intervalle de points échantillonnés (`s ≤ e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub s: usize,
    pub e: usize,
}

/// Emprunt assemblé d'un tronçon, avec son sens et ses métadonnées.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Passage {
    pub s: usize,
    pub e: usize,
    /// Sens relatif à la référence du segment : `0` référence, `+1` même sens,
    /// `−1` sens opposé.
    pub rel: i8,
    /// Distance cumulée le long de la trace (km).
    pub km0: f64,
    pub km1: f64,
    /// Numéros de points du **GPX d'origine** (1-based).
    pub pt0: usize,
    pub pt1: usize,
}

/// Tronçon répété et ses emprunts, référence en tête.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub passages: Vec<Passage>,
}

// ─── Outils ───────────────────────────────────────────────────────────

/// Clé symétrique d'une paire : la paire doit être reconnue quel que soit
/// l'ordre d'énumération.
fn pair_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Union-find avec compression de chemin, sur des identifiants entiers.
struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        let mut root = value;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = value;
        while self.parent[current] != root {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (root_a, root_b) = (self.find(a), self.find(b));
        if root_a != root_b {
            self.parent[root_b] = root_a;
        }
    }
}

/// Carré de la distance entre deux points échantillonnés.
fn distance2(r: &Resample, a: usize, b: usize) -> f64 {
    let dx = r.px[a] - r.px[b];
    let dy = r.py[a] - r.py[b];
    dx * dx + dy * dy
}

/// Vecteur de déplacement sur `BOUNDARY_SPAN` points, en arrière depuis la fin
/// d'un intervalle ou en avant depuis son début.
fn direction_at(r: &Resample, index: usize, backwards: bool) -> Option<(f64, f64)> {
    let last = r.px.len() - 1;
    let (from, to) = if backwards {
        (index.saturating_sub(BOUNDARY_SPAN), index)
    } else {
        (index, (index + BOUNDARY_SPAN).min(last))
    };
    if from == to {
        return None;
    }
    Some((r.px[to] - r.px[from], r.py[to] - r.py[from]))
}

// ─── Prédicats ────────────────────────────────────────────────────────

/// Deux items décrivent-ils le même emprunt ?
///
/// Soit leurs intervalles se recouvrent — fragments d'un même emprunt vus par
/// plusieurs runs —, soit ils sont contigus à `fusePts` près, et l'on recoud
/// alors une **fente de chaînage**.
fn same_passage(x: &Interval, y: &Interval, fuse_pts: usize) -> bool {
    let overlap = x.e.min(y.e) as isize - x.s.max(y.s) as isize;
    let gap = x.s.max(y.s) as isize - x.e.min(y.e) as isize;
    overlap >= SAME_PASSAGE_OVERLAP as isize || (0 <= gap && gap <= fuse_pts as isize)
}

/// La jonction entre `x` (qui se termine) et `y` (qui commence) porte-t-elle la
/// signature d'une **frontière entre deux passes** ?
///
/// Deux tests indépendants ; un seul suffit à refuser la fusion. Ce prédicat ne
/// dépend pas du réglage de fusion : c'est ce qui interdit à un curseur de fusion
/// de recoller un aller et son retour.
pub fn boundary_between_passes(r: &Resample, x: &Interval, y: &Interval, tol_m: f64) -> bool {
    reverse_of_course(r, x, y) || route_reborrowed(r, x, y, tol_m)
}

/// T1 — inversion de cap : les caps de fin de `x` et de début de `y` sont
/// opposés à plus de 120°. Un demi-tour inverse le cap ; deux fragments d'un même
/// emprunt le conservent.
fn reverse_of_course(r: &Resample, x: &Interval, y: &Interval) -> bool {
    let Some((dx, dy)) = direction_at(r, x.e, true) else {
        return false;
    };
    let Some((ex, ey)) = direction_at(r, y.s, false) else {
        return false;
    };
    let scale = dx.hypot(dy) * ex.hypot(ey);
    if scale <= 0.0 {
        return false;
    }
    (dx * ex + dy * ey) / scale < REVERSE_DOT
}

/// T2 — ré-emprunt de route : le début de `y` repasse sur la route couverte par
/// `x` **loin de la jonction**, donc il a reculé sur cette route au lieu de la
/// poursuivre.
///
/// La double condition est le cœur du test : `dd ≤ (4·tol)²` dit « q est sur la
/// route de x », et `dd < 0,5·dE` dit « q est nettement plus proche d'un point
/// interne de x que de la jonction ». Un fragment de continuation, lui, ne fait
/// que s'éloigner de la jonction.
fn route_reborrowed(r: &Resample, x: &Interval, y: &Interval, tol_m: f64) -> bool {
    let gate = (REEMPRUNT_GATE_TOL * tol_m) * (REEMPRUNT_GATE_TOL * tol_m);
    let start = (y.s + REEMPRUNT_MIN_OFFSET).max(x.e + 2);
    let end = y.e.min(y.s + REEMPRUNT_LOOKAHEAD);
    let stride = (x.e.saturating_sub(x.s) / REEMPRUNT_STRIDE_MAX).max(1);

    let mut q = start;
    while q <= end {
        let distance_to_junction = distance2(r, q, x.e);
        // q collé à la jonction : non discriminant.
        if distance_to_junction > 1.0 {
            let mut t = x.e;
            loop {
                let distance_to_x = distance2(r, q, t);
                if distance_to_x < REEMPRUNT_FACTOR * distance_to_junction
                    && distance_to_x <= gate
                {
                    return true;
                }
                if t <= x.s {
                    break;
                }
                t = t.saturating_sub(stride).max(x.s);
            }
        }
        q += REEMPRUNT_STEP;
    }
    false
}

// ─── Phases A, B, C ───────────────────────────────────────────────────

/// Phase A — fusion des items en passages.
///
/// Deux items fusionnent s'ils décrivent le même emprunt, **sauf** s'ils sont
/// liés par un run (le lien est l'anti-fusionnement : ses extrémités décrivent
/// deux passes différentes) ou si leur jonction est une frontière.
///
/// Retourne les passages (enveloppes des classes d'items) et les liens remappés,
/// dédupliqués — un lien dont les deux extrémités retombent dans le même passage
/// est **consommé**, sinon il reconnecterait des entités déjà fusionnées aux
/// itérations suivantes.
fn phase_a(
    items: &[Interval],
    links: &[(usize, usize)],
    r: &Resample,
    tol_m: f64,
    fuse_pts: usize,
) -> (Vec<Interval>, Vec<(usize, usize)>) {
    let linked: BTreeSet<(usize, usize)> = links.iter().map(|&(a, b)| pair_key(a, b)).collect();

    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].s, items[i].e, i));

    let mut uf = UnionFind::new(items.len());
    for (position, &i) in order.iter().enumerate() {
        for &j in &order[position + 1..] {
            // Items triés par `s` : dès que la fente dépasse `fusePts`, aucune
            // paire ultérieure n'est plus proche.
            if items[j].s as isize - items[i].e as isize > fuse_pts as isize {
                break;
            }
            if uf.find(i) == uf.find(j) || linked.contains(&pair_key(i, j)) {
                continue;
            }
            if !same_passage(&items[i], &items[j], fuse_pts) {
                continue;
            }
            // X est celui qui se termine, Y celui qui commence.
            let (x, y) = if items[i].e <= items[j].e {
                (&items[i], &items[j])
            } else {
                (&items[j], &items[i])
            };
            if boundary_between_passes(r, x, y, tol_m) {
                continue;
            }
            uf.union(i, j);
        }
    }

    // Chaque classe devient un passage, défini par son enveloppe. Les indices
    // sont attribués dans l'ordre des items, donc de façon déterministe.
    let mut index_of_root: HashMap<usize, usize> = HashMap::new();
    let mut passages: Vec<Interval> = Vec::new();
    let mut passage_of_item = vec![0usize; items.len()];
    for i in 0..items.len() {
        let root = uf.find(i);
        let index = *index_of_root.entry(root).or_insert_with(|| {
            passages.push(items[i]);
            passages.len() - 1
        });
        passages[index].s = passages[index].s.min(items[i].s);
        passages[index].e = passages[index].e.max(items[i].e);
        passage_of_item[i] = index;
    }

    let mut remapped: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &(a, b) in links {
        let (passage_a, passage_b) = (passage_of_item[a], passage_of_item[b]);
        if passage_a != passage_b {
            remapped.insert(pair_key(passage_a, passage_b));
        }
    }

    (passages, remapped.into_iter().collect())
}

/// Phase B — regroupement des passages en segments, par composantes connexes des
/// liens remappés.
fn phase_b(passages: &[Interval], links: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut uf = UnionFind::new(passages.len());
    for &(a, b) in links {
        uf.union(a, b);
    }

    let mut index_of_root: HashMap<usize, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..passages.len() {
        let root = uf.find(i);
        let index = *index_of_root.entry(root).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[index].push(i);
    }
    groups
}

/// Phase C — unification des références.
///
/// Deux segments distincts ne doivent partager aucune partie commune, ni être
/// séparés de moins de `fusePts`. Le **contact exact** n'est pas absorbé : c'est
/// le cas légitime du demi-tour au sommet d'un col, où la référence de la
/// descente commence là où finit celle de la montagne.
///
/// Retourne les groupes après fusion et indique si au moins une fusion a eu lieu
/// — donc si la boucle doit être rejouée.
fn phase_c(
    groups: Vec<Vec<usize>>,
    passages: &[Interval],
    fuse_pts: usize,
) -> (Vec<Vec<usize>>, bool) {
    // Chaque groupe est caractérisé par sa référence : le passage de plus petit
    // `s`. Le tri par début de référence rend le parcours déterministe.
    let mut ordered: Vec<(Interval, usize)> = groups
        .iter()
        .enumerate()
        .map(|(index, members)| {
            let reference = members
                .iter()
                .copied()
                .map(|i| passages[i])
                .min_by_key(|interval| (interval.s, interval.e))
                .expect("groupe non vide");
            (reference, index)
        })
        .collect();
    ordered.sort_by_key(|(reference, index)| (reference.s, reference.e, *index));

    let mut fused = false;
    let mut result: Vec<Vec<usize>> = Vec::new();
    let mut hull: Option<Interval> = None;
    for (reference, group_index) in ordered {
        if let Some(current) = hull {
            let overlap =
                reference.e.min(current.e) as isize - reference.s.max(current.s) as isize;
            let gap = reference.s as isize - current.e as isize;
            if overlap > 0 || (0 < gap && gap <= fuse_pts as isize) {
                let last = result.last_mut().expect("groupe courant");
                last.extend(groups[group_index].iter().copied());
                hull = Some(Interval {
                    s: current.s.min(reference.s),
                    e: current.e.max(reference.e),
                });
                fused = true;
                continue;
            }
        }
        result.push(groups[group_index].clone());
        hull = Some(reference);
    }

    (result, fused)
}

// ─── Finalisation ─────────────────────────────────────────────────────

/// Fusion ordonnée des emprunts d'un même groupe (étape 1).
///
/// Seul un **recouvrement interne strict** fusionne : le contact simple
/// (`p.s == last.e`) produit deux passages distincts, ce qui préserve la
/// structure aller/retour au demi-tour.
pub fn merge_ordered_passages(intervals: &[Interval]) -> Vec<Interval> {
    let mut ordered: Vec<Interval> = intervals.to_vec();
    ordered.sort_by_key(|interval| (interval.s, interval.e));

    let mut merged: Vec<Interval> = Vec::new();
    for interval in ordered {
        if let Some(last) = merged.last_mut() {
            if interval.s < last.e {
                last.e = last.e.max(interval.e);
                continue;
            }
        }
        merged.push(interval);
    }
    merged
}

/// Passe D — fusion intra-segment (étape 3).
///
/// Recoud deux passages **contigus de même sens** séparés d'un trou court : ce
/// que le chaînage des runs et la fusion des items n'ont pas su rapprocher
/// (virages serrés, tunnels, croisements en X rejetés à l'appariement). Elle ne
/// fusionne que des passages **contigus**, donc ne peut pas créer d'intervalle
/// chevauchant un passage tiers, et la **frontière entre deux sens est
/// intangible** — c'est la seule frontière qu'elle pourrait franchir par erreur.
pub fn merge_within_segment(passages: &[Passage], max_gap_pts: usize) -> Vec<Passage> {
    let mut merged: Vec<Passage> = Vec::new();
    for passage in passages {
        if let Some(last) = merged.last_mut() {
            if last.e <= passage.s
                && passage.s - last.e <= max_gap_pts
                && last.rel == passage.rel
            {
                last.e = passage.e;
                last.km1 = passage.km1;
                last.pt1 = passage.pt1;
                continue;
            }
        }
        merged.push(*passage);
    }
    merged
}

// ─── Assemblage ───────────────────────────────────────────────────────

/// Assemble les runs en segments.
///
/// `tol_m` est la tolérance de superposition, `fuse_m` la distance de fusion des
/// références proches (0 désactive toute fusion automatique).
pub fn build_segments(
    runs: &[Run],
    r: &Resample,
    geom: &MultirideGeom,
    tol_m: f64,
    fuse_m: f64,
) -> Vec<Segment> {
    let fuse_pts = (fuse_m / r.step).round() as usize;
    let max_gap_pts = (MERGE_WITHIN_SEGMENT_FACTOR * fuse_pts as f64).round() as usize;

    // Items : deux intervalles par run. Liens : la paire d'un run, qui exprime
    // que ses deux extrémités décrivent deux emprunts différents.
    let mut items: Vec<Interval> = Vec::with_capacity(runs.len() * 2);
    let mut links: Vec<(usize, usize)> = Vec::with_capacity(runs.len());
    for run in runs {
        let first = items.len();
        items.push(Interval {
            s: run.a0,
            e: run.a1,
        });
        items.push(Interval {
            s: run.b0,
            e: run.b1,
        });
        links.push((first, first + 1));
    }

    // Boucle des phases A → B → C : la fusion de la Phase A change les
    // intervalles, ce qui peut créer de nouvelles adjacences à traiter. La boucle
    // s'arrête à la stabilité de la Phase C, ou au bout du garde-fou.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut passages: Vec<Interval> = Vec::new();
    for _ in 0..MAX_ITERATIONS {
        let (next_passages, next_links) = phase_a(&items, &links, r, tol_m, fuse_pts);
        let (next_groups, fused) = phase_c(
            phase_b(&next_passages, &next_links),
            &next_passages,
            fuse_pts,
        );
        passages = next_passages;
        groups = next_groups;
        if !fused {
            break;
        }
        items = passages.clone();
        links = next_links;
    }

    // Finalisation.
    let mut segments: Vec<Segment> = Vec::new();
    for members in groups {
        let intervals: Vec<Interval> = members.iter().map(|&i| passages[i]).collect();

        // Étape 1 — fusion ordonnée ; un groupe à un seul emprunt n'est pas une
        // répétition.
        let merged = merge_ordered_passages(&intervals);
        if merged.len() < 2 {
            continue;
        }

        // Étape 2 — métadonnées. Le sens du premier emprunt reste `0` : c'est la
        // référence du segment.
        let reference = merged[0];
        let built: Vec<Passage> = merged
            .iter()
            .enumerate()
            .map(|(index, interval)| Passage {
                s: interval.s,
                e: interval.e,
                rel: if index == 0 {
                    0
                } else {
                    relative_direction(
                        r,
                        (reference.s, reference.e),
                        (interval.s, interval.e),
                        tol_m,
                    )
                },
                km0: r.cum[interval.s] / 1000.0,
                km1: r.cum[interval.e] / 1000.0,
                pt0: raw_point_number(geom, r, interval.s),
                pt1: raw_point_number(geom, r, interval.e),
            })
            .collect();

        // Étape 3 — Passe D, puis rejet des groupes ramenés à un seul emprunt.
        let final_passages = merge_within_segment(&built, max_gap_pts);
        if final_passages.len() < 2 {
            continue;
        }

        segments.push(Segment {
            passages: final_passages,
        });
    }

    // Ordre stable : par début de référence, puis par fin.
    segments.sort_by_key(|segment| {
        let reference = segment.passages[0];
        (reference.s, reference.e)
    });
    segments
}
