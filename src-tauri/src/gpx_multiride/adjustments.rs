//! Ajustements manuels de la détection (spécification §F-14 à §F-16).
//!
//! Quatre gestes, sur des fonctions **pures** — l'écriture du fichier
//! appartient aux commandes :
//!
//! - **fusion** d'un segment avec le précédent : la décision explicite de
//!   l'utilisateur que deux tronçons n'en font qu'un (deux rues d'un même
//!   carrefour, portions séparées par une interruption de trace) ;
//! - **marquage faux positif** d'un segment : détecté à tort, exclu de l'export
//!   et des kilomètres répétés ;
//! - **annulation** d'un ajustement, par segment (voir [`undo_segment`]) ;
//! - **réinitialisation**, qui rejoue la détection — portée par la commande, la
//!   détection n'étant pas du ressort de ce module.
//!
//! **Un segment écarté ne fusionne pas**, et un segment fusionné ne s'écarte
//! pas : la fusion et le faux positif s'excluent, de sorte qu'un segment ne
//! porte jamais qu'un ajustement — donc une seule annulation à offrir. En
//! revanche les fusions **s'enchaînent** : un segment peut absorber son
//! précédent puis le suivant, et la chaîne s'annule de la plus récente à la
//! plus ancienne (cf. [`undo_segment`]).
//!
//! Aucun de ces gestes ne touche au **statut** de la trace : ils n'ont pas
//! d'incidence sur l'édition caméra, et un état validé le reste.

use std::cmp::Ordering;

use super::types::{MultirideArchive, MultiridePassage, MultirideSens};

/// Tolérance de la fusion manuelle (km), **indépendante** du réglage `fuse` de
/// la détection.
///
/// La fusion est une décision explicite de l'utilisateur : elle peut couvrir de
/// grands trous — interruption de trace, sens uniques d'un village, portion
/// manquante — là où la fusion automatique doit rester prudente.
pub const MERGE_MANUAL_TOL_KM: f64 = 1.0;

/// Rang de sens d'un emprunt, dans la sémantique de la spécification : `0` pour
/// la référence, `+1` pour un aller, `−1` pour un retour.
///
/// C'est ce rang — et non la direction réelle — que compare la fusion manuelle :
/// une référence ne se fusionne qu'avec une autre référence, un aller avec un
/// aller. La frontière entre deux **sens** reste ainsi intangible.
pub fn sens_rank(sens: MultirideSens) -> i8 {
    match sens {
        MultirideSens::Reference => 0,
        MultirideSens::Aller => 1,
        MultirideSens::Retour => -1,
    }
}

/// Emprunts d'un segment, dans l'ordre des emprunts.
fn segment_passages(passages: &[MultiridePassage], segment: usize) -> Vec<MultiridePassage> {
    let mut found: Vec<MultiridePassage> = passages
        .iter()
        .filter(|p| p.segment == segment)
        .cloned()
        .collect();
    found.sort_by_key(|p| p.passage);
    found
}

/// Marque ou démarque un segment en faux positif (§F-15).
///
/// Un segment est faux positif lorsque **tous** ses emprunts le sont : c'est le
/// geste de l'utilisateur, porté par chaque emprunt du segment, que le fichier
/// conserve afin qu'une réouverture retrouve le marquage.
///
/// Le marquage est refusé sur un segment **fusionné** : la fusion et le faux
/// positif s'excluent, un segment ne portant qu'un ajustement à la fois. Le
/// retrait du marqueur, lui, reste toujours possible.
pub fn toggle_fp(archive: &MultirideArchive, segment: usize) -> Result<MultirideArchive, String> {
    let passages = segment_passages(&archive.passages, segment);
    if passages.is_empty() {
        return Err(format!("Segment introuvable : {}.", segment));
    }

    // Démarquer si le segment est déjà écarté, le marquer sinon.
    let mark = !passages.iter().all(|p| p.faux_positif);
    if mark && passages.iter().any(|p| p.fusionne) {
        return Err(format!(
            "Le segment {} a été fusionné : annulez la fusion pour le marquer faux positif.",
            segment
        ));
    }

    let mut updated = archive.clone();
    for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
        passage.faux_positif = mark;
    }
    Ok(updated)
}

