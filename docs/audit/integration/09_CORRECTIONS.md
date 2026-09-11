# Livrable 9 — Plan de portage Phase 3 : corrections

**Modules cibles** :
- `src-tauri/src/gpx_audit/corrections.rs` (moteur de correction)
- `src-tauri/src/gpx_audit/migration.rs` (D1 + D2c)
- `src-tauri/src/gpx_audit/export.rs` (réécriture GPX)

**Référence JS** : `docs/audit/reference/verifgpx-V3.0.html`
(sections 3, 6, 7, 8 — fonctions `syncIndexes`, `afterTraceEdit`,
`nestingGuard`, `absorbFpFindings`, `doDeleteRange`, `applyRoute`,
`undoCorrection`)

**Spécification normative** : `docs/audit/spec/CORRECTIONS.md`
(§3 à §9)

**Durée estimée** : 3 à 4 jours

---

## 1. Objectif

Porter en Rust le **moteur de correction au niveau des données** :
- Réalignement des index après édition (`sync_indexes`)
- Suppression de plage (`apply_delete`)
- Routage ORS (`apply_route`)
- Faux positif (`mark_fp` / `unmark_fp`)
- Annulation (`undo_correction`)
- Garde d'imbrication (`nesting_guard`)
- Absorption des faux positifs (`absorb_fp_findings`)
- Migration (D1 + D2c)
- Réécriture GPX (`rewrite_gpx`)

**Principe de portage** : reproduction **littérale** de la logique JS.
Invariants C1 à C12 (CORRECTIONS §10) doivent être vérifiables par
inspection du code.

---

## 2. Signatures Rust — `corrections.rs`

```rust
// src-tauri/src/gpx_audit/corrections.rs

use super::types::{
    AuditPoint, AuditState, CorrectionType, Finding, FindingStatus,
    LatLon, UndoDelete, UndoRecord, UndoRoute,
};

// ─── Réalignement ─────────────────────────────────────────────────────

/// Traduction des index par identifiants stables (CORRECTIONS §3.1).
/// Modifie `findings` en place. Ne touche pas les findings corrigés.
pub fn sync_indexes(
    points: &[AuditPoint],
    findings: &mut Vec<Finding>,
);

/// Chaîne post-édition (CORRECTIONS §4).
/// Applique sync_indexes et re-trie les findings par position.
pub fn after_trace_edit(
    points: &[AuditPoint],
    findings: &mut Vec<Finding>,
);

// ─── Garde d'imbrication ──────────────────────────────────────────────

/// Retourne Some(finding_id) si une anomalie `pending` (≠ current)
/// intersecte la plage `[zs, ze]`. Sinon `None`.
pub fn nesting_guard(
    findings: &[Finding],
    current_id: &str,
    zs: usize,
    ze: usize,
) -> Option<String>;

/// Absorption des faux positifs (CORRECTIONS §9.3).
/// Retire de `findings` ceux dont la zone intersecte `[zs, ze]`.
/// Retourne la liste des findings absorbés (à stocker dans undo).
pub fn absorb_fp_findings(
    findings: &mut Vec<Finding>,
    current_id: &str,
    zs: usize,
    ze: usize,
) -> Vec<Finding>;

// ─── Suppression ──────────────────────────────────────────────────────

/// Suppression de la plage `[ds..=de]` (CORRECTIONS §5).
pub fn apply_delete(
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: &str,
    ds: usize,
    de: usize,
    next_point_id: u32,
) -> Result<AuditState, String>;

// ─── Routage ──────────────────────────────────────────────────────────

/// Remplacement de l'intérieur `[start+1..end-1]` par les points ORS
/// (CORRECTIONS §6). Les ancres `start` et `end` sont conservées.
pub fn apply_route(
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: &str,
    start: usize,
    end: usize,
    coords: Vec<LatLon>,
    profile: &str,
    next_point_id: u32,
) -> Result<AuditState, String>;

// ─── Faux positif ─────────────────────────────────────────────────────

/// Marque un finding comme faux positif (CORRECTIONS §7).
pub fn mark_fp(
    findings: Vec<Finding>,
    finding_id: &str,
) -> Result<Vec<Finding>, String>;

/// Retire le marqueur faux positif.
pub fn unmark_fp(
    findings: Vec<Finding>,
    finding_id: &str,
) -> Result<Vec<Finding>, String>;

// ─── Annulation ───────────────────────────────────────────────────────

/// Annule la correction d'un finding (CORRECTIONS §8).
pub fn undo_correction(
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: &str,
) -> Result<AuditState, String>;

// ─── Sous-fonctions privées ──────────────────────────────────────────

/// Reconstruction des textes d'un finding après réalignement
/// (CORRECTIONS §2.4).
fn rebuild_texts(f: &mut Finding);

/// Recherche de l'index d'un id dans points (None si absent).
fn find_index_by_id(points: &[AuditPoint], id: u32) -> Option<usize>;

/// Tri des findings par position croissante (parts[0].s).
fn sort_findings_by_position(findings: &mut Vec<Finding>);

/// Recherche d'un finding par id (mutable).
fn find_finding_mut<'a>(
    findings: &'a mut Vec<Finding>,
    id: &str,
) -> Result<&'a mut Finding, String>;

/// Recherche d'un finding par id (immuable).
fn find_finding<'a>(
    findings: &'a [Finding],
    id: &str,
) -> Result<&'a Finding, String>;
```

