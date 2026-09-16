//! Rééchantillonnage de la trace à pas quasi constant.
//!
//! Deuxième étape du pipeline : la trace nettoyée est ramenée à des points
//! équidistants **le long de la trace**. C'est la prémisse de toute la
//! détection : elle rend les comparaisons d'intervalles homogènes quelle que
//! soit la densité de points du GPX d'origine, et permet de convertir les seuils
//! exprimés en mètres en nombres de points par simple division.

use super::projection::MultirideGeom;

/// Plafond du nombre de points rééchantillonnés (spécification §6.3) : au-delà,
/// le pas est relevé automatiquement et le fait est consigné dans le fichier de
/// description (`pas_plafonne`).
pub const MAX_SAMPLES: usize = 120_000;

/// Tolérance de comparaison des distances cumulées (m).
///
/// Le curseur du rééchantillonnage compare des distances calculées : un
/// échantillon qui tombe **exactement** sur un sommet de la géométrie peut, à
/// cause du bruit flottant, être rattaché au point précédent — et le numéro de
/// point du contrat décalé d'une unité. Le grain retenu (1 µm) est très fin
/// devant la précision d'un relevé GPS, et les points de la géométrie sont
/// garantis espacés d'au moins 5 cm (dédoublonnage) : il ne peut donc jamais
/// faire sauter un sommet réel.
const CUM_EPS_M: f64 = 1e-6;

/// Trace rééchantillonnée — entrée de la détection des correspondances.
#[derive(Debug)]
pub struct Resample {
    /// Nombre d'intervalles : la trace compte `n + 1` points échantillonnés.
    pub n: usize,
    /// Pas **effectif** (m) : `total / n`, calé pour que le dernier point tombe
    /// exactement sur la fin de trace. Il peut différer du pas demandé lorsque
    /// le garde-fou a relevé celui-ci.
    pub step: f64,
    /// Coordonnées géographiques des points échantillonnés.
    pub lat: Vec<f64>,
    pub lon: Vec<f64>,
    /// Coordonnées métriques (index spatial de la détection).
    pub px: Vec<f64>,
    pub py: Vec<f64>,
    /// Distance cumulée **le long de la trace** depuis le départ (m).
    pub cum: Vec<f64>,
    /// Point de la géométrie nettoyée précédant chaque point échantillonné.
    pub orig: Vec<usize>,
    /// `true` lorsque le pas a été relevé pour contenir le nombre de points.
    pub capped: bool,
}

/// Rééchantillonne la trace nettoyée au pas demandé, sous le plafond de la
/// spécification.
pub fn resample(geom: &MultirideGeom, step_m: f64) -> Result<Resample, String> {
    resample_capped(geom, step_m, MAX_SAMPLES)
}

/// Rééchantillonnage à plafond explicite — le garde-fou est ainsi testable sans
/// construire une trace de plusieurs centaines de kilomètres.
pub fn resample_capped(
    geom: &MultirideGeom,
    step_m: f64,
    max_samples: usize,
) -> Result<Resample, String> {
    if !(step_m > 0.0) {
        return Err(format!("Pas d'échantillonnage invalide : {} m.", step_m));
    }
    if max_samples == 0 {
        return Err("Plafond d'échantillonnage nul.".to_string());
    }
    let count = geom.px.len();
    if !(geom.total > 0.0) {
        return Err("Trace de longueur nulle.".to_string());
    }

    // Le nombre d'intervalles est calé sur la fin de trace : le pas effectif est
    // `total / n`, de sorte qu'aucun reliquat ne subsiste après le dernier point.
    let wanted = (geom.total / step_m).round().max(1.0);
    let capped = wanted > max_samples as f64;
    let n = if capped { max_samples } else { wanted as usize };
    let eff = geom.total / n as f64;

    let mut lat = Vec::with_capacity(n + 1);
    let mut lon = Vec::with_capacity(n + 1);
    let mut px = Vec::with_capacity(n + 1);
    let mut py = Vec::with_capacity(n + 1);
    let mut cum = Vec::with_capacity(n + 1);
    let mut orig = Vec::with_capacity(n + 1);

    // Curseur monotone sur les points nettoyés : il ne recule jamais, ce qui
    // borne le coût total du parcours à O(n + count).
    let mut k = 0usize;
    for i in 0..=n {
        let d = (i as f64) * eff;
        while k + 1 < count && geom.cum[k + 1] <= d + CUM_EPS_M {
            k += 1;
        }

        // Interpolation linéaire entre les deux points encadrant la cible. Au
        // dernier point (`k` est le dernier de la géométrie), il n'y a plus rien
        // à interpoler : la cible est le point lui-même.
        let (x, y) = if k + 1 < count {
            let span = geom.cum[k + 1] - geom.cum[k];
            let t = if span > 0.0 {
                ((d - geom.cum[k]) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (
                geom.px[k] + (geom.px[k + 1] - geom.px[k]) * t,
                geom.py[k] + (geom.py[k + 1] - geom.py[k]) * t,
            )
        } else {
            (geom.px[k], geom.py[k])
        };
        let (lat_i, lon_i) = geom.proj.inv(x, y);

        lat.push(lat_i);
        lon.push(lon_i);
        px.push(x);
        py.push(y);
        // `d` est la distance parcourue le long de la trace jusqu'à ce point :
        // c'est la définition même du rééchantillonnage — et la valeur que
        // réclame le contrat pour `km_entree` / `km_sortie` (distance cumulée le
        // long de la trace, et non distance euclidienne entre bornes).
        cum.push(d);
        orig.push(k);
    }

    Ok(Resample {
        n,
        step: eff,
        lat,
        lon,
        px,
        py,
        cum,
        orig,
        capped,
    })
}

/// Numéro de point GPX (1-based) porté par un point rééchantillonné.
///
/// C'est l'**espace d'index du contrat** : `point_entree` et `point_sortie` du
/// fichier de description désignent les `<trkpt>` du GPX d'origine, et non les
/// points de la trace nettoyée — sans quoi la jointure décrite en annexe 13.6 ne
/// tomberait plus juste dès qu'un point a été écarté au dédoublonnage.
pub fn raw_point_number(geom: &MultirideGeom, resample: &Resample, index: usize) -> usize {
    geom.raw_index[resample.orig[index]] + 1
}