/// Refuse une fusion dont un des deux camps est écarté.
///
/// Un faux positif ne fusionne ni avec son précédent ni avec son suivant : le
/// résultat serait à la fois fusionné et écarté, et l'utilisateur perdrait le
/// moyen de défaire l'un sans défaire l'autre.
fn assert_mergeable(passages: &[MultiridePassage], segment: usize) -> Result<(), String> {
    if passages.iter().any(|p| p.faux_positif) {
        return Err(format!(
            "Le segment {} est un faux positif : un faux positif ne peut pas être fusionné.",
            segment
        ));
    }
    Ok(())
}

/// Étend un emprunt sur l'étendue d'un autre, sans jamais rétrécir.
///
/// L'emprunt qui absorbe garde son entrée ; seule sa sortie est repoussée, et
/// avec elle sa longueur et ses bornes de points — l'ordre de parcours est
/// conservé.
fn extend(last: &mut MultiridePassage, absorbed: &MultiridePassage) {
    if absorbed.km_sortie > last.km_sortie {
        last.km_sortie = absorbed.km_sortie;
        last.longueur_km = last.km_sortie - last.km_entree;
        last.point_sortie = absorbed.point_sortie;
        last.sortie = absorbed.sortie;
    }
}

/// Fusionne un segment avec le précédent (§F-14).
///
/// Les emprunts des deux segments sont repris dans l'ordre de la trace, puis
/// fusionnés deux à deux lorsqu'ils vont **dans le même sens** et que le trou
/// qui les sépare tient dans [`MERGE_MANUAL_TOL_KM`]. Le premier emprunt du
/// résultat devient la référence du segment fusionné ; les anciennes références
/// qui ne sont plus en tête basculent en « aller ».
///
/// Les **emprunts d'avant** sont enregistrés sur le résultat
/// (`avant_fusion`) : la fusion réécrit bornes et numérotation, elle ne se
/// recalcule pas, et c'est cet instantané qui rend [`undo_segment`] possible.
/// Il est pris *avant* toute mutation et conserve les emprunts tels quels —
/// avec leur propre enregistrement, s'ils en portaient un. Un segment peut donc
/// absorber son précédent puis son suivant : la chaîne s'annule ensuite de la
/// plus récente à la plus ancienne.
///
/// La fusion est refusée si l'un des deux segments est écarté (faux positif) :
/// le résultat serait à la fois fusionné et exclu de l'export, sans qu'aucun des
/// deux gestes ne puisse être annulé seul.
pub fn merge_segment(
    archive: &MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    if segment < 2 {
        return Err("Le premier segment n'a pas de précédent à fusionner.".to_string());
    }
    let previous = segment - 1;
    let group = [
        segment_passages(&archive.passages, previous),
        segment_passages(&archive.passages, segment),
    ];
    if group[0].is_empty() || group[1].is_empty() {
        return Err(format!(
            "Segments à fusionner introuvables : {} et {}.",
            previous, segment
        ));
    }
    assert_mergeable(&group[0], previous)?;
    assert_mergeable(&group[1], segment)?;

    // Instantané d'avant fusion, dans l'ordre d'origine des emprunts — c'est
    // lui que réinstallera l'annulation, numéros de segment compris.
    let mut snapshot: Vec<MultiridePassage> =
        group[0].iter().chain(group[1].iter()).cloned().collect();
    snapshot.sort_by_key(|p| (p.segment, p.passage));

    // Ordre de la trace : par début, puis par fin.
    let mut ordered: Vec<MultiridePassage> = group.into_iter().flatten().collect();
    ordered.sort_by(|a, b| {
        a.km_entree
            .partial_cmp(&b.km_entree)
            .unwrap_or(Ordering::Equal)
            .then(a.km_sortie.partial_cmp(&b.km_sortie).unwrap_or(Ordering::Equal))
    });

    let mut merged: Vec<MultiridePassage> = Vec::new();
    for passage in ordered {
        if let Some(last) = merged.last_mut() {
            let same_direction = sens_rank(last.sens) == sens_rank(passage.sens);
            let gap = passage.km_entree - last.km_sortie;
            if same_direction && gap <= MERGE_MANUAL_TOL_KM {
                extend(last, &passage);
                continue;
            }
        }
        merged.push(passage);
    }

    for (index, passage) in merged.iter_mut().enumerate() {
        passage.sens = if index == 0 {
            MultirideSens::Reference
        } else if passage.sens == MultirideSens::Reference {
            MultirideSens::Aller
        } else {
            passage.sens
        };
        passage.segment = previous;
        passage.passage = index + 1;
        passage.fusionne = true;
        // Refusé plus haut : aucun des deux segments n'est écarté.
        passage.faux_positif = false;
        passage.avant_fusion = Some(snapshot.clone());
    }

    // Les segments suivants se décalent d'un rang ; leur numérotation interne
    // est intacte, aucun de leurs emprunts n'ayant bougé.
    let mut updated: Vec<MultiridePassage> = archive
        .passages
        .iter()
        .filter(|p| p.segment != previous && p.segment != segment)
        .cloned()
        .collect();
    for passage in updated.iter_mut() {
        if passage.segment > segment {
            passage.segment -= 1;
        }
    }
    updated.extend(merged);
    updated.sort_by_key(|p| (p.segment, p.passage));

    Ok(MultirideArchive {
        passages: updated,
        ..archive.clone()
    })
}

