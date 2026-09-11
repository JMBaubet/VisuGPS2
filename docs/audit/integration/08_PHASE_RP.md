# Livrable 8 — Plan de portage Phase 2 : détecteur RP

**Modules cibles** :
- `src-tauri/src/gpx_audit/rp.rs` (détection)
- `src-tauri/src/gpx_audit/anchor.rs` (ancres d'accès)

**Référence JS** : `docs/audit/reference/verifgpx-V3.0.html`
(fonctions `resampleGeo`, `detectRP`, `rpAnchorIndices`, `segsCross`,
`rpWalk`, `loopDegenerate`, `closingTurn`, `closingDelta`, `turnText`,
`rpNorm180`)

**Spécification normative** : `docs/audit/spec/ANALYSE.md` §6 à §13

**Durée estimée** : 5 à 7 jours (le plus gros morceau du portage)

**Fichiers de test** : les 8 fichiers de `docs/audit/reference/test_files/`

---

## 1. Objectif

Porter en Rust le détecteur de boucles giratoires (`detectRP`) et le
calcul dérivé des ancres d'accès (`rpAnchorIndices`). C'est la partie
la plus complexe du portage, avec 12 étapes algorithmiques subtiles.

**Principe de portage** : reproduction **littérale** de la logique JS.
Aucune amélioration, aucun refactoring. La structure procédurale du JS
est conservée, avec des `struct` de données simples.

---

## 2. Signatures Rust — `rp.rs`

```rust
// src-tauri/src/gpx_audit/rp.rs

use super::types::{AuditPoint, AuditParams, Finding};

/// Structure de la trace rééchantillonnée (ANALYSE §7).
///
/// Le rééchantillonnage à pas constant rend les seuils métriques
/// indépendants de la densité du GPX d'entrée.
pub struct ResampledGeo {
    pub n: usize,       // nombre d'intervalles (n+1 échantillons)
    pub rx: Vec<f64>,   // coordonnées métriques X
    pub ry: Vec<f64>,   // coordonnées métriques Y
    pub cum_r: Vec<f64>,// distance cumulée le long des échantillons
    pub orig: Vec<u32>, // R i -> index de référence (pivot du contrat)
    pub step: f64,      // pas effectif (m)
}

/// Rééchantillonnage à pas constant (ANALYSE §7).
pub fn resample_geo(
    px: &[f64],
    py: &[f64],
    cum: &[f64],
    total: f64,
    step: f64,
) -> ResampledGeo;

/// Détecteur de boucles giratoires (ANALYSE §6 à §12).
pub fn detect_rp(
    points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    ids: &[u32],
    geo_total: f64,
    params: &AuditParams,
) -> Vec<Finding>;

// ─── Sous-fonctions (privées, testables) ─────────────────────────────

/// É2 : test de croisement de deux segments (ANALYSE §9.1).
fn segs_cross(r: &ResampledGeo, a: usize, b: usize) -> bool;

/// Marche de cap sur les échantillons bruts (ANALYSE §11.3).
fn rp_walk(
    r: &ResampledGeo,
    from: usize,
    dir: i32,
    min_m: f64,
    max_seg: usize,
) -> Option<usize>;

/// Normalisation d'un angle dans ]-180, +180] (ANALYSE §8.1).
fn rp_norm_180(a: f64) -> f64;

/// Coin de refermeture au point rb (ANALYSE §11.3).
fn closing_turn(r: &ResampledGeo, rb: usize) -> Option<f64>;

/// Delta de cap entrée/sortie (ANALYSE §11.5).
fn closing_delta(r: &ResampledGeo, ra: usize, rb: usize) -> Option<f64>;

/// Garde anti-aiguille : circularité + miroir (ANALYSE §11.4).
fn loop_degenerate(
    r: &ResampledGeo,
    a: usize,
    b: usize,
    close_thr: f64,
) -> bool;

/// Traduction humaine de la rotation (ANALYSE §12.2).
pub fn turn_text(total: f64) -> String;

// ─── Types internes ──────────────────────────────────────────────────

/// Candidat issu de É1 (proximité) ou É2 (croisement).
#[derive(Debug, Clone)]
struct Candidate {
    s: usize,
    e: usize,
    paire: Option<(usize, usize)>,      // É1
    croisement: Option<(usize, usize)>, // É2
}

/// Groupe fusionné par É3.
#[derive(Debug, Clone)]
struct MergedGroup {
    s: usize,
    e: usize,
    paires: Vec<(usize, usize)>,
    croisements: Vec<(usize, usize)>,
}

/// Paire raffinée (sur points de référence).
#[derive(Debug, Clone, Copy)]
struct RefinedPair {
    a: usize,
    b: usize,
    d: f64,
}

/// Fenêtre refermée bornée (candidat finding RP).
#[derive(Debug, Clone)]
struct Window {
    a: usize,
    b: usize,
    d: Option<f64>,
    total: f64,
    propre: bool,
    pairs_in: Vec<RefinedPair>,
}
```

---

## 3. Signatures Rust — `anchor.rs`

```rust
// src-tauri/src/gpx_audit/anchor.rs

/// Résultat du calcul d'ancres (ANALYSE §13).
pub struct AnchorResult {
    pub up: Option<usize>,   // ancre amont (index de référence) ou None
    pub dn: Option<usize>,   // ancre aval
    pub cx: Option<f64>,     // centre ajusté (métrique) ou None
    pub cy: Option<f64>,
    pub r: Option<f64>,      // rayon médian (m)
}

/// Ancres d'accès d'une boucle RP (ANALYSE §13).
///
/// Calcule le centre (Kåsa), le rayon médian, puis marche vers l'amont
/// et l'aval pour trouver les points de sortie de la zone RP.
///
/// `junc` = début de boucle (index de référence)
/// `ce`   = fin de boucle (index de référence)
/// `close_thr` = seuil de fermeture (paramètre p-close)
pub fn rp_anchor_indices(
    px: &[f64],
    py: &[f64],
    junc: usize,
    ce: usize,
    close_thr: f64,
) -> AnchorResult;

// ─── Sous-fonctions privées ──────────────────────────────────────────

/// Ajustement de cercle par la méthode de Kåsa (ANALYSE §13.2).
/// Retourne (cx, cy, r) ou None si l'ajustement échoue.
fn fit_circle_kasa(samples: &[(f64, f64)]) -> Option<(f64, f64, f64)>;

/// Test d'appartenance à la zone RP (ANALYSE §13.2).
/// Combine disque élargi + superposition au cœur.
fn in_rp_zone(
    px: &[f64],
    py: &[f64],
    i: usize,
    cx: f64,
    cy: f64,
    r: f64,
    close_thr: f64,
    core_samples: &[(f64, f64)],
) -> bool;
```

---

## 4. Constantes (à définir dans `rp.rs`)

```rust
// ─── Rééchantillonnage ────────────────────────────────────────────────
const RP_STEP_DEFAULT: f64 = 4.0;      // pas d'échantillonnage (m)
const RP_MAX_SAMPLES: usize = 120_000; // plafond performance
const RP_MIN_SAMPLES: usize = 12;      // garde d'entrée

// ─── É1 (paires par proximité) ────────────────────────────────────────
const RP_LMAX: f64 = 800.0;            // périmètre max d'un cœur (m)
const RP_MAX_GAP_M: f64 = 4000.0;      // portée max d'une boucle (m)
const RP_MAX_PAIRS: usize = 40_000;    // borne du nombre de paires
const RP_MIN_SEP: usize = 6;           // séparation min (échantillons)

// ─── É2 (croisements) ─────────────────────────────────────────────────
const RP_MAX_TESTS: usize = 1_500_000; // borne des tests de croisement
const RP_EPS_CROSS: f64 = 1e-7;        // seuil colinéarité (m²)

// ─── É4 (anti-aiguille) ───────────────────────────────────────────────
const RP_CIRC_MIN: f64 = 0.10;         // circularité min
const RP_MIRROR_MAX: f64 = 0.95;       // fraction miroir max
const RP_COIN_LO: f64 = 130.0;         // borne basse du coin (°)
const RP_COIN_HI: f64 = 240.0;         // borne haute du coin (°)
const RP_PROPRE_DEG: f64 = 60.0;       // seuil drapeau propre (°)
const RP_QUANT_TOL: f64 = 40.0;        // tolérance quantification (°)

// ─── É5 (publication) ─────────────────────────────────────────────────
const RP_PAIRS_TOP: usize = 6;         // top paires restituées
const RP_ZONE_M: f64 = 80.0;           // largeur zone approche/sortie

// ─── Ancres ───────────────────────────────────────────────────────────
const RP_ANCHOR_DELTA_ABS: f64 = 12.0; // δ absolu (m)
const RP_ANCHOR_DELTA_K: f64 = 0.4;    // δ relatif
const RP_ANCHOR_EXT_M: f64 = 15.0;     // marge d'extension (m)
const RP_ANCHOR_SEARCH_BASE: f64 = 150.0;
const RP_ANCHOR_HARD_M: f64 = 2000.0;  // plafond global (m)
const RP_ANCHOR_PERSIST: usize = 2;    // points consécutifs hors zone
const RP_ANCHOR_MAX_SAMPLES: usize = 200; // échantillonnage du cœur
```

---

## 5. Ordre d'implémentation (12 étapes)

Les 12 étapes sont **regroupées en 5 sous-étapes de validation** pour
Zcode (voir §6). Les numéros ci-dessous suivent ANALYSE §6 à §13.

### Étape 1 — `resample_geo` (ANALYSE §7)

**Logique** :
1. `n = max(1, round(total / step))`, plafonné à `RP_MAX_SAMPLES`
2. `eff = total / n` (pas effectif)
3. Pour `i = 0..=n` :
   - Distance cible `d = min(i * eff, total)`
   - Avancer `k` jusqu'à ce que `cum[k+1] >= d`
   - Interpolation linéaire (paramètre `t`)
   - `cum_r[i] = cum_r[i-1] + hypot(...)`
   - `orig[i] = (i == n) ? m-1 : k`

### Étape 2 — `rp_norm_180` + `turn_text` (ANALYSE §8.1 + §12.2)

**`rp_norm_180(a)`** :
```
x = ((a % 360) + 540) % 360 - 180
return x <= -180 ? 180 : x
```

**`turn_text(total)`** :
- `k = round(total / 360)`
- Si `k >= 1` et `|total - 360k| <= 30` → `"tour complet (1×)"` ou `"k tours complets"`
- Sinon si `total < 240` → `"demi-tour dépassé"`
- Sinon : décomposition en base + quart
  - `base = floor(total / 360)`
  - `q = round((total - 360*base) / 90)`
  - Table `['', '1/4 de tour', '1/2 tour', '3/4 de tour', 'tour complet']`

### Étape 3 — Caps lissés + cumul angulaire (ANALYSE §8.1)

**Logique** :
1. Pour `k = 1..n-1` : `head[k] = atan2(RY[k+1] - RY[k-1], RX[k+1] - RX[k-1]) * 180/π`
2. Si `hypot < 1e-9` : `head[k] = NaN`
3. `pre[0] = pre[1] = 0`
4. Pour `k = 2..n-1` :
   - `d = head[k] - head[k-1]` normalisé à `]-180, +180]`
   - `pre[k] = pre[k-1] + d`
5. `angle_between(i, j) = pre[j] - pre[i]` (O(1))

### Étape 4 — É1 : paires par proximité spatiale (ANALYSE §8.2)

**Logique** :
1. `cell = max(close_thr, 1.0)`
2. `max_gap = min(n, ceil(RP_MAX_GAP_M / step) + 8)`
3. Grille `HashMap<(i32, i32), Vec<usize>>`
4. Pour `i = 0..=n` :
   - Explorer les 9 cellules voisines
   - Pour chaque `j` de la cellule :
     - Skip si `i - j < RP_MIN_SEP` ou `i - j > max_gap`
     - Si `hypot(RX[i] - RX[j], RY[i] - RY[j]) < close_thr` :
       - `cands += Candidate { s: j, e: i, paire: Some((j, i)) }`
       - Sortir si `|cands| >= RP_MAX_PAIRS`
   - **Insérer `i` APRÈS** la comparaison (insertion différée)

### Étape 5 — `segs_cross` + É2 (ANALYSE §9)

**`segs_cross(a, b)`** : deux niveaux.
- Niveau 1 : croisement strict par produits vectoriels
- Niveau 2 : contact quasi exact (colinéarité, EPS = 1e-7, expansion 1e-6 m)

**É2** :
1. `cell2 = max(close_thr, 3.0 * step)`
2. Grille de segments `HashMap<(i32, i32), Vec<usize>>`
3. Pour `k = 0..n-1` :
   - Explorer les cellules de la boîte englobante de `[k, k+1]`
   - Pour chaque segment `m` déjà indexé :
     - Skip si `k - m < 3` (adjacents)
     - Clé `m * 1e6 + k` pour dédoublonner
     - Incrémenter `tests`, sortir si `> RP_MAX_TESTS`
     - Si `segs_cross(m, k)` : `cands += { s: m, e: k+1, croisement: Some((m, k+1)) }`
   - Indexer `[k, k+1]` dans ses cellules

### Étape 6 — É3 : fusion des candidats (ANALYSE §10)

**Logique** :
1. Trier `cands` par `(s, e)`
2. Fusion glissante :
   - Si `c.s <= last.e` : étendre `last.e = max(last.e, c.e)` et accumuler
   - Sinon : nouveau groupe
3. Attention : l'intervalle fusionné est un **conteneur**, pas une anomalie

### Étape 7 — É4a : raffinage des paires (ANALYSE §11.1)

**Logique** :
1. Pour chaque groupe, pour chaque paire `(a, b)` :
   - Explorer `da = -2..=2`, `db = -2..=2` (25 combinaisons)
   - Trouver le `(ia, ib)` minimisant `refDist(ia, ib)`
   - `refDist` utilise `orig[ia]`, `orig[ib]` pour lire `px`/`py` originaux
2. Dédoublonner par clé `best.a * 1e6 + best.b`
3. `dMin = min` des distances raffinées

### Étape 8 — `rp_walk` + `closing_turn` + `closing_delta` (ANALYSE §11.3 + §11.5)

**`rp_walk(from, dir, min_m = 8, max_seg = 4)`** :
- Marche dans `dir` en accumulant les distances
- Micro-segments < 0,05 m sautés
- S'arrête quand `acc >= min_m` ou `segs >= max_seg`
- Retourne `Some(i)` ou `None`

**`closing_turn(rb)`** :
- `ja = rp_walk(rb, -1)`, `ka = rp_walk(rb, +1)`
- Cap entrant : `atan2(RY[rb]-RY[ja], RX[rb]-RX[ja])`
- Cap sortant : `atan2(RY[ka]-RY[rb], RX[ka]-RX[rb])`
- Retourne la différence normalisée

**`closing_delta(ra, rb)`** :
- Identique mais symétrique (comparer sortie de `b` et entrée de `a`)

### Étape 9 — `loop_degenerate` (ANALYSE §11.4)

**Test 1 — circularité** :
```
A2 = Σ (x_i·y_{i+1} - x_{i+1}·y_i) + (x_b·y_a - x_a·y_b)
L  = Σ |P(i+1) - P(i)| + |P(b) - P(a)|
circ = 4π · |A2/2| / L²
rejet si circ < 0,10
```

**Test 2 — miroir** :
```
pour i de a à b :
    miroir = b - (i - a)
    si dist(i, miroir) < closeThr : near++
rejet si near / total >= 0,95
```

**Rejet si Test 1 OU Test 2 déclenche.**

### Étape 10 — É4b/c/d : fenêtres + quantification + sélection (ANALYSE §11.2 + §11.6)

**`push_window(a, b, d, with_coin)`** :
1. Rejet si `b - a < RP_MIN_SEP` ou `cumR[b] - cumR[a] > RP_LMAX`
2. `ang = angle_between(a, b)`
3. `coin = 0`
   - Si `with_coin && d <= dMin + max(2, 0.25 * closeThr)` :
     - `t = closing_turn(b)`
     - Si `|t| ∈ [130, 240]` : aligner le signe si nécessaire
     - `coin = t`
4. `total = |ang + coin|`
5. **Quantification** : `k = round(total / 360)` ; si `k >= 1` et `|total - 360k| <= 40` → `total = 360 * k`
6. Rejet si `total < p-angle`
7. Rejet si `loop_degenerate(a, b, closeThr)`
8. `dc = with_coin ? closing_delta(a, b) : None`
9. `propre = with_coin ? (dc == None || |dc| >= 60) : true`

**`pairs_in`** (É4d) : paires raffinées contenues dans `[a, b]`

**Sélection gloutonne** (É4e) :
- Tri : `propre` desc, `total` desc, `d` asc, `span` asc
- Retenir si pas de chevauchement avec les retenues
- Retrier par `a` croissant

### Étape 11 — É5 : publication (ANALYSE §12)

**Logique** :
1. `W = max(2, round(RP_ZONE_M / step))`
2. Pour chaque fenêtre retenue (index `wi`) :
   - `cs_ref = orig[w.a]`, `ce_ref = orig[w.b]`
   - Rejet si `ce_ref <= prev_e_ref`
   - `s_ref = max(0, prev_e_ref + 1, orig[max(0, w.a - W)])`, borné à `cs_ref`
   - `e_ref = min(m-1, next_cs_ref - 1, orig[min(n, w.b + W)])`, borné à `ce_ref`
   - **Élargissement D2 aux ancres** : `rp_anchor_indices` puis extension
   - Re-clamp si nécessaire
   - `prev_e_ref = e_ref`
3. Construire le `Finding` complet :
   - `label = total > 340 ? "Tour de rond-point" : "Boucle giratoire"`
   - `summary`, `turn_text`, `total_angle`
   - `pairs` (top 6 triées par `d`)
   - `zoneIds`, `coreIds`, `ctxIds`, `ctx`, `parts`

### Étape 12 — `anchor.rs` complet (ANALYSE §13)

**`fit_circle_kasa`** :
1. Centroïde des `samples`
2. Cumuls : `Suu, Suv, Svv, Su, Sv, nn, Suz, Svz, Sz`
3. Déterminant de la matrice 3×3
4. Résolution par Cramer
5. `rFit = sqrt(max(0, c + a²/4 + b²/4))`
6. Vérifier `rFit ∈ [3, 1000]`

**`in_rp_zone`** :
- Disque : `dist(i, centre) <= r + δ` avec `δ = max(12, 0.4 * r)`
- OU superposition : `dist(i, échantillon du cœur) <= closeThr`

**`rp_anchor_indices`** :
1. Échantillonner le cœur (stride pour ≤ 200 points)
2. Kåsa → centre + rayon
3. Rayon médian
4. `walk_side(start, dir)` :
   - Marche jusqu'à sortie de zone confirmée (2 points consécutifs)
   - Garde sur distance **hors zone** (pas totale)
   - Plafond dur 2000 m
   - Cas C1 : borne de trace = confirmation
5. Extension de 15 m après le plancher
6. Retourner `AnchorResult { up, dn, cx, cy, r }`

---

## 6. Sous-étapes de validation Zcode

**Décision Q8** : validation à chaque sous-étape. Les 12 étapes
algorithmiques sont regroupées en **5 sous-étapes** cohérentes.

### Sous-étape 2.1 — Rééchantillonnage et helpers

**Étapes couvertes** : 1, 2
**Fonctions** : `resample_geo`, `rp_norm_180`, `turn_text`
**Tests** : 9 tests

### Sous-étape 2.2 — Candidats géométriques (É1 + É2)

**Étapes couvertes** : 3, 4, 5
**Fonctions** : cumul angulaire, grille de hachage, `segs_cross`
**Tests** : 12 tests

### Sous-étape 2.3 — Fusion et raffinage (É3 + É4a)

**Étapes couvertes** : 6, 7
**Fonctions** : fusion, raffinage sur points de référence
**Tests** : 6 tests

### Sous-étape 2.4 — Fenêtres bornées (É4b/c/d + anti-aiguille)

**Étapes couvertes** : 8, 9, 10
**Fonctions** : `rp_walk`, `closing_turn`, `closing_delta`,
`loop_degenerate`, `push_window`, sélection gloutonne
**Tests** : 14 tests

### Sous-étape 2.5 — Publication et ancres (É5 + anchor.rs)

**Étapes couvertes** : 11, 12
**Fonctions** : `rp_anchor_indices`, `fit_circle_kasa`, `in_rp_zone`,
publication des findings
**Tests** : 10 tests

**Total : 51 tests** (au lieu des 34 annoncés dans le Livrable 4 —
révision à la hausse pour couvrir les 5 sous-étapes).

---

## 7. Scénarios de validation §17.2

Fichiers dans `docs/audit/reference/test_files/`.

| Fichier | Motif | Verdict attendu (défauts 15 m / 270°) |
|---|---|---|
| `rondpoints_g1.gpx` | Tour complet + branches communes | 360°, fenêtre **propre**, paires 0,0 m affichées |
| `rondpoints_g2.gpx` | Tour complet | 360° |
| `rondpoints_g3.gpx` | Refermeture non exacte (~12 m) | 330–350° bruts → **360°** après quantification |
| `RP_Santa_Susanna.gpx` | Tour complet + réengagement branche d'entrée | ~175° lissé + ~164° de coin → 360° |
| `AR_Santa_Susanna.gpx` | **Aiguille** (aller-retour exact) | Rejetée par RP (anti-aiguille) + détectée par AR |
| `AR_Detecté_aussi_en_RP.gpx` | Aiguille ambiguë | Rejetée par RP + détectée par AR |
| `RP_erreur_Magny.gpx` | A-R macroscopique + giratoire 4 tours | **1 anomalie** : 1440° « 4 tours complets », fenêtre bornée |

**Note** : le scénario `scenario_17_1_ar_27pts.gpx` (Livrable 8 initial)
concernait l'AR, pas le RP. Il n'est pas dans ce tableau.

---

## 8. Jeux de tests complets (`tests/rp_test.rs`)

```rust
// src-tauri/src/gpx_audit/tests/rp_test.rs

use super::rp::*;
use crate::gpx_audit::types::*;

// ─── Sous-étape 2.1 : rééchantillonnage et helpers ───────────────────
#[test] fn test_resample_geo_linear() { /* ... */ }
#[test] fn test_resample_geo_loop() { /* ... */ }
#[test] fn test_resample_geo_max_samples() { /* ... */ }
#[test] fn test_resample_geo_zero_length() { /* ... */ }
#[test] fn test_rp_norm_180_positive() { /* ... */ }
#[test] fn test_rp_norm_180_negative() { /* ... */ }
#[test] fn test_rp_norm_180_boundary() { /* ... */ }
#[test] fn test_turn_text_complete_turn() { /* ... */ }
#[test] fn test_turn_text_multiple_turns() { /* ... */ }
#[test] fn test_turn_text_partial() { /* ... */ }
#[test] fn test_turn_text_uturn() { /* ... */ }

// ─── Sous-étape 2.2 : candidats géométriques ─────────────────────────
#[test] fn test_heading_cumul_circle() { /* ... */ }
#[test] fn test_heading_cumul_s_curve() { /* ... */ }
#[test] fn test_heading_cumul_straight() { /* ... */ }
#[test] fn test_e1_proximity_grid() { /* ... */ }
#[test] fn test_e1_min_separation() { /* ... */ }
#[test] fn test_e1_max_gap() { /* ... */ }
#[test] fn test_e1_max_pairs_truncation() { /* ... */ }
#[test] fn test_e1_deferred_insertion() { /* ... */ }
#[test] fn test_e2_segs_cross_strict() { /* ... */ }
#[test] fn test_e2_segs_cross_colinear() { /* ... */ }
#[test] fn test_e2_no_adjacent() { /* ... */ }
#[test] fn test_e2_dedup_key() { /* ... */ }

// ─── Sous-étape 2.3 : fusion et raffinage ────────────────────────────
#[test] fn test_e3_fusion_overlap() { /* ... */ }
#[test] fn test_e3_fusion_disjoint() { /* ... */ }
#[test] fn test_e3_fusion_containers() { /* ... */ }
#[test] fn test_e4a_refine_pair() { /* ... */ }
#[test] fn test_e4a_dedup() { /* ... */ }
#[test] fn test_e4a_dmin() { /* ... */ }

// ─── Sous-étape 2.4 : fenêtres bornées ───────────────────────────────
#[test] fn test_rp_walk_min_distance() { /* ... */ }
#[test] fn test_rp_walk_micro_segments() { /* ... */ }
#[test] fn test_rp_walk_bounds() { /* ... */ }
#[test] fn test_closing_turn_uturn() { /* ... */ }
#[test] fn test_closing_turn_straight() { /* ... */ }
#[test] fn test_closing_delta_straight() { /* ... */ }
#[test] fn test_closing_delta_turn() { /* ... */ }
#[test] fn test_loop_degenerate_uturn() { /* ... */ }
#[test] fn test_loop_degenerate_circle() { /* ... */ }
#[test] fn test_loop_degenerate_partial_mirror() { /* ... */ }
#[test] fn test_e4_window_lmax() { /* ... */ }
#[test] fn test_e4_window_quantification() { /* ... */ }
#[test] fn test_e4_window_propre() { /* ... */ }
#[test] fn test_e4_greedy_selection() { /* ... */ }

// ─── Sous-étape 2.5 : publication et ancres ──────────────────────────
#[test] fn test_e5_convert_indices() { /* ... */ }
#[test] fn test_e5_pairs_top_6() { /* ... */ }
#[test] fn test_e5_junction_is_start() { /* ... */ }
#[test] fn test_anchor_fit_circle() { /* ... */ }
#[test] fn test_anchor_median_radius() { /* ... */ }
#[test] fn test_anchor_walk_radial() { /* ... */ }
#[test] fn test_anchor_walk_persist() { /* ... */ }
#[test] fn test_anchor_bord_de_trace() { /* ... */ }
#[test] fn test_anchor_repli_silencieux() { /* ... */ }
#[test] fn test_anchor_hors_zone_distance() { /* ... */ }

// ─── Scénarios complets §17.2 ────────────────────────────────────────
#[test] fn test_scenario_rondpoints_g1() { /* ... */ }
#[test] fn test_scenario_rondpoints_g2() { /* ... */ }
#[test] fn test_scenario_rondpoints_g3() { /* ... */ }
#[test] fn test_scenario_rp_santa_susanna() { /* ... */ }
#[test] fn test_scenario_ar_santa_susanna_rejected() { /* ... */ }
#[test] fn test_scenario_ar_detecte_aussi_en_rp() { /* ... */ }
#[test] fn test_scenario_rp_erreur_magny_4_turns() { /* ... */ }

// ─── Helper : chargement GPX ─────────────────────────────────────────
fn load_gpx(filename: &str) -> Vec<AuditPoint> {
    // Charge docs/audit/reference/test_files/{filename}
    // Parse avec le crate gpx
    // Retourne Vec<AuditPoint>
}
```

---

## 9. Points de vigilance (leçons documentées ANALYSE §16)

| # | Piège | Mitigation |
|---|---|---|
| 1 | **Lissage ±1 annule le demi-tour de réengagement** (mesure ~175° au lieu de ~340°) | `closingTurn` compense (mesure sur échantillons bruts) |
| 2 | **Anti-aiguille** : deux demi-tours de même sens = ±360° indiscernable du tour par le cap seul | Circularité + miroir (Test 1 OU Test 2) |
| 3 | **Méga-paire** de ~2 km englobe 8 tours du vrai giratoire | Fenêtres bornées `RP_LMAX = 800 m` + glouton disjoint |
| 4 | **Quantification** : un tour complet oscille 320–400° bruts | Tolérance ±40° → alignement sur multiple de 360° |
| 5 | **Conversion `orig[]`** : toute grandeur publiée doit passer par `R.orig` | Sinon index hors tableau → « Invalid LatLng NaN » |
| 6 | **Jonction = début de boucle** (pas milieu de la paire) | Sinon tombe au centre du giratoire quand voies superposées |
| 7 | **Ancres** : garde sur distance **hors zone** (pas totale) | Un long re-parcours superposé épuise la portée totale sinon |
| 8 | **Ancres** : repli silencieux (`null` → `junc-1` / `ce+1`) | Ne jamais paniquer |
| 9 | **É3** : l'intervalle fusionné est un conteneur, jamais une anomalie | Les fenêtres bornées (É4) démêlent |
| 10 | **É1 insertion différée** : insérer `i` dans la grille **après** la comparaison | Sinon chaque paire est examinée deux fois |
| 11 | **É2 dédoublonnage** : clé `m * 1e6 + k` | Garantie par `RP_MAX_SAMPLES = 120_000 < 10⁶` |
| 12 | **É2 contact colinéaire** : seuil 1e-7 (superposition au cm) | Un seuil plus large crée des faux contacts entre voies |
| 13 | **Rayon médian** des distances au centre (pas moyen) | Robuste aux aberrants |
| 14 | **Kåsa adopté seulement si** `rFit ∈ [3, 1000]` | Sinon repli sur centroïde |
| 15 | **Extension des ancres** : 15 m après le plancher, interrompue si re-entry | L'ancre marque la frontière, pas le milieu de la route |

---

## 10. Critère de passage Phase 2 → Phase 3

**Tous les critères suivants doivent être satisfaits** :

- [ ] `cargo check` sans warning ni erreur
- [ ] `cargo test --lib gpx_audit::rp` : **51 tests verts**
- [ ] `cargo test --lib gpx_audit::anchor` : inclus dans les 51
- [ ] Les 7 scénarios §17.2 produisent les verdicts documentés
- [ ] Le test `test_scenario_ar_santa_susanna_rejected` confirme
      que l'aiguille est bien rejetée par RP (anti-aiguille)
- [ ] Le test `test_scenario_rp_erreur_magny_4_turns` produit 1440°
      sur une fenêtre bornée au giratoire (méga-paire exclue)
- [ ] Aucune déviation par rapport à `detectRP` du HTML de référence

Toute déviation doit être **justifiée par écrit** (commentaire dans le
code ou note dans le commit) avant de passer à la Phase 3.

---

## 11. Compte-rendu attendu de Zcode

À la fin de la Phase 2, Zcode doit fournir :

1. **Résultat de `cargo test --lib gpx_audit::rp`** : 51 tests verts
2. **Contenu complet de `rp.rs` et `anchor.rs`**
3. **Points de doute rencontrés** et comment ils ont été résolus
4. **Déviations justifiées** par rapport au HTML de référence
5. **Temps réel passé** sur chaque sous-étape (2.1 à 2.5)
6. **Performance mesurée** : durée moyenne de `detect_rp` sur les
   fichiers de test (attendu : < 200 ms pour une trace de 5 000 points)

---

## 12. Notes sur le style de portage

- **Reproduire la structure du JS** : garder les noms de variables
  (`cands`, `merged`, `refined`, `windows`, `taken`, `pairsIn`),
  les ordres de calcul, les conditions exactes.
- **Grilles de hachage** : utiliser `std::collections::HashMap<(i32, i32), Vec<usize>>`
  (pas de dépendance externe).
- **Tri stable** : `Vec::sort_by` (stable en Rust).
- **Comparaisons flottantes** : utiliser des seuils explicites
  (`1e-9`, `1e-7`, `0.05`), jamais `==` sur `f64`.
- **Pas de panic** : tous les accès tableau sont bornés.
- **Commentaires** : en français, uniquement quand la logique du JS
  n'est pas évidente (ex. justifier le `+ 540 % 360 - 180`).
- **Logging** : aucun log dans `detect_rp` (fonction pure). Les
  commandes Tauri se chargent du logging (Livrable 4).