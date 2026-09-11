# Livrable 3 — Plan de portage Phase 1 : détecteur AR

**Module cible** : `src-tauri/src/gpx_audit/ar.rs`

**Référence JS** : `docs/audit/reference/verifgpx-V3.0.html`
(fonction `detectAR`, lignes ~430-540)

**Spécification normative** : `docs/audit/spec/ANALYSE.md` §5

**Durée estimée** : 1,5 à 2 jours

**Fichier de test** : `docs/audit/reference/test_files/scenario_17_1_ar_27pts.gpx`

---

## 1. Objectif

Porter en Rust la fonction `detectAR` du HTML de référence. Cette phase
est **indépendante** de la détection RP (Phase 2) : elle produit un
détecteur complet, testé, validé, qui pourra être utilisé seul.

**Principe de portage** : reproduction **littérale** de la logique JS.
Aucune amélioration, aucun refactoring élégant, aucune abstraction
nouvelle. La structure procédurale du JS est conservée (fonctions libres
+ structs simples).

---

## 2. Signatures Rust (`ar.rs`)

```rust
// src-tauri/src/gpx_audit/ar.rs

use super::types::{AuditPoint, AuditParams, Finding};

/// Portage fidèle de `detectAR` (verifgpx-V3.0.html §5).
///
/// # Préconditions
/// - `points.len() >= 5`
/// - `points` consolidée (cf. consolidation.rs)
/// - `px`, `py`, `ids` : tableaux parallèles de même longueur que `points`
///
/// # Postconditions
/// - Findings triés par `parts[0].s` croissant
/// - Emprises disjointes (`f[i].parts[0].s > f[i-1].parts[0].e`)
/// - Chaque finding a `kind = FindingKind::Ar`
pub fn detect_ar(
    points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    ids: &[u32],
    params: &AuditParams,
) -> Vec<Finding>;

// ─── Sous-fonctions privées (testables individuellement) ─────────────

/// Phase 1 — caps des segments (atan2, filet EPS).
fn compute_headings(px: &[f64], py: &[f64]) -> Vec<f64>;

/// Phase 2 — candidats sommets (C1 retournement + C2 branches).
fn find_candidates(
    px: &[f64],
    py: &[f64],
    headings: &[f64],
    margin: usize,
    params: &AuditParams,
) -> Vec<Candidate>;

/// Phase 3 — groupage (R1 voisins + R2 fuse).
fn group_candidates(candidates: &[Candidate]) -> Vec<Candidate>;

/// Phase 4 — paires miroirs symétriques autour d'un sommet.
fn find_mirror_pairs(
    peak: usize,
    px: &[f64],
    py: &[f64],
    params: &AuditParams,
    prev_end: Option<usize>,
) -> Vec<MirrorPair>;

// ─── Types internes ──────────────────────────────────────────────────

/// Candidat sommet produit par `find_candidates`.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    i: usize,     // index du sommet
    diff: f64,    // écart à 180° (degrés)
    d1: f64,      // branche amont (m)
    d2: f64,      // branche aval (m)
}

/// Paire miroir détectée par `find_mirror_pairs`.
#[derive(Debug, Clone, Copy)]
struct MirrorPair {
    a: usize,     // index amont
    b: usize,     // index aval
    d: f64,       // distance (m)
}
```

---

## 3. Constantes (à définir dans `ar.rs` ou `constants.rs`)

```rust
/// Filet de sécurité des caps : segment plus court -> cap reporté.
const EPS: f64 = 0.05;

/// Seuil « trace fermée » : d(P0, P_last) < 100 m -> marge 3.
const CLOSED_TRACE_THRESHOLD_M: f64 = 100.0;

/// Écart maximal de groupage de deux sommets (points).
const FUSE_GAP: usize = 6;
```

---

## 4. Ordre d'implémentation (strict, 5 sous-étapes)

### Sous-étape 1 — `compute_headings`

**Référence** : ANALYSE §5.2

