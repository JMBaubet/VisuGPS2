//! Moteur de correction de la trace de travail (spec CORRECTIONS §3 à §9).
//!
//! Portage des fonctions JS de l'application de référence (`verifgpx-V3.0.html`
//! §6 « Corrections ») : réalignement des index par identifiants stables (§3),
//! chaîne post-édition (§4), suppression de plage (§5), routage (§6), garde
//! d'imbrication (§9.1), absorption des faux positifs (§9.3), marquage « faux
//! positif » (§7) et annulation par anomalie (§8).
//!
//! Principe : toute référence d'un finding à un point passe par son identifiant
//! stable ; la validité après édition est obtenue par **traduction**, jamais par
//! arithmétique d'index (CORRECTIONS §1.4).

use std::collections::HashMap;

use super::types::{
    AuditPoint, AuditState, CorrectionType, Finding, FindingKind, FindingStatus, LatLon,
    UndoDelete, UndoRecord, UndoRoute,
};

// ─── Réalignement ─────────────────────────────────────────────────────

/// Traduction des index par identifiants stables (CORRECTIONS §3.1).
///
/// Modifie `findings` en place : les index de chaque finding non corrigé sont
/// retraduits depuis ses identifiants stables, les textes affichables sont
/// régénérés (§2.4) et la liste est re-triée par position de trace.
///
/// Les findings `corrected` sont laissés **intacts** (invariant C6) : leur
/// emprise d'origine est intraduisible après édition — les points ont disparu.
/// La perte du réalignement est non silencieuse (`eprintln!`) et laisse le
/// finding inchangé : jamais d'index faux silencieux (§3.3).
pub fn sync_indexes(points: &[AuditPoint], findings: &mut Vec<Finding>) {
    let idx: HashMap<u32, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let m = points.len();

    for f in findings.iter_mut() {
        // Invariant C6 — un finding corrigé n'est jamais retraduit.
        if f.status == FindingStatus::Corrected {
            continue;
        }

        let label = f.label.clone();
        let s = f.zone_ids.first().and_then(|id| idx.get(id).copied());
        let e = f.zone_ids.last().and_then(|id| idx.get(id).copied());
        let pk = idx.get(&f.peak_id).copied();
        let (Some(s), Some(e), Some(pk)) = (s, e, pk) else {
            eprintln!("[GPX] emprise introuvable pour {}", label);
            continue;
        };
        if f.parts.is_empty() {
            eprintln!("[GPX] parties absentes pour {}", label);
            continue;
        }

        f.parts[0].s = s;
        f.parts[0].e = e;
        f.peak = pk;

        if f.kind == FindingKind::Rp {
            let cs = f.core_ids.first().and_then(|id| idx.get(id).copied());
            let ce = f.core_ids.last().and_then(|id| idx.get(id).copied());
            match (cs, ce) {
                (Some(cs), Some(ce)) if f.parts.len() >= 2 => {
                    f.parts[1].s = cs;
                    f.parts[1].e = ce;
                }
                (Some(_), Some(_)) => eprintln!("[GPX] partie de cœur absente pour {}", label),
                _ => eprintln!("[GPX] cœur introuvable pour {}", label),
            }
        } else if f.parts.len() >= 2 {
            // AR : le cœur est le voisinage immédiat du sommet.
            f.parts[1].s = pk.saturating_sub(1);
            f.parts[1].e = (pk + 1).min(m.saturating_sub(1));
        }

        for p in f.pairs.iter_mut() {
            match (idx.get(&p.aid).copied(), idx.get(&p.bid).copied()) {
                (Some(a), Some(b)) => {
                    p.a = a;
                    p.b = b;
                }
                // Le JS écrirait `undefined` : on conserve l'index précédent et
                // on signale — aucune indexation fausse ne peut être propagée.
                _ => eprintln!("[GPX] paire d'index introuvable pour {}", label),
            }
        }
        f.pair_idx = f.pairs.iter().flat_map(|p| [p.a, p.b]).collect();

        f.ctx.up = f.ctx_ids.up.and_then(|id| idx.get(&id).copied());
        f.ctx.dn = f.ctx_ids.dn.and_then(|id| idx.get(&id).copied());

        rebuild_texts(f);
    }

    sort_findings_by_position(findings);
}

