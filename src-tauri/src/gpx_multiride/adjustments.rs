//! Ajustements manuels de la détection (spécification §F-14 à §F-16).
//!
//! Trois gestes, sur des fonctions **pures** — l'écriture du fichier appartient
//! aux commandes :
//!
//! - **fusion** d'un segment avec le précédent : la décision explicite de
//!   l'utilisateur que deux tronçons n'en font qu'un (deux rues d'un même
//!   carrefour, portions séparées par une interruption de trace) ;
//! - **marquage faux positif** d'un segment : détecté à tort, exclu de l'export
//!   et des kilomètres répétés ;
//! - **réinitialisation**, qui rejoue la détection — portée par la commande, la
//!   détection n'étant pas du ressort de ce module.
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
pub fn toggle_fp(archive: &MultirideArchive, segment: usize) -> Result<MultirideArchive, String> {
    let passages = segment_passages(&archive.passages, segment);
    if passages.is_empty() {
        return Err(format!("Segment introuvable : {}.", segment));
    }

    // Démarquer si le segment est déjà écarté, le marquer sinon.
    let mark = !passages.iter().all(|p| p.faux_positif);

    let mut updated = archive.clone();
    for passage in updated.passages.iter_mut().filter(|p| p.segment == segment) {
        passage.faux_positif = mark;
    }
    Ok(updated)
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
/// Le segment fusionné est marqué faux positif si l'un des deux l'était : c'est
/// le choix conservateur — mieux vaut continuer d'exclure de l'export ce que
/// l'utilisateur avait écarté que de le réintroduire à son insu.
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

    let false_positive = merged.iter().any(|p| p.faux_positif);
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
        passage.faux_positif = false_positive;
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