**Logique** :
1. Pour chaque segment `j = 0..m-2`, calculer
   `atan2(dx, dy) * 180/π + 360 % 360` (azimut depuis le nord)
2. Si `hypot(dx, dy) < EPS`, reporter le cap précédent (`prev_head`)
3. Parcours arrière pour reporter les caps indéfinis
4. Si `head[0]` reste indéfini → retour `[]` (trace immobile)

**Signature** : `fn compute_headings(px: &[f64], py: &[f64]) -> Vec<f64>`

**Tests à écrire** :
```rust
#[test]
fn test_compute_headings_linear() {
    // 3 points alignés nord : 45.0, 45.001, 45.002 / lon 2.0
    // -> caps attendus : [0.0, 0.0] (heading = 0 = nord)
}

#[test]
fn test_compute_headings_east() {
    // 3 points alignés est : lat 45.0 / lon 2.0, 2.001, 2.002
    // -> caps attendus : [90.0, 90.0]
}

#[test]
fn test_compute_headings_eps() {
    // micro-segments < EPS doivent reporter le cap précédent
}
```

**Critère de validation** : 3 tests verts, aucune erreur `cargo check`.

---

### Sous-étape 2 — `find_candidates`

**Référence** : ANALYSE §5.3

**Logique** :
1. Calculer la marge (`3` si trace fermée, sinon `1`)
2. Pour `i` dans `[margin, m-1-margin]` :
   - **C1** : `diff = |head[i] - head[i-1]|` réduite à `[0, 180]`.
     Retenir si `180 - diff ≤ p-tol` (tolérance angulaire)
   - **C2** : `d1 = hypot(P[i] - P[i-1])`, `d2 = hypot(P[i+1] - P[i])`.
     Retenir si `d1 ≤ p-seg` et `d2 ≤ p-seg`
3. Retourner la liste des `Candidate { i, diff, d1, d2 }`

**Signature** :
`fn find_candidates(px, py, headings, margin, params) -> Vec<Candidate>`

**Tests à écrire** :
```rust
#[test]
fn test_find_candidates_single_uturn() {
    // Trace en L avec retournement 180° au point 5
    // -> 1 candidat au point 5
}

#[test]
fn test_find_candidates_branches_too_long() {
    // Trace avec d1 = 500 m, p-seg = 200 m
    // -> 0 candidat (C2 rejette)
}

#[test]
fn test_find_candidates_margin() {
    // Trace de 10 points, trace ouverte (marge 1)
    // -> les indices 0 et 9 ne sont jamais candidats
}
```

**Critère de validation** : 3 tests verts.

---

### Sous-étape 3 — `group_candidates`

**Référence** : ANALYSE §5.4

**Logique** :
1. Trier les candidats par `i` croissant
2. Grouper par chaînage :
   - **R1** : écart au dernier membre du groupe `< 3` → fusion inconditionnelle
   - **R2** : écart `≤ FUSE_GAP` (6) et **l'une des conditions** :
     - caps « jumeaux » : `|head[g[0].i - 1] - head[c.i]| ≤ p-tol`
     - caps « opposés » : `|180 - même écart| ≤ p-tol`
     - sommets proches : `dist(P[c.i], P[dernier membre]) ≤ p-pair`
3. Représentant du groupe = candidat **médian** (minimise `|c.i - milieu|`,
   à égalité : `diff` maximal, puis premier)

**Signature** : `fn group_candidates(candidates: &[Candidate]) -> Vec<Candidate>`

**Tests à écrire** :
```rust
#[test]
fn test_group_candidates_r1_chain() {
    // 5 candidats consécutifs (i = 10..14)
    // -> 1 groupe, représentant = candidat médian
}

#[test]
fn test_group_candidates_r2_fuse() {
    // 2 candidats à i = 5 et i = 9 (écart 4 ≤ FUSE_GAP)
    // + caps jumeaux -> fusion
}

#[test]
fn test_group_candidates_median() {
    // 3 candidats : i = 10, 11, 12 / milieu = 11
    // -> représentant = i = 11
}
```