/// Annule l'ajustement d'un segment (§F-14, §F-15).
///
/// Un segment ne portant qu'un ajustement à la fois, la nature de l'annulation
/// se lit dans l'état reçu :
///
/// - **faux positif** : aucune donnée à restaurer, le marqueur est retiré ;
/// - **fusion** : l'instantané `avant_fusion` est réinstallé tel quel — emprunts
///   d'origine, numéros de segment d'origine, et l'enregistrement qu'ils
///   portaient eux-mêmes. Les segments que la fusion avait décalés d'un rang
///   (`segment` exclu, tout ce qui le suit) reprennent leur numéro.
///
/// La réinstallation de l'instantané **imbriqué** est ce qui permet d'annuler
/// une chaîne de fusions pas à pas : défaire la fusion la plus récente rend au
/// segment son état d'alors, où l'enregistrement de la fusion précédente est
/// toujours présent, donc annulable à son tour.
///
/// Un segment sans ajustement, ou fusionné sans enregistrement — cas d'un
/// fichier écrit avant l'introduction du champ —, est refusé.
pub fn undo_segment(
    archive: &MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let passages = segment_passages(&archive.passages, segment);
    if passages.is_empty() {
        return Err(format!("Segment introuvable : {}.", segment));
    }

    // Faux positif : le marqueur est le seul état à défaire. Le test porte sur
    // « au moins un emprunt marqué » — le marquage est un geste de segment, et
    // l'annulation doit pouvoir le défaire même sur un état partiellement
    // marqué, qu'aucune commande ne produit mais qu'un fichier peut porter.
    if passages.iter().any(|p| p.faux_positif) {
        let mut updated = archive.clone();
        for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
            passage.faux_positif = false;
        }
        return Ok(updated);
    }

    // Fusion : l'instantané porte l'état d'avant, tel quel.
    if !passages.iter().all(|p| p.avant_fusion.is_some()) {
        return Err(format!(
            "Aucun ajustement à annuler sur le segment {}.",
            segment
        ));
    }
    // Le même instantané est porté par chaque emprunt du segment fusionné —
    // comme `faux_positif` et `fusionne`, qui sont des faits de segment : on ne
    // le lit donc qu'une fois.
    let snapshot: Vec<MultiridePassage> = passages
        .iter()
        .find_map(|p| p.avant_fusion.clone())
        .unwrap_or_default();

    let mut updated: Vec<MultiridePassage> = archive
        .passages
        .iter()
        .filter(|p| p.segment != segment)
        .cloned()
        .collect();
    // Les segments décalés par la fusion remontent d'un rang.
    for passage in updated.iter_mut() {
        if passage.segment > segment {
            passage.segment += 1;
        }
    }
    updated.extend(snapshot);
    updated.sort_by_key(|p| (p.segment, p.passage));

    Ok(MultirideArchive {
        passages: updated,
        ..archive.clone()
    })
}