/// Chaîne post-édition (CORRECTIONS §4) : réalignement puis re-tri.
///
/// Le JS enchaîne aussi `buildGeometry` — qui rétablit l'invariant C4
/// (`geo.ids[i] == working[i].id`) — ainsi que les re-rendus et les verrous :
/// la reconstruction de la géométrie est du ressort de la couche commande, les
/// re-rendus du frontend. Au niveau des données, la chaîne se réduit à §3.
pub fn after_trace_edit(points: &[AuditPoint], findings: &mut Vec<Finding>) {
    sync_indexes(points, findings);
}

// ─── Garde d'imbrication ──────────────────────────────────────────────

/// Garde d'imbrication (CORRECTIONS §9.1).
///
/// Retourne l'identifiant du **premier** finding `pending` (autre que
/// `current_id`) dont l'emprise intersecte la plage réellement affectée
/// `[zs, ze]`, sinon `None`. Un finding `fp` ne bloque jamais (§9.3), pas plus
/// qu'un finding `corrected` (déjà traité).
pub fn nesting_guard(
    findings: &[Finding],
    current_id: &str,
    zs: usize,
    ze: usize,
) -> Option<String> {
    if zs > ze {
        return None;
    }
    for g in findings {
        if g.id == current_id || g.status != FindingStatus::Pending {
            continue;
        }
        if let Some(p) = g.parts.first() {
            if p.e >= zs && p.s <= ze {
                return Some(g.id.clone());
            }
        }
    }
    None
}

/// Absorption des faux positifs (CORRECTIONS §9.3).
///
/// Retire de `findings` les findings `fp` (autres que `current_id`) dont
/// l'emprise intersecte `[zs, ze]` et retourne la liste des absorbés — à
/// stocker dans `undo.absorbed_fp` pour être réinjectés avec leur statut à
/// l'annulation de la correction (invariant C9).
pub fn absorb_fp_findings(
    findings: &mut Vec<Finding>,
    current_id: &str,
    zs: usize,
    ze: usize,
) -> Vec<Finding> {
    if zs > ze {
        return Vec::new();
    }

    let mut absorbed: Vec<Finding> = Vec::new();
    let mut keep: Vec<Finding> = Vec::new();

    for g in findings.drain(..) {
        let inter = g
            .parts
            .first()
            .map_or(false, |p| p.e >= zs && p.s <= ze);
        if g.id != current_id && g.status == FindingStatus::Fp && inter {
            absorbed.push(g);
        } else {
            keep.push(g);
        }
    }

    if !absorbed.is_empty() {
        println!(
            "[GPX] {} faux positif(s) absorbé(s) par la correction.",
            absorbed.len()
        );
    }
    *findings = keep;

    absorbed
}

// ─── Suppression ──────────────────────────────────────────────────────