**Critère de validation** : 3 tests verts.

---

### Sous-étape 4 — `find_mirror_pairs`

**Référence** : ANALYSE §5.5

**Logique** :
1. Pour `k = 1..=p-maxpairs` :
   - `a = i - k - 1`, `b = i + k + 1` (symétrie stricte par index)
   - Si `a < 0` ou `b > m-1` ou `a ≤ prev_end` : sortir
   - `d = hypot(P[b] - P[a])`
   - Si `d > p-pair` : **sortir** (arrêt à la première rejetée)
   - Sinon : accumuler `MirrorPair { a, b, d }`
2. Retourner la liste (ordre k croissant)

**Signature** :
`fn find_mirror_pairs(peak, px, py, params, prev_end) -> Vec<MirrorPair>`

**Tests à écrire** :
```rust
#[test]
fn test_find_mirror_pairs_zero_distance() {
    // Trace avec 3 paires à 0,0 m autour du sommet
    // -> 3 paires : d = [0.0, 0.0, 0.0]
}

#[test]
fn test_find_mirror_pairs_stops_on_first_reject() {
    // Trace avec paires à 0, 0, 100 m (p-pair = 50)
    // -> 2 paires (arrêt à la 3e)
}

#[test]
fn test_find_mirror_pairs_budget_kmax() {
    // Trace avec 10 paires possibles, p-maxpairs = 5
    // -> 5 paires maximum
}
```

**Critère de validation** : 3 tests verts.

---

### Sous-étape 5 — `detect_ar` (assemblage)

