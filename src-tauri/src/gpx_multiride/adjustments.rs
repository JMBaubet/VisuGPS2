//! Ajustements manuels de la détection (spécification §F-14 à §F-16).
//!
//! Quatre gestes, sur des fonctions **pures** — l'écriture du fichier
//! appartient aux commandes :
//!
//! - **validation** d'un segment : le geste le plus fréquent, celui qui dit
//!   qu'un segment détecté est un vrai passage multiple, sans rien y changer ;
//! - **fusion** d'un segment avec le précédent : la décision explicite de
//!   l'utilisateur que deux tronçons n'en font qu'un (deux rues d'un même
//!   carrefour, portions séparées par une interruption de trace) ;
//! - **marquage faux positif** d'un segment : détecté à tort, exclu de l'export
//!   et des kilomètres répétés ;
//! - **annulation** d'un ajustement, par segment (voir [`undo_segment`]) ;
//! - **réinitialisation**, qui rejoue la détection — portée par la commande, la
//!   détection n'étant pas du ressort de ce module.
//!
//! **Deux axes, et non un seul état.** Une **fusion** réorganise la détection :
//! elle ne dit rien de la justesse du résultat, et le segment fusionné est un
//! segment **neuf**, qui perd donc l'approbation de ses deux camps — il reste à
//! approuver. Un **verdict**, lui, porte sur le segment dans son état courant :
//! approuvé, ou écarté, les deux s'excluant puisqu'un segment exclu de l'export
//! ne peut pas être dit juste. Un segment fusionné peut donc être approuvé, et
//! la fusion s'annule indépendamment du verdict (l'annulation retire d'abord le
//! verdict, puis la fusion).
//!
//! Les gestes se refusent ainsi entre eux : une fusion qui prendrait un segment
//! écarté, et l'approbation d'un segment écarté. Approuver ne conservant aucune
//! donnée, écarter un segment approuvé **efface** simplement l'approbation ; une
//! fusion, elle, garde l'enregistrement des emprunts d'avant — et cet
//! enregistrement porte aussi les approbations, que l'annulation restitue donc
//! avec le reste.
//!
//! Les fusions **s'enchaînent** : un segment peut absorber son précédent puis
//! le suivant, et la chaîne s'annule de la plus récente à la plus ancienne.
//!
//! **Tout geste invalide la validation de la détection.** Elle portait sur un
//! état qui n'est plus celui-ci : l'édition caméra se referme donc, jusqu'à ce
//! que la détection modifiée soit relue et validée à nouveau. Le statut du
//! registre suit, écrit par les commandes — les gestes, eux, n'entretiennent
//! aucun état et ne touchent pas au registre.

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

/// Invalide la validation de la **détection**.
///
/// Tout geste sur un segment passe par ici : la détection validée n'est plus
/// celle qu'on regarde, et l'édition caméra doit se refermer le temps qu'elle
/// soit relue. Le statut du registre suit, à la charge des commandes — les
/// gestes n'entretiennent aucune état, et n'écrivent pas le registre eux-mêmes.
///
/// À ne pas confondre avec `valide` sur un **emprunt**, qui est l'approbation de
/// son segment : approuver un segment ne vaut pas valider la détection, et
/// inversement.
fn invalidate(archive: MultirideArchive) -> MultirideArchive {
    MultirideArchive {
        valide: false,
        ..archive
    }
}

/// Marque ou démarque un segment en faux positif (§F-15).
///
/// Un segment est faux positif lorsque **tous** ses emprunts le sont : c'est le
/// geste de l'utilisateur, porté par chaque emprunt du segment, que le fichier
/// conserve afin qu'une réouverture retrouve le marquage.
///
/// Le marquage est refusé sur un segment **fusionné** : la fusion et le faux
/// positif s'excluent, un segment ne portant qu'un état à la fois. Sur un
/// segment **approuvé**, il passe et efface l'approbation — rien n'est perdu,
/// une approbation ne portant aucune donnée. Le retrait du marqueur, lui, reste
/// toujours possible.
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
        // Écarter un segment approuvé est une décision plus forte : elle prend
        // sa place. Un segment ne porte qu'un verdict.
        if mark {
            passage.valide = false;
        }
    }
    Ok(invalidate(updated))
}