/// Suppression de la plage `[ds..=de]` (CORRECTIONS §5).
///
/// Retire les points de la plage, pose l'instantané d'annulation **avant**
/// toute modification (invariant C1), marque le finding `corrected`, absorbe les
/// faux positifs recouverts (§9.3) puis réaligne les index (§3).
///
/// L'ordre des gardes est strict (CORRECTIONS §5.2) : imbrication (§9.1) →
/// non-dégénérescence (C3) → absorption (§9.3). Tout refus laisse la trace et
/// les findings intacts.
pub fn apply_delete(
    mut points: Vec<AuditPoint>,
    mut findings: Vec<Finding>,
    finding_id: &str,
    ds: usize,
    de: usize,
    next_point_id: u32,
) -> Result<AuditState, String> {
    // `next_point_id` : la suppression n'alloue aucun id (invariant C5) ; le
    // paramètre est présent pour l'homogénéité du contrat avec `apply_route`.
    let _ = next_point_id;

    // Garde défensive : plage inversée. Le cas RP « fin = début + 1 » est refusé
    // en amont (§5.2 garde 1, scénario D9) ; ce test évite tout débordement
    // d'entier sur `de − ds + 1`.
    if ds > de {
        return Err(
            "Aucun point à supprimer entre les curseurs — écartez-les d'abord.".to_string(),
        );
    }

    // Le JS refuse silencieusement toute correction d'un finding absent ou déjà
    // traité (`if (!f || f.status !== 'pending') return;`). Invariant C12 : un
    // `corrected` n'est jamais re-corrigé — le refus est explicite ici car une
    // seconde correction écraserait l'instantané d'annulation de la première.
    // Ce contrôle précède toute modification : un `Err` garantit un état intact.
    match findings.iter().find(|f| f.id == finding_id) {
        None => return Err(format!("Anomalie inconnue : {}.", finding_id)),
        Some(f) if f.status != FindingStatus::Pending => {
            return Err(format!("Anomalie « {} » déjà traitée.", f.label))
        }
        Some(_) => {}
    }

    // Garde 2 — imbrication : aucun point d'une anomalie `pending` tierce n'est
    // détruit (invariant C2).
    if let Some(blocking) = nesting_guard(&findings, finding_id, ds, de) {
        return Err(format!(
            "Correction refusée : traitez d'abord « {} », imbriquée dans cette zone (ou marquez-la faux positif).",
            blocking
        ));
    }

    // Copies défensives des points supprimés (invariant C7) — `get` fait aussi
    // office de contrôle de bornes (invariant C11) : jamais de panic, même sur
    // une plage hors trace.
    let orig_pts: Vec<AuditPoint> = points
        .get(ds..=de)
        .ok_or_else(|| "Plage de suppression hors de la trace.".to_string())?
        .to_vec();

    // Garde 3 — non-dégénérescence : la trace de travail garde ≥ 2 points (C3).
    if points.len() - orig_pts.len() < 2 {
        return Err("Suppression refusée : la trace deviendrait dégénérée.".to_string());
    }

    // §9.3 — le choix de l'absorption **avant** le retrait est un point d'ordre :
    // elle lit les index courants, qui ne sont plus interprétables après édition.
    let absorbed = absorb_fp_findings(&mut findings, finding_id, ds, de);

    // Ancres de réinsertion (§5.4) : points conservés adjacents à la plage.
    let anchor_left_id = if ds > 0 { Some(points[ds - 1].id) } else { None };
    let anchor_right_id = if de + 1 < points.len() {
        Some(points[de + 1].id)
    } else {
        None
    };

    // Invariant C1 — l'instantané est posé avant toute modification de `points`.
    let removed = orig_pts.len();
    let undo = UndoDelete {
        orig_pts,
        anchor_left_id,
        anchor_right_id,
        first_no: ds + 1,
        absorbed_fp: absorbed,
    };

    // §5.3 — retrait de la plage.
    points.drain(ds..=de);

    {
        let f = find_finding_mut(&mut findings, finding_id)?;
        f.status = FindingStatus::Corrected;
        f.correction = Some(CorrectionType::Delete);
        f.undo = Some(UndoRecord::Delete(undo));
    }

    // §4 — réalignement des index des findings restants + re-tri.
    sync_indexes(&points, &mut findings);

    println!(
        "[GPX] suppression de {} point(s) — trace : {} points",
        removed,
        points.len()
    );

    Ok(AuditState { points, findings })
}

// ─── Routage ──────────────────────────────────────────────────────────