**Référence** : ANALYSE §5.1 (vue d'ensemble), §5.6 (publication)

**Logique** : orchestrer les 4 sous-fonctions :

```
1. Garde : m < 5 -> return []
2. Calculer margin (3 ou 1 selon trace fermée)
3. headings = compute_headings(px, py)
4. Si headings vide -> return []
5. candidates = find_candidates(px, py, headings, margin, params)
6. peaks = group_candidates(candidates)
7. Pour chaque peak (ordre croissant) :
   a. pairs = find_mirror_pairs(peak.i, px, py, params, prev_end)
   b. k = pairs.len()
   c. Calculer s = peak.i - k - 1, e = peak.i + k + 1
   d. Si s <= prev_end -> ignorer (emprise incluse)
   e. Construire le Finding complet (ANALYSE §5.6) :
      - kind, label, summary, peak, peak_id
      - pairs, pair_idx
      - ecart, d1, d2
      - zone_ids, ctx_ids, ctx
      - status, correction, undo
      - parts[0] (emprise), parts[1] (cœur)
   f. prev_end = e
8. Retour findings
```

**Signature** :
`pub fn detect_ar(points, px, py, ids, params) -> Vec<Finding>`

**Tests à écrire** :
```rust
#[test]
fn test_detect_ar_scenario_17_1() {
    // Fichier : docs/audit/reference/test_files/scenario_17_1_ar_27pts.gpx
    // Après consolidation 0.5m : 23 points, 1 finding AR
    // Assertions détaillées ci-dessous.
}

#[test]
fn test_detect_ar_scenario_17_1_p_pair_30() {
    // Même fichier, p-pair = 30
    // -> 1 finding, 2 paires (au lieu de 3), emprise pts 9-15
}

#[test]
fn test_detect_ar_scenario_17_1_p_maxpairs_1() {
    // Même fichier, p-maxpairs = 1
    // -> 1 finding, 1 paire (10↔14), emprise pts 10-14
}

#[test]
fn test_detect_ar_scenario_17_1_p_consol_0() {
    // Même fichier, p-consol = 0
    // -> 27 points analysés, filet EPS opérationnel
}

#[test]
fn test_detect_ar_empty() {
    // Trace < 5 points -> []
}

#[test]
fn test_detect_ar_immobile() {
    // Trace immobile (tous points identiques) -> []
}
```

**Critère de validation** : 6 tests verts + scénario §17.1 exact.

---

## 5. Scénario de validation §17.1 (jeu AR — 27 points)

**Fichier** : `docs/audit/reference/test_files/scenario_17_1_ar_27pts.gpx`

**Entrée** :
- 27 points dans le fichier
- `p-consol = 0.5 m`
- Autres paramètres par défaut : `p-tol = 20`, `p-pair = 50`,
  `p-maxpairs = 5`, `p-seg = 200`

**Résultat attendu** :

```
1. Après consolidation : working.len() == 23
   4 points supprimés : indices fichier 5, 10, 11, 14 (0-based)

2. findings.len() == 1

3. Finding AR :
   - kind            = FindingKind::Ar
   - label           = "Aller-retour : 1"
   - peak            = 11 (index base 0 dans working)
   - parts[0].s      = 7
   - parts[0].e      = 15
   - pairs (3) :
       * { a: 9,  b: 13, d: 0.0  }   // pts 10 ↔ 14
       * { a: 8,  b: 14, d: 0.0  }   // pts 9  ↔ 15
       * { a: 7,  b: 15, d: 40.0 }   // pts 8  ↔ 16
   - ctx.up          = 6   (pt 7)
   - ctx.dn          = 16  (pt 17)
   - summary         = "sommet pt 12 · 3 paires · écart 0°"
   - ecart           = 0.0
   - d1              = 100.0 (branche amont, m)
   - d2              = 100.0 (branche aval, m)
   - status          = FindingStatus::Pending
   - correction      = None
   - undo            = None

4. Invariants :
   - peak ∈ [parts[0].s, parts[0].e]
   - Cohérence ids : peak_id = ids[11], zone_ids = ids[7..=15]
   - pair_idx = [9, 13, 8, 14, 7, 15]
```

**Tests de sensibilité** (variantes) :

| Variante | Résultat attendu |
|---|---|
| `p-pair = 70` | Identique au défaut (pas de paire supplémentaire) |
| `p-pair = 30` | 1 finding, 2 paires (10↔14, 9↔15), emprise pts 9-15, ctx pts 8 et 16 |
| `p-maxpairs = 1` | 1 finding, 1 paire (10↔14), emprise pts 10-14, ctx pts 9 et 15 |
| `p-consol = 0` | 27 points analysés (filet EPS opérationnel) |

---

## 6. Jeux de tests complets (`tests/ar_test.rs`)

```rust
// src-tauri/src/gpx_audit/tests/ar_test.rs

use super::ar::*;
use crate::gpx_audit::types::*;

// ─── Sous-étape 1 : compute_headings ─────────────────────────────────
#[test] fn test_compute_headings_linear() { /* ... */ }
#[test] fn test_compute_headings_east() { /* ... */ }
#[test] fn test_compute_headings_eps() { /* ... */ }

// ─── Sous-étape 2 : find_candidates ──────────────────────────────────
#[test] fn test_find_candidates_single_uturn() { /* ... */ }
#[test] fn test_find_candidates_branches_too_long() { /* ... */ }
#[test] fn test_find_candidates_margin() { /* ... */ }

// ─── Sous-étape 3 : group_candidates ─────────────────────────────────
#[test] fn test_group_candidates_r1_chain() { /* ... */ }
#[test] fn test_group_candidates_r2_fuse() { /* ... */ }
#[test] fn test_group_candidates_median() { /* ... */ }

// ─── Sous-étape 4 : find_mirror_pairs ────────────────────────────────
#[test] fn test_find_mirror_pairs_zero_distance() { /* ... */ }
#[test] fn test_find_mirror_pairs_stops_on_first_reject() { /* ... */ }
#[test] fn test_find_mirror_pairs_budget_kmax() { /* ... */ }

// ─── Sous-étape 5 : detect_ar (assemblage) ───────────────────────────
#[test] fn test_detect_ar_scenario_17_1() { /* ... */ }
#[test] fn test_detect_ar_scenario_17_1_p_pair_30() { /* ... */ }
#[test] fn test_detect_ar_scenario_17_1_p_maxpairs_1() { /* ... */ }
#[test] fn test_detect_ar_scenario_17_1_p_consol_0() { /* ... */ }
#[test] fn test_detect_ar_empty() { /* ... */ }
#[test] fn test_detect_ar_immobile() { /* ... */ }

// ─── Helper de test : chargement du GPX ──────────────────────────────
fn load_scenario_17_1() -> Vec<AuditPoint> {
    // Charge le fichier depuis docs/audit/reference/test_files/
    // Parse avec le crate gpx (déjà utilisé dans import_gpx.rs)
    // Retourne Vec<AuditPoint>
}
```

---

## 7. Points de vigilance (pièges identifiés)

| # | Piège | Mitigation |
|---|---|---|
| 1 | `atan2` en Rust : identique à JS (`atan2(dy, dx)`) | Ne pas inverser les arguments |
| 2 | Modulo négatif : `((x % 360) + 540) % 360 - 180` | Reproduire exactement la formule JS |
| 3 | `NaN` dans les comparaisons | Reproduire explicitement `is_finite()` |
| 4 | Groupage R2 : « cap entrant » = `head[g[0].i - 1]` (segment **précédant le premier candidat**) | Ne pas utiliser `head[g[0].i]` |
| 5 | Représentant médian : à égalité de distance, prendre celui avec `diff` max, puis le premier | Ordre stable |
| 6 | Filtre `prev_end` : `s <= prev_end` → finding ignoré | Réinitialiser `prev_end` à chaque itération |
| 7 | Marge `3` si trace fermée (`d(P0, P_last) < 100 m`), sinon `1` | Recalculer à chaque appel |
| 8 | Filet EPS : si `head[0]` reste indéfini après les deux parcours → trace immobile → `[]` | Ne jamais paniquer, retourner vide |

---

## 8. Critère de passage Phase 1 → Phase 2

**Tous les critères suivants doivent être satisfaits** :

- [ ] `cargo check` sans warning ni erreur
- [ ] `cargo test --lib gpx_audit::ar` : **18 tests verts**
- [ ] Scénario §17.1 : résultat **exact** documenté ci-dessus
- [ ] Les 3 variantes de sensibilité produisent les résultats documentés
- [ ] Aucune déviation par rapport à `detectAR` du HTML de référence

Toute déviation doit être **justifiée par écrit** (commentaire dans le
code ou note dans le commit) **avant** de passer à la Phase 2 (RP).

---

## 9. Compte-rendu attendu de Zcode

À la fin de la Phase 1, Zcode doit fournir :

1. **Résultat de `cargo test --lib gpx_audit::ar`** : 18 tests verts
2. **Contenu complet de `ar.rs`** (ou diff si intégration dans un
   fichier existant)
3. **Points de doute rencontrés** et comment ils ont été résolus
4. **Déviations justifiées** par rapport au HTML de référence (s'il y
   en a — idéalement aucune)
5. **Temps réel passé** sur chaque sous-étape

---

## 10. Notes sur le style de portage

- **Reproduire la structure du JS** : garder les noms de variables
  (`head`, `cands`, `groups`, `peaks`, `pairs`), les ordres de calcul,
  les conditions exactes.
- **Pas de `#![feature]`** ni de dépendances exotiques. Utiliser
  uniquement `std` et `serde`.
- **Commentaires** : en français, uniquement quand la logique du JS
  n'est pas évidente (ex. justifier le `+ 360 % 360`).
- **Pas de panic** : tous les accès tableau sont bornés par des
  conditions explicites (comme dans le JS).
- **Logging** : `println!` / `eprintln!` (pattern existant du projet),
  mais aucun log dans `detect_ar` (fonction pure).