---

## 3. Signatures Rust — `migration.rs`

```rust
// src-tauri/src/gpx_audit/migration.rs

use std::path::Path;

/// D1 : détection de registre obsolète (format pré-audit).
/// Retourne true si `raw` contient `"cleaning_status"`.
pub fn is_obsolete_registry(raw: &str) -> bool;

/// D2c : nettoyage des fichiers `cleaning.*.json` orphelins.
/// Silencieux (D3b) — pas de log, pas de retour d'erreur.
pub fn cleanup_obsolete_cleaning_files(mode_dir: &Path);
```

---

## 4. Signatures Rust — `export.rs`

```rust
// src-tauri/src/gpx_audit/export.rs

use std::path::Path;
use super::types::AuditPoint;

/// Réécrit le GPX après audit (CORRECTIONS §5.4 + IHM §20).
///
/// - Pose le backup `.orig` si absent (jamais écrasé)
/// - Réécrit le GPX avec les points corrigés
/// - Préserve l'entête source (attributs `<gpx>`, `<metadata>`,
///   `<name>` de la trace)
/// - Ajoute un bloc d'audit horodaté dans `<metadata><extensions>`
pub fn rewrite_gpx(
    original_path: &Path,
    points: &[AuditPoint],
    source_meta_xml: Option<&str>,
    source_gpx_attrs: Option<&str>,
    source_trk_name: Option<&str>,
    app_name: &str,
    findings_summary: FindingsSummary,
) -> Result<(), String>;

/// Compteurs pour le bloc d'audit.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindingsSummary {
    pub routes: usize,
    pub deletions: usize,
    pub false_positives: usize,
}

/// Échappement XML minimal.
fn xml_escape(s: &str) -> String;

/// Horodatage ISO 8601 UTC.
fn iso_now() -> String;
```

---

## 5. Ordre d'implémentation (5 sous-étapes)

### Sous-étape 3.1 — Réalignement et garde d'imbrication

**Étapes couvertes** : 1, 2
**Fonctions** : `sync_indexes`, `rebuild_texts`, `after_trace_edit`,
`nesting_guard`, `absorb_fp_findings`
**Référence** : CORRECTIONS §3, §4, §9
**Tests** : 9

**`sync_indexes`** (CORRECTIONS §3.1) :

```
1. Construire idx = HashMap<u32, usize> (id -> index courant)
2. Pour chaque finding f :
   - Skip si f.status == Corrected (invariant C6)
   - s = idx[f.zone_ids[0]], e = idx[f.zone_ids.last], pk = idx[f.peak_id]
   - Si l'un manque : eprintln! + continuer (dégradation non silencieuse)
   - Mettre à jour f.parts[0].s, f.parts[0].e, f.peak
   - Si f.kind == Ar :
       f.parts[1].s = max(0, pk - 1)
       f.parts[1].e = min(m - 1, pk + 1)
     Sinon (Rp) :
       cs = idx[f.core_ids[0]], ce = idx[f.core_ids.last]
       Si cs et ce présents : f.parts[1] = [cs, ce]
   - Pour chaque paire p : p.a = idx[p.aid], p.b = idx[p.bid]
   - f.pair_idx = aplatissement
   - f.ctx.up = idx.get(f.ctx_ids.up).copied().flatten()
   - f.ctx.dn = idx.get(f.ctx_ids.dn).copied().flatten()
   - rebuild_texts(f)
3. sort_findings_by_position(findings)
```