/// Routage OpenRouteService (CORRECTIONS §6).
///
/// Remplace l'intérieur `[start+1..end−1]` par les points routés issus de
/// `coords` (tracé ORS reçu, sans ids) : les **ancres** `points[start]` et
/// `points[end]` sont conservées telles quelles et ne sont pas dupliquées. Le
/// `route_pts` de l'instantané est le **tracé de rendu**, préfixé/suffixé par
/// les ancres exactes (§2.3) — il ne vit pas dans la trace de travail.
///
/// `profile` : `"driving-car"` ou `"cycling-road"` (§6.1). Les points insérés
/// reçoivent des ids neufs alloués depuis `next_point_id`, strictement
/// croissants et jamais recyclés (invariant C5).
pub fn apply_route(
    mut points: Vec<AuditPoint>,
    mut findings: Vec<Finding>,
    finding_id: &str,
    start: usize,
    end: usize,
    coords: Vec<LatLon>,
    profile: &str,
    next_point_id: u32,
) -> Result<AuditState, String> {
    // §6.1 — profil déterminé par l'IHM ; deux valeurs admises.
    if profile != "driving-car" && profile != "cycling-road" {
        return Err(format!("Profil de routage inconnu : {}.", profile));
    }

    // §6.3 — le tracé ORS doit porter au moins ses deux extrémités.
    if coords.len() < 2 {
        return Err("Tracé ORS invalide (moins de 2 points).".to_string());
    }

    // §6.1 — contrainte de bornes des ancres : `end ≥ start + 1` (garantit aussi
    // que `end − 1` et `start + 1` sont calculables sans débordement).
    if end < start + 1 {
        return Err("Ancres de routage invalides.".to_string());
    }

    // Le JS refuse silencieusement toute correction d'un finding absent ou déjà
    // traité (`if (!f || f.status !== 'pending') return;`) — invariant C12.
    match findings.iter().find(|f| f.id == finding_id) {
        None => return Err(format!("Anomalie inconnue : {}.", finding_id)),
        Some(f) if f.status != FindingStatus::Pending => {
            return Err(format!("Anomalie « {} » déjà traitée.", f.label))
        }
        Some(_) => {}
    }

    // Garde — imbrication : la zone **remplacée** est l'intérieur ; les ancres
    // sont conservées, la garde ne porte donc que sur `[start+1..end−1]` (§6.2).
    // Ancres adjacentes (`start+1 > end−1`) : plage vide, la garde laisse passer.
    if let Some(blocking) = nesting_guard(&findings, finding_id, start + 1, end - 1) {
        return Err(format!(
            "Correction refusée : traitez d'abord « {} », imbriquée dans cette zone (ou marquez-la faux positif).",
            blocking
        ));
    }

    // Ancres exactes (§2.3) : bornes du tracé de rendu et support de la
    // réinsertion à l'annulation. Accès contrôlé — jamais de panic.
    let start_ll = points
        .get(start)
        .map(|p| LatLon {
            lat: p.lat,
            lon: p.lon,
        })
        .ok_or_else(|| "Plage de routage hors de la trace.".to_string())?;
    let end_ll = points
        .get(end)
        .map(|p| LatLon {
            lat: p.lat,
            lon: p.lon,
        })
        .ok_or_else(|| "Plage de routage hors de la trace.".to_string())?;

    // Copies défensives des points remplacés (invariant C7). Ancres adjacentes :
    // `get(start+1..end)` renvoie une plage vide — `inner` est vide, l'annulation
    // reste possible (§6.4).
    let inner: Vec<AuditPoint> = points
        .get(start + 1..end)
        .ok_or_else(|| "Plage de routage hors de la trace.".to_string())?
        .to_vec();

    // §9.3 — absorption des faux positifs recouverts, avant toute modification.
    let absorbed = absorb_fp_findings(&mut findings, finding_id, start + 1, end - 1);

    // Points insérés : coordonnées ORS **amputées de leurs deux extrémités** (les
    // ancres ne sont pas dupliquées). `LatLon` ne porte pas d'élévation (contrat
    // 01_TYPES_RS) : l'élévation ORS n'est pas propagée dans cette version.
    let mids: Vec<AuditPoint> = coords
        .get(1..coords.len() - 1)
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(k, c)| AuditPoint {
            id: next_point_id.saturating_add(k as u32),
            lat: c.lat,
            lon: c.lon,
            ele: None,
        })
        .collect();

    // Tracé de rendu borné aux ancres exactes (§2.3) : |route_pts| = |coords|.
    let mut route_pts: Vec<LatLon> = Vec::with_capacity(mids.len() + 2);
    route_pts.push(start_ll.clone());
    route_pts.extend(mids.iter().map(|p| LatLon {
        lat: p.lat,
        lon: p.lon,
    }));
    route_pts.push(end_ll.clone());

    // Invariant C1 — l'instantané est posé avant toute modification de `points`.
    let undo = UndoRoute {
        orig_pts: inner,
        inserted_ids: mids.iter().map(|p| p.id).collect(),
        route_pts,
        start_pt: start_ll,
        end_pt: end_ll,
        first_no: start + 2,
        absorbed_fp: absorbed,
    };

    // §6.3 — `working ← working[0..=start] ∪ mids ∪ working[end..]`. L'équivalent
    // JS est `slice(0, start+1).concat(mids, slice(end))`.
    let tail = points.split_off(end);
    points.truncate(start + 1);
    points.extend(mids);
    points.extend(tail);

    {
        let f = find_finding_mut(&mut findings, finding_id)?;
        f.status = FindingStatus::Corrected;
        f.correction = Some(if profile == "driving-car" {
            CorrectionType::RouteCar
        } else {
            CorrectionType::RouteBike
        });
        f.undo = Some(UndoRecord::Route(undo));
    }

    // §4 — réalignement des index des findings restants + re-tri.
    sync_indexes(&points, &mut findings);

    println!(
        "[GPX] routage {} — trace : {} points",
        if profile == "driving-car" {
            "voiture"
        } else {
            "vélo de route"
        },
        points.len()
    );

    Ok(AuditState { points, findings })
}