/// Approuve un segment tel qu'il a été détecté (§F-15).
///
/// C'est le geste le plus fréquent — un segment sur lequel il n'y a rien à
/// redire —, et le seul qui n'ait besoin d'aucune donnée pour être défait.
///
/// L'approbation porte sur le segment **dans son état courant** : elle est donc
/// ouverte sur un segment fusionné, qui doit être approuvé comme les autres —
/// une fusion réorganise la détection, elle ne dit rien de sa justesse. Refusée
/// en revanche sur un segment **écarté** : un segment exclu de l'export ne
/// s'approuve pas, la contradiction serait dans les termes. L'appelant l'annonce
/// en grisant le bouton ; le refus est ici la défense en profondeur.
pub fn validate_segment(
    archive: &MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let passages = segment_passages(&archive.passages, segment);
    if passages.is_empty() {
        return Err(format!("Segment introuvable : {}.", segment));
    }
    if passages.iter().any(|p| p.faux_positif) {
        return Err(format!(
            "Le segment {} est un faux positif : un segment écarté ne peut pas être approuvé.",
            segment
        ));
    }

    // Le champ remis à `true` ici est l'approbation du **segment** ; celui que
    // `invalidate` remet à `false` est la validation de la **détection**, la
    // barrière de l'édition caméra. Un segment approuvé ne vaut pas une
    // détection validée.
    let mut updated = archive.clone();
    for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
        passage.valide = true;
    }
    Ok(invalidate(updated))
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
        // Le segment fusionné est un segment **neuf** : une approbation portée
        // par l'un des deux camps ne le couvre pas. Elle n'est pas perdue pour
        // autant — l'instantané la conserve, et l'annulation la restitue.
        passage.valide = false;
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

    Ok(invalidate(MultirideArchive {
        passages: updated,
        ..archive.clone()
    }))
}

/// Annule un geste sur un segment (§F-14, §F-15).
///
/// Les gestes étant de deux ordres — un **verdict** sur le segment (approuvé,
/// écarté) et une **réorganisation** (la fusion) —, l'annulation retire d'abord
/// le verdict, puis la fusion : un segment écarté redevient à examiner, un
/// segment approuvé de même, et un segment fusionné retrouve ses emprunts
/// d'avant. C'est l'ordre des gestes : on revient sur son avis avant de défaire
/// la réorganisation sur laquelle il portait.
///
/// La fusion s'annule en réinstallant l'instantané `avant_fusion` tel quel —
/// emprunts d'origine, numéros de segment d'origine, et l'enregistrement qu'ils
/// portaient eux-mêmes. Les segments que la fusion avait décalés d'un rang
/// (`segment` exclu, tout ce qui le suit) reprennent leur numéro.
///
/// La réinstallation de l'instantané **imbriqué** est ce qui permet d'annuler
/// une chaîne de fusions pas à pas : défaire la fusion la plus récente rend au
/// segment son état d'alors, où l'enregistrement de la fusion précédente est
/// toujours présent, donc annulable à son tour. L'instantané portant l'état
/// complet des emprunts, il restitue aussi les approbations qu'ils avaient.
///
/// Un segment sans geste, ou fusionné sans enregistrement — cas d'un fichier
/// écrit avant l'introduction du champ —, est refusé.
pub fn undo_segment(
    archive: &MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let passages = segment_passages(&archive.passages, segment);
    if passages.is_empty() {
        return Err(format!("Segment introuvable : {}.", segment));
    }

    // Écarté : le marqueur est le seul état à défaire. Le test porte sur « au
    // moins un emprunt marqué » — le marquage est un geste de segment, et
    // l'annulation doit pouvoir le défaire même sur un état partiellement
    // marqué, qu'aucune commande ne produit mais qu'un fichier peut porter.
    if passages.iter().any(|p| p.faux_positif) {
        let mut updated = archive.clone();
        for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
            passage.faux_positif = false;
        }
        return Ok(invalidate(updated));
    }

    // Approuvé : rien à restaurer, l'approbation se retire comme elle s'est
    // posée — la fusion éventuelle du segment, elle, reste en place et reste
    // annulable au coup suivant.
    if passages.iter().any(|p| p.valide) {
        let mut updated = archive.clone();
        for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
            passage.valide = false;
        }
        return Ok(invalidate(updated));
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

    Ok(invalidate(MultirideArchive {
        passages: updated,
        ..archive.clone()
    }))
}