**`nesting_guard`** (CORRECTIONS §9.1) :

```
Pour chaque g de findings :
    Si g.id == current_id : continuer
    Si g.status != Pending : continuer
    Si g.parts[0].e >= zs et g.parts[0].s <= ze :
        Retourner Some(g.id.clone())
Retourner None
```

**`absorb_fp_findings`** (CORRECTIONS §9.3) :

```
absorbed = []
keep = []
Pour chaque g de findings :
    Si g.id != current_id et g.status == Fp
       et g.parts[0] intersecte [zs, ze] :
        absorbed.push(g.clone())
    Sinon :
        keep.push(g)
*findings = keep
Retourner absorbed
```

---

### Sous-étape 3.2 — Suppression de plage

**Étape couverte** : 3
**Fonction** : `apply_delete`
**Référence** : CORRECTIONS §5
**Tests** : 8

**`apply_delete`** (CORRECTIONS §5.2 et §5.3) :

```
1. Gardes (dans l'ordre, tout échec = Err) :
   a. nesting_guard(findings, finding_id, ds, de) → Some → Err
   b. points.len() - (de - ds + 1) < 2 → Err

2. absorbed = absorb_fp_findings(&mut findings, finding_id, ds, de)

3. Récupérer les ancres :
   anchor_left_id  = if ds > 0 { Some(points[ds - 1].id) } else { None }
   anchor_right_id = if de < m - 1 { Some(points[de + 1].id) } else { None }

4. Construire undo :
   UndoDelete {
       orig_pts: points[ds..=de].iter().map(|p| p.clone()).collect(),
       anchor_left_id,
       anchor_right_id,
       first_no: ds + 1,
       absorbed_fp: absorbed,
   }

5. Retirer la plage :
   points.splice(ds..=de, std::iter::empty())

6. Marquer le finding :
   f.status = Corrected
   f.correction = Some(Delete)
   f.undo = Some(UndoRecord::Delete(undo))

7. sync_indexes(&points, &mut findings)

8. Retourner Ok(AuditState { points, findings })
```