// ─── Faux positif ─────────────────────────────────────────────────────

/// Marquage d'un finding en faux positif (CORRECTIONS §7).
///
/// La trace est **intacte** : seule la décision de l'utilisateur est
/// enregistrée. Un `fp` ne bloque plus les corrections des autres anomalies
/// (§9.3) et reste annulable par [`unmark_fp`].
pub fn mark_fp(mut findings: Vec<Finding>, finding_id: &str) -> Result<Vec<Finding>, String> {
    let f = find_finding_mut(&mut findings, finding_id)?;
    if f.status != FindingStatus::Pending {
        return Err("Finding déjà traité.".to_string());
    }
    f.status = FindingStatus::Fp;
    f.correction = None;
    Ok(findings)
}

/// Retrait du marqueur faux positif (CORRECTIONS §7) : retour à `pending`.
pub fn unmark_fp(mut findings: Vec<Finding>, finding_id: &str) -> Result<Vec<Finding>, String> {
    let f = find_finding_mut(&mut findings, finding_id)?;
    if f.status != FindingStatus::Fp {
        return Err("Finding non marqué faux positif.".to_string());
    }
    f.status = FindingStatus::Pending;
    f.correction = None;
    Ok(findings)
}

// ─── Annulation ───────────────────────────────────────────────────────

/// Annulation de la correction d'un finding (CORRECTIONS §8).
///
/// Restitue **exactement** le multi-ensemble de points d'avant correction
/// (invariant C8 : mêmes ids, mêmes coordonnées, même ordre), remet le finding
/// `pending` et consomme l'instantané (`undo = None`, invariant C12). Les faux
/// positifs absorbés par la correction sont réinjectés avec leur statut `fp`
/// (§8.3, invariant C9).
pub fn undo_correction(
    mut points: Vec<AuditPoint>,
    mut findings: Vec<Finding>,
    finding_id: &str,
) -> Result<AuditState, String> {
    // 1 à 3 — le finding doit être corrigé et porter un instantané.
    let undo = {
        let f = find_finding(&findings, finding_id)?;
        if f.status != FindingStatus::Corrected {
            return Err("Aucune correction à annuler.".to_string());
        }
        f.undo.clone().ok_or_else(|| "Pas d'undo disponible.".to_string())?
    };

    // 4 — restauration de la trace, selon le type de correction.
    match &undo {
        UndoRecord::Route(u) => undo_route(&mut points, &findings, finding_id, u)?,
        UndoRecord::Delete(u) => undo_delete(&mut points, u)?,
    }

    // 5 — le finding redevient traitable ; l'instantané est consommé (C12).
    {
        let f = find_finding_mut(&mut findings, finding_id)?;
        f.status = FindingStatus::Pending;
        f.correction = None;
        f.undo = None;
    }

    // 6 — §8.3 : les faux positifs absorbés retrouvent leur statut `fp` et leur
    // place dans la liste (C9). Leurs ids repartent avec les points restaurés
    // (les `orig_pts` portent les ids d'origine) : `sync_indexes` les retrouve.
    let absorbed_fp = match undo {
        UndoRecord::Delete(u) => u.absorbed_fp,
        UndoRecord::Route(u) => u.absorbed_fp,
    };
    if !absorbed_fp.is_empty() {
        println!(
            "[GPX] {} faux positif(s) restauré(s) par l'annulation",
            absorbed_fp.len()
        );
        for mut g in absorbed_fp {
            g.status = FindingStatus::Fp;
            g.correction = None;
            findings.push(g);
        }
    }

    // §4 — réalignement des index + re-tri.
    sync_indexes(&points, &mut findings);

    println!(
        "[GPX] correction annulée — trace : {} points",
        points.len()
    );

    Ok(AuditState { points, findings })
}

// ─── Sous-fonctions privées ──────────────────────────────────────────

/// Annulation d'un routage (§8.1) — trois garde-fous successifs.
///
/// Aucune modification n'est appliquée si l'un des garde-fous refuse.
fn undo_route(
    points: &mut Vec<AuditPoint>,
    findings: &[Finding],
    current_id: &str,
    undo: &UndoRoute,
) -> Result<(), String> {
    // Garde-fou 1 — la zone routée n'est pas devenue l'origine (point gris)
    // d'une correction ultérieure.
    let used_by_other = findings.iter().any(|g| {
        g.id != current_id
            && g.undo
                .as_ref()
                .map_or(false, |u| {
                    undo_orig_pts(u)
                        .iter()
                        .any(|p| undo.inserted_ids.contains(&p.id))
                })
    });
    if used_by_other {
        return Err(
            "Annulation impossible : la zone routée a été réutilisée par une correction ultérieure."
                .to_string(),
        );
    }

    // Garde-fou 2 — les points insérés sont retrouvés dans la trace courante.
    // `inserted_ids` vide (routage à `mids` vide) retombe ici, comme le JS.
    let first_id = undo.inserted_ids.first().copied().ok_or_else(|| {
        "Annulation impossible : points de routage introuvables.".to_string()
    })?;
    let pos = points
        .iter()
        .position(|p| p.id == first_id)
        .ok_or_else(|| "Annulation impossible : points de routage introuvables.".to_string())?;

    // Garde-fou 3 — la séquence insérée est intacte, contiguë et dans l'ordre.
    for (k, id) in undo.inserted_ids.iter().enumerate() {
        match points.get(pos + k) {
            Some(p) if p.id == *id => {}
            _ => {
                return Err(
                    "Annulation impossible : la zone a été modifiée par une correction ultérieure."
                        .to_string(),
                )
            }
        }
    }

    // Restauration : copies défensives des points remplacés (invariant C7).
    let restored = undo.orig_pts.clone();
    let tail = points.split_off(pos + undo.inserted_ids.len());
    points.truncate(pos);
    points.extend(restored);
    points.extend(tail);

    Ok(())
}