**Points de vigilance** :
- `absorb_fp_findings` est appelé **avant** le retrait (les index des
  FP sont encore valides — point d'ordre CORRECTIONS §5.3).
- `orig_pts` : utiliser `clone()` pour copies défensives (invariant C7).
- L'ordre `nesting_guard → absorb → undo → retrait` est strict.

---

### Sous-étape 3.3 — Routage

**Étape couverte** : 4
**Fonction** : `apply_route`
**Référence** : CORRECTIONS §6
**Tests** : 7

**`apply_route`** (CORRECTIONS §6.3) :

```
1. Valider profile :
   si profile != "driving-car" && != "cycling-road" : Err

2. Valider coords :
   si coords.len() < 2 : Err("Tracé ORS invalide (moins de 2 points).")

3. Gardes :
   a. nesting_guard(findings, finding_id, start + 1, end - 1)
   b. start + 1 > end - 1 : cas "intérieur vide" — autorisé

4. absorbed = absorb_fp_findings(&mut findings, finding_id, start + 1, end - 1)

5. inner = if start + 1 < end { points[start+1..end].to_vec() } else { vec![] }

6. Construire mids (points insérés) :
   coords_internes = coords[1..coords.len()-1]  // retirer ancres
   mids = coords_internes.iter().enumerate().map(|(k, c)| AuditPoint {
       id: next_point_id + k as u32,
       lat: c.lat,
       lon: c.lon,
       ele: None,  // ORS fournit l'élévation si demandée (à propager)
   }).collect()

7. Construire routePts (tracé de rendu borné aux ancres) :
   route_pts = [
       LatLon { lat: points[start].lat, lon: points[start].lon },
       ...mids.iter().map(|p| LatLon { lat: p.lat, lon: p.lon }),
       LatLon { lat: points[end].lat, lon: points[end].lon },
   ]

8. Construire undo :
   UndoRoute {
       orig_pts: inner.iter().map(|p| p.clone()).collect(),
       inserted_ids: mids.iter().map(|p| p.id).collect(),
       route_pts,
       start_pt: LatLon { lat: points[start].lat, lon: points[start].lon },
       end_pt:   LatLon { lat: points[end].lat,   lon: points[end].lon },
       first_no: start + 2,
       absorbed_fp: absorbed,
   }

9. Insérer les mids :
   points.splice(start + 1..end, mids.clone())

10. Marquer le finding :
    f.status = Corrected
    f.correction = Some(if profile == "driving-car" { RouteCar } else { RouteBike })
    f.undo = Some(UndoRecord::Route(undo))

11. sync_indexes(&points, &mut findings)

12. Retourner Ok(AuditState { points, findings })
```

**Points de vigilance** :
- Les **mids** portent les nouveaux ids (à partir de `next_point_id`).
- Les **route_pts** sont préfixés/suffixés par les ancres **exactes**
  de `points` (CORRECTIONS §2.3).
- Le cas `start + 1 > end - 1` (ancres adjacentes) est valide :
  `inner` est vide, `orig_pts` est vide, mais l'annulation reste possible.
- L'élévation ORS n'est pas propagée dans cette version (extension
  future possible).

---

### Sous-étape 3.4 — Faux positif et annulation

**Étapes couvertes** : 5, 6
**Fonctions** : `mark_fp`, `unmark_fp`, `undo_correction`
**Référence** : CORRECTIONS §7, §8
**Tests** : 12

**`mark_fp`** :

```
1. f = find_finding_mut(&mut findings, finding_id)?
2. Si f.status != Pending : Err("Finding déjà traité.")
3. f.status = Fp
4. f.correction = None
5. Retourner Ok(findings)
```

**`unmark_fp`** :

```
1. f = find_finding_mut(&mut findings, finding_id)?
2. Si f.status != Fp : Err("Finding non marqué faux positif.")
3. f.status = Pending
4. f.correction = None
5. Retourner Ok(findings)
```

**`undo_correction`** (CORRECTIONS §8) :

```
1. f = find_finding(&findings, finding_id)?
2. Si f.status != Corrected : Err("Aucune correction à annuler.")
3. undo = f.undo.clone().ok_or("Pas d'undo disponible.")?

4. Selon undo.type :
   
   Cas Route :
   a. Vérifier garde 1 : usedByOther
      Pour chaque g != f avec g.undo défini :
          Si un id de undo.inserted_ids est dans g.undo.orig_pts :
              Err("Zone réutilisée par une correction ultérieure.")
   
   b. Vérifier garde 2 : position
      pos = points.iter().position(|p| p.id == undo.inserted_ids[0])
      Si None : Err("Points de routage introuvables.")
   
   c. Vérifier garde 3 : contiguïté ordonnée
      Pour k in 0..undo.inserted_ids.len() :
          Si points[pos + k].id != undo.inserted_ids[k] :
              Err("Zone modifiée par une correction ultérieure.")
   
   d. Restaurer :
      points.splice(pos..pos + undo.inserted_ids.len(), undo.orig_pts.clone())
   
   Cas Delete :
   a. pos = if let Some(anchor_right_id) = undo.anchor_right_id {
          find_index_by_id(&points, anchor_right_id)?
      } else if let Some(anchor_left_id) = undo.anchor_left_id {
          find_index_by_id(&points, anchor_left_id)? + 1
      } else {
          return Err("Ancres disparues.")
      }
   
   b. points.splice(pos..pos, undo.orig_pts.clone())

5. f.status = Pending
6. f.correction = None
7. f.undo = None

8. Réinjecter les absorbed_fp :
   pour chaque g de undo.absorbed_fp :
       g.status = Fp
       g.correction = None
       findings.push(g)

9. sync_indexes(&points, &mut findings)

10. Retourner Ok(AuditState { points, findings })
```

**Points de vigilance** :
- **Garde 3** (contiguïté ordonnée) : `pos + k` doit rester dans les
  bornes, sinon erreur.
- **Absorbed_fp réinjectés** : leurs ids viennent des `orig_pts`
  (restaurés avec leurs ids d'origine) — `sync_indexes` les retrouve.
- **Statut Fp restauré** (invariant C9).
- `f.undo = None` **après** avoir utilisé le contenu (sinon on perd la
  référence).

---

### Sous-étape 3.5 — Migration et export

**Étapes couvertes** : 7, 8, 9
**Fonctions** : `is_obsolete_registry`, `cleanup_obsolete_cleaning_files`,
`rewrite_gpx`
**Référence** : Livrable 5 §3 et §4, CORRECTIONS §5.4, IHM §20
**Tests** : 12

**`is_obsolete_registry`** :

```
Contient la sous-chaîne "\"cleaning_status\"" (avec guillemets)
```

**`cleanup_obsolete_cleaning_files`** :

```
Voir Livrable 5 §4.2 (code complet fourni)
```

**`rewrite_gpx`** (IHM §20.3) :

```
1. Si !original_path.exists() : Err
2. Construire orig_path = original_path.with_extension("gpx.orig")
3. Si !orig_path.exists() :
       fs::copy(original_path, &orig_path)?
   // sinon, ne pas écraser

4. Sérialiser le XML :
   - <?xml version="1.0" encoding="UTF-8"?>\n
   - <gpx {source_gpx_attrs ou fallback}>\n
   - [metadata source ou minimal + audit]\n
   - <trk><name>{source_trk_name}</name><trkseg>\n
   - pour chaque point :
       <trkpt lat="{:.7}" lon="{:.7}">
       [<ele>{:.2}</ele>]
       </trkpt>\n
   - </trkseg></trk>\n
   - </gpx>\n

5. Écrire dans un fichier .tmp puis rename (atomique)
```

**Bloc d'audit** (IHM §20.2) :

```xml
<extensions>
  <audit xmlns="http://VérificationGPX.example/gpx/audit/1">
    <modified>2026-01-15T14:32:07Z</modified>
    <tool>VérificationGPX</tool>
    <corrections total="5" routes="2" deletions="3" falsePositives="1"/>
  </audit>
</extensions>
```

**Points de vigilance** :
- **Backup `.orig`** : ne jamais écraser. C'est le seul filet de
  sécurité en cas de problème.
- **Écriture atomique** : tmp + rename. Si le rename échoue, le GPX
  original est intact.
- **Métadonnées source** : relire le GPX actuel au moment de la
  validation (elles ne sont pas stockées dans `TraceMetadata`).
- **7 décimales** pour lat/lon, **2 décimales** pour ele.
- **Échappement XML** : `&`, `<`, `>`, `"`.
- **Un seul `<trkseg>`** (pas de préservation des segments multiples —
  décision de simplification).

---

## 6. Jeux de tests complets (`tests/corrections_test.rs`)

```rust
// src-tauri/src/gpx_audit/tests/corrections_test.rs

use super::corrections::*;
use super::migration::*;
use crate::gpx_audit::types::*;

// ─── Sous-étape 3.1 : réalignement et imbrication ────────────────────
#[test] fn test_sync_indexes_after_delete_upstream() { /* ... */ }
#[test] fn test_sync_indexes_after_delete_downstream() { /* ... */ }
#[test] fn test_sync_indexes_skips_corrected() { /* ... */ }
#[test] fn test_sync_indexes_rebuild_texts() { /* ... */ }
#[test] fn test_rebuild_texts_ar() { /* ... */ }
#[test] fn test_rebuild_texts_rp() { /* ... */ }
#[test] fn test_nesting_guard_refuses() { /* ... */ }
#[test] fn test_nesting_guard_allows_fp() { /* ... */ }
#[test] fn test_absorb_fp_findings() { /* ... */ }

// ─── Sous-étape 3.2 : suppression ────────────────────────────────────
#[test] fn test_apply_delete_basic() { /* ... */ }
#[test] fn test_apply_delete_nesting_refused() { /* ... */ }
#[test] fn test_apply_delete_degenerate_refused() { /* ... */ }
#[test] fn test_apply_delete_single_point() { /* ... */ }
#[test] fn test_apply_delete_undo_complete() { /* ... */ }
#[test] fn test_apply_delete_anchor_left_null() { /* ... */ }
#[test] fn test_apply_delete_anchor_right_null() { /* ... */ }
#[test] fn test_apply_delete_absorbs_fp() { /* ... */ }

// ─── Sous-étape 3.3 : routage ────────────────────────────────────────
#[test] fn test_apply_route_basic() { /* ... */ }
#[test] fn test_apply_route_profile_invalid() { /* ... */ }
#[test] fn test_apply_route_coords_too_short() { /* ... */ }
#[test] fn test_apply_route_nesting_refused() { /* ... */ }
#[test] fn test_apply_route_empty_inner() { /* ... */ }
#[test] fn test_apply_route_undo_route_pts() { /* ... */ }
#[test] fn test_apply_route_new_ids_allocated() { /* ... */ }

// ─── Sous-étape 3.4 : FP et annulation ───────────────────────────────
#[test] fn test_mark_fp_basic() { /* ... */ }
#[test] fn test_mark_fp_rejects_corrected() { /* ... */ }
#[test] fn test_unmark_fp_basic() { /* ... */ }
#[test] fn test_undo_delete_anchor_right() { /* ... */ }
#[test] fn test_undo_delete_anchor_left_only() { /* ... */ }
#[test] fn test_undo_delete_anchors_lost() { /* ... */ }
#[test] fn test_undo_delete_restores_fp() { /* ... */ }
#[test] fn test_undo_route_basic() { /* ... */ }
#[test] fn test_undo_route_used_by_other() { /* ... */ }
#[test] fn test_undo_route_contiguity_broken() { /* ... */ }
#[test] fn test_undo_route_pos_not_found() { /* ... */ }
#[test] fn test_undo_route_restores_fp() { /* ... */ }

// ─── Sous-étape 3.5 : migration et export ────────────────────────────
#[test] fn test_is_obsolete_registry_true() { /* ... */ }
#[test] fn test_is_obsolete_registry_false() { /* ... */ }
#[test] fn test_cleanup_obsolete_cleaning_files() { /* ... */ }
#[test] fn test_cleanup_idempotent() { /* ... */ }
#[test] fn test_rewrite_gpx_basic() { /* ... */ }
#[test] fn test_rewrite_gpx_backup_created() { /* ... */ }
#[test] fn test_rewrite_gpx_backup_preserved() { /* ... */ }
#[test] fn test_rewrite_gpx_preserves_attrs() { /* ... */ }
#[test] fn test_rewrite_gpx_ele_optional() { /* ... */ }
#[test] fn test_rewrite_gpx_7_decimals() { /* ... */ }
#[test] fn test_rewrite_gpx_atomic_write() { /* ... */ }
#[test] fn test_rewrite_gpx_audit_block() { /* ... */ }
```

**Total : 48 tests** (au lieu des 12 scénarios D1..D12 CORRECTIONS §12
annoncés — révision à la hausse pour couvrir chaque sous-étape).

---

## 7. Scénarios D1 à D12 (CORRECTIONS §12)

Ces tests d'intégration regroupent plusieurs tests unitaires ci-dessus
pour valider des comportements bout-en-bout.

| # | Scénario | Couverture |
|---|---|---|
| D1 | Suppression **amont** d'une anomalie puis `sync_indexes` | `test_sync_indexes_after_delete_upstream`, `test_apply_delete_basic` |
| D2 | Suppression **aval** puis `sync_indexes` | `test_sync_indexes_after_delete_downstream` |
| D3 | Routage appliqué puis annulation | `test_apply_route_basic`, `test_undo_route_basic` |
| D4 | Suppression puis annulation | `test_apply_delete_basic`, `test_undo_delete_anchor_right` |
| D5 | AR imbriquée dans RP : correction AR puis RP, annulations quelconques | `test_apply_delete_nesting_refused`, `test_sync_indexes_skips_corrected` |
| D6 | FP absorbé par routage puis annulation | `test_apply_route_absorbs_fp` (à ajouter), `test_undo_route_restores_fp` |
| D7 | Routage mord un `pending` tiers | `test_apply_route_nesting_refused` |
| D8 | Routage, puis suppression chevauchante, puis annulation du routage | `test_undo_route_used_by_other`, `test_undo_route_contiguity_broken` |
| D9 | Suppression RP avec `fin = début + 1` | Validé côté commande (`audit_apply_delete`) — à tester en intégration |
| D10 | Suppression amenant à 1 point | `test_apply_delete_degenerate_refused` |
| D11 | Double tour : suppression réduite, re-détection propre | Test d'intégration bout-en-bout |
| D12 | Compteurs d'audit déterministes | `test_rewrite_gpx_audit_block` |

**Test supplémentaire à ajouter** : `test_apply_route_absorbs_fp`.

---

## 8. Points de vigilance (invariants CORRECTIONS §10)

| # | Invariant | Où il est appliqué |
|---|---|---|
| **C1** | Undo posé **avant** modification | `apply_delete`, `apply_route` : construire `undo` avant `points.splice` |
| **C2** | Plages ne détruisent jamais un point d'un finding non corrigé | `nesting_guard` appelé en tête des corrections |
| **C3** | `points.len() >= 2` après correction | `apply_delete` : garde explicite |
| **C4** | `geo.ids[i] == points[i].id` après édition | Le front recalcule le geo (buildGeometry) après chaque action |
| **C5** | Ids jamais recyclés | `next_point_id` passé par le front, jamais recalculé côté Rust |
| **C6** | `sync_indexes` laisse les `corrected` intacts | Skip explicite dans la boucle |
| **C7** | `orig_pts` sont des copies défensives | `clone()` sur chaque point |
| **C8** | Annulation restitue exactement le multi-ensemble | Vérifié par `test_undo_*_basic` |
| **C9** | FP absorbés restaurés avec leur statut | `undo_correction` réinjecte avec `status = Fp` |
| **C10** | Trace exportée = working | `audit_validate` réécrit `points` tel quel |
| **C11** | Plages bornées par l'IHM | Front + `nesting_guard` (double protection) |
| **C12** | Un `corrected` ne peut être re-corrigé ni annulé deux fois | `undo = None` après consommation, `status` vérifié |

---

## 9. Structure interne des fichiers

### 9.1 `corrections.rs` — Organisation

```
corrections.rs
├── sync_indexes
├── after_trace_edit
├── nesting_guard
├── absorb_fp_findings
├── apply_delete
├── apply_route
├── mark_fp
├── unmark_fp
├── undo_correction
│   ├── undo_route (privée)
│   └── undo_delete (privée)
└── helpers privés
    ├── rebuild_texts
    ├── find_index_by_id
    ├── sort_findings_by_position
    ├── find_finding
    └── find_finding_mut
```

### 9.2 `migration.rs` — Organisation

```
migration.rs
├── is_obsolete_registry
└── cleanup_obsolete_cleaning_files
```

### 9.3 `export.rs` — Organisation

```
export.rs
├── rewrite_gpx
├── build_gpx_xml (privée)
├── build_metadata_xml (privée)
├── build_audit_xml (privée)
├── xml_escape (privée)
└── iso_now (privée)
```

---

## 10. Critère de passage Phase 3 → Phase 4 (frontend)

**Tous les critères suivants doivent être satisfaits** :

- [ ] `cargo check` sans warning ni erreur
- [ ] `cargo test --lib gpx_audit::corrections` : **48 tests verts**
- [ ] `cargo test --lib gpx_audit::migration` : inclus
- [ ] `cargo test --lib gpx_audit::export` : inclus
- [ ] Les 12 scénarios D1..D12 passent
- [ ] Les invariants C1..C12 sont vérifiables par inspection
- [ ] Aucune déviation par rapport au HTML de référence

---

## 11. Compte-rendu attendu de Zcode

À la fin de la Phase 3, Zcode doit fournir :

1. **Résultat de `cargo test --lib gpx_audit::corrections`** : 48 tests verts
2. **Contenu complet de `corrections.rs`, `migration.rs`, `export.rs`**
3. **Points de doute rencontrés** et comment ils ont été résolus
4. **Table de correspondance** entre les invariants C1..C12 et leur
   lieu d'application (fichier + fonction + ligne)
5. **Temps réel passé** sur chaque sous-étape (3.1 à 3.5)

---

## 12. Notes sur le style de portage

- **Reproduire la structure du JS** : `syncIndexes`, `applyRoute`,
  `doDeleteRange`, `undoCorrection`.
- **Copies défensives partout** : `.clone()` sur les `AuditPoint`
  avant de les stocker dans `undo`.
- **`find_finding_mut`** : utiliser `iter_mut().find(...)` — pas
  d'index (le borrow checker n'aime pas).
- **`splice` vs `Vec::drain`** : `splice` est plus expressif, mais
  `drain` + `extend` est parfois plus clair. Choisir selon le contexte.
- **Commentaires** : en français, uniquement quand la logique du JS
  n'est pas évidente (justifier les invariants C1..C12).
- **Logging** : `println!` pour les corrections appliquées (pattern
  existant), `eprintln!` pour les erreurs de `sync_indexes` (dégradation
  non silencieuse).