/// Annulation d'une suppression (§8.2) — réinsertion par ancre.
///
/// L'ancre **droite** est préférée : elle couvre tous les cas sauf une plage
/// touchant la fin de trace, où l'ancre gauche prend le relais (insertion en
/// queue). L'échec n'intervient que si **les deux** ancres ont disparu.
fn undo_delete(points: &mut Vec<AuditPoint>, undo: &UndoDelete) -> Result<(), String> {
    let pos = match (undo.anchor_right_id, undo.anchor_left_id) {
        (Some(right), _) => points.iter().position(|p| p.id == right),
        (None, Some(left)) => points.iter().position(|p| p.id == left).map(|i| i + 1),
        (None, None) => None,
    }
    .ok_or_else(|| {
        "Annulation impossible : points d'ancrage disparus (corrections ultérieures).".to_string()
    })?;

    // Copies défensives des points supprimés (invariant C7) : la réinsertion
    // restitue les objets d'origine, ids compris.
    let restored = undo.orig_pts.clone();
    let tail = points.split_off(pos);
    points.extend(restored);
    points.extend(tail);

    Ok(())
}

/// Points remplacés par une correction, quel qu'en soit le type (§2.3).
fn undo_orig_pts(undo: &UndoRecord) -> &[AuditPoint] {
    match undo {
        UndoRecord::Delete(u) => &u.orig_pts,
        UndoRecord::Route(u) => &u.orig_pts,
    }
}

/// Recherche d'un finding par id (immuable).
fn find_finding<'a>(findings: &'a [Finding], id: &str) -> Result<&'a Finding, String> {
    findings
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| format!("Anomalie inconnue : {}.", id))
}

/// Reconstruction des textes d'un finding après réalignement (CORRECTIONS §2.4).
///
/// Seuls les textes portant des numéros sont régénérés ; les mesures
/// géométriques (`d`, `ecart`, `total_angle`, `d1/d2`) sont invariantes et ne
/// sont jamais recalculées. Comme le JS, le texte de cœur d'un AR — qui ne
/// contient aucun numéro — n'est pas retouché.
fn rebuild_texts(f: &mut Finding) {
    let k = f.pairs.len();
    let pair_txt = pair_text(f);

    match f.kind {
        FindingKind::Rp => {
            let total = f.total_angle.unwrap_or(0);
            let tt = f.turn_text.clone().unwrap_or_default();
            f.summary = format!(
                "jonction pt {} · {}° ({}) · {} paire{}",
                f.peak + 1,
                total,
                tt,
                k,
                if k > 1 { "s" } else { "" }
            );
            if f.parts.is_empty() {
                return;
            }
            f.parts[0].text = if k > 0 {
                format!("superpositions : {}", pair_txt)
            } else {
                "refermeture par croisement de trace".to_string()
            };
            if f.parts.len() >= 2 {
                f.parts[1].text = format!("cœur : angle cumulé {}° ({})", total, tt);
            }
        }
        FindingKind::Ar => {
            let ecart = f.ecart.unwrap_or(0.0);
            f.summary = format!(
                "sommet pt {} · {} paire{} · écart {:.0}°",
                f.peak + 1,
                k,
                if k > 1 { "s" } else { "" },
                ecart
            );
            if f.parts.is_empty() {
                return;
            }
            f.parts[0].text = if k > 0 {
                format!("paires miroirs : {}", pair_txt)
            } else {
                "retournement isolé, aucune paire miroir".to_string()
            };
        }
    }
}

/// Formate la liste des paires (gabarit du JS `rebuildTexts`) : distance à une
/// décimale en deçà de 10 m, arrondie au-delà.
fn pair_text(f: &Finding) -> String {
    f.pairs
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
        .join(" · ")
}

/// Recherche d'un finding par id (mutable).
fn find_finding_mut<'a>(
    findings: &'a mut Vec<Finding>,
    id: &str,
) -> Result<&'a mut Finding, String> {
    findings
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| format!("Anomalie inconnue : {}.", id))
}

/// Tri des findings par position croissante (`parts[0].s`).
///
/// Le tri est **stable**, comme `Array.prototype.sort` : à positions égales,
/// l'ordre de publication des détecteurs est conservé.
fn sort_findings_by_position(findings: &mut Vec<Finding>) {
    findings.sort_by(|u, v| {
        let su = u.parts.first().map_or(0, |p| p.s);
        let sv = v.parts.first().map_or(0, |p| p.s);
        su.cmp(&sv)
    });
}
