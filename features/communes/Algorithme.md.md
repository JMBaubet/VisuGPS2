# Spécification technique — Algorithme COMMUNES

**Description complète de la détection des communes traversées**

| | |
|---|---|
| **Document** | Spécification algorithmique |
| **Application** | Communes GPX (module d'analyse) |
| **Version algorithmique** | v1.0 |
| **Entrée** | Trace GPX nettoyée (points lat/lon ordonnés), longueur cumulée |
| **Sortie** | Liste ordonnée de passages `{ commune, km_entree, km_sortie, longueur }` + paramètres |

---

## Table des matières

1. [Vue d'ensemble](#1-vue-densemble)
2. [Simplification et bounding box](#2-simplification-et-bounding-box)
3. [Requête Overpass](#3-requête-overpass)
4. [Pré-filtrage géométrique](#4-pré-filtrage-géométrique)
5. [Analyse fine : timeline et arbitrage](#5-analyse-fine--timeline-et-arbitrage)
6. [Raffinement des transitions](#6-raffinement-des-transitions)
7. [Calcul du km par interpolation locale](#7-calcul-du-km-par-interpolation-locale)
8. [Post-traitement](#8-post-traitement)
9. [Analyse de complexité](#9-analyse-de-complexité)
10. [Invariants et correction](#10-invariants-et-correction)
11. [Limites connues](#11-limites-connues)

---

## 1. Vue d'ensemble

Le pipeline se décompose en 7 étapes principales :

```
GPX → Simplification → BBox → Overpass → Filtrage → Analyse fine
    → Raffinement → Post-traitement → Tri
```

| Étape | Entrée | Sortie | Rôle |
|---|---|---|---|
| Simplification | Trace brute | Trace simplifiée (5 m) | Réduire le bruit et le coût |
| BBox | Trace simplifiée | Bbox élargie (1 km) | Zone de recherche Overpass |
| Overpass | BBox | Relations OSM | Récupérer les polygones communaux |
| Filtrage | Relations + trace | Candidates | Éliminer les non-intersectantes et aberrantes |
| Analyse fine | Candidates + trace | Timeline + segments | Identifier qui gagne où |
| Raffinement | Segments bruts | Bornes précises | Précision des entrées/sorties |
| Post-traitement | Passages raffinés | Passages finaux | Fusion courte, contiguë, oscillation |

---

## 2. Simplification et bounding box

### 2.1 Simplification Turf

```
turf.simplify(line, { tolerance: 0.00005, highQuality: true })
```

- Tolérance ≈ 5 m (0.00005°).
- Algorithme : Douglas-Peucker avec option haute qualité.
- Sur une trace de 626 points, on passe à environ 50-80 points.

**Justification** : une simplification trop légère (< 1 m) n'apporte rien puisque la tolérance OSM est de l'ordre de quelques mètres. Une simplification trop agressive (> 20 m) risque de déplacer la trace hors des polygones dans les zones étroites.

### 2.2 Calcul de la bounding box

```
rawBbox = turf.bbox(simplified)   // [minLon, minLat, maxLon, maxLat]
bbox = [
  rawBbox[0] - buffer,   // minLon
  rawBbox[1] - buffer,   // minLat
  rawBbox[2] + buffer,   // maxLon
  rawBbox[3] + buffer    // maxLat
]
```

Avec `buffer = bbox_buffer_deg` (défaut 0.01°, ≈ 1 km).

**Justification** : une commune traversée en bordure de trace peut avoir son polygone qui s'étend au-delà de la bbox brute. Le buffer garantit que ces communes sont bien capturées.

**Compromis** : un buffer plus grand (0.05°) couvre mieux les cas marginaux, mais augmente le nombre de relations Overpass retournées. Sur une zone dense (agglomération), cela peut mener à des HTTP 504 (timeout serveur).

---

## 3. Requête Overpass

### 3.1 Construction de la requête

```
[out:json][timeout:180];
relation["boundary"="administrative"]["admin_level"="8"](S,W,N,E);
out geom;
```

- `boundary=administrative` : filtre sur les limites administratives.
- `admin_level=8` : niveau des communes en France, Espagne, Italie, Suisse.
- `out geom` : retourne la géométrie complète des relations (et non juste leurs IDs).

### 3.2 Conversion bbox

Turf retourne `[minLon, minLat, maxLon, maxLat]`. Overpass attend `(minLat, minLon, maxLat, maxLon)`. Conversion :

```
S = bbox[1], W = bbox[0], N = bbox[3], E = bbox[2]
```

### 3.3 Fallback sur miroirs

Quatre endpoints en cascade :

1. `https://overpass-api.de/api/interpreter` — serveur principal, souvent surchargé.
2. `https://overpass.kumi.systems/api/interpreter` — miroir rapide.
3. `https://overpass.private.coffee/api/interpreter` — miroir communautaire.
4. `https://overpass.osm.ch/api/interpreter` — miroir suisse.

En cas d'erreur HTTP (4xx/5xx), de timeout ou d'erreur réseau, passe au suivant. Lève une erreur `ERR_OVERPASS_FAILED` si tous échouent.

Le champ `overpass_endpoint_used` et `overpass_attempts` sont enregistrés dans les paramètres de sortie.

---

## 4. Pré-filtrage géométrique

### 4.1 Conversion OSM → GeoJSON

`osmtogeojson(data, { flatProperties: false })` transforme la réponse JSON Overpass en `FeatureCollection`. Chaque `Feature` correspond à une relation de limite administrative.

Seules les features avec un tag `name` non vide sont conservées.

### 4.2 Filtre d'intersection

```js
const candidates = [];
for (const f of withName) {
  if (turf.booleanIntersects(simplified, f)) candidates.push(f);
}
```

Garde uniquement les polygones qui croisent la trace.

### 4.3 Filtre des polygones aberrants

```js
const areas = candidates.map(safeArea).filter(isFinite);
const medArea = median(areas);
const threshold = medArea * max_area_ratio;   // défaut 10
const kept = candidates.filter(c => safeArea(c) <= threshold);
```

**Justification** : dans de rares cas, un polygone OSM est cassé (multipolygon mal assemblé, anneau extérieur démesuré). Sa surface est alors anormalement grande. Le seuil de 10× la médiane élimine ces cas.

**Sécurité** : si le filtre élimine toutes les candidates, on retombe sur la liste complète.

---

## 5. Analyse fine : timeline et arbitrage

### 5.1 Échantillonnage

```js
const step = Math.max(min_sample_step_km, lengthKm / target_samples);
const samples = [];
for (let d = 0; d <= lengthKm; d += step) {
  samples.push({ d, pt: turf.along(simplified, d, { units: 'kilometers' }) });
}
```

Défauts : pas de 100 m minimum, 2000 échantillons max.

| Longueur de trace | Pas effectif | Nb échantillons |
|---|---|---|
| 5 km | 100 m | 50 |
| 75 km | 100 m | 750 |
| 200 km | 100 m | 2000 |
| 400 km | 200 m | 2000 |

### 5.2 Construction de la timeline

Pour chaque échantillon, on identifie la commune gagnante :

```js
for (let i = 0; i < samples.length; i++) {
  const [lon, lat] = samples[i].pt.geometry.coordinates;
  const matches = [];

  // 1. Filtrer par bbox (rejet rapide)
  for (const c of enriched) {
    const [minX, minY, maxX, maxY] = c.bbox;
    if (lon < minX || lon > maxX || lat < minY || lat > maxY) continue;

    // 2. Test exact
    if (turf.booleanPointInPolygon(samples[i].pt, c.feature)) matches.push(c);
  }

  // 3. Arbitrage par distance au centroïde
  let winner = matches[0];
  if (matches.length > 1) {
    let bestDist = Infinity;
    for (const m of matches) {
      const d = turf.distance(samples[i].pt, m.centroid, { units: 'kilometers' });
      if (d < bestDist) { bestDist = d; winner = m; }
    }
  }

  timeline[i] = winner || null;
}
```

### 5.3 Arbitrage par centroïde

**Règle** : en cas de chevauchement, la commune dont le centroïde est le plus proche du point gagne.

**Pourquoi pas la plus petite surface ?** Cette règle donne de mauvais résultats quand une commune légitime est plus grande que sa voisine, ou quand un polygone « fantôme » a une surface normale mais une géométrie défectueuse qui s'étend sur son voisin.

**Pourquoi la distance au centroïde ?** Un polygone fantôme a son centroïde **au bon endroit** (au centre de sa commune réelle), même s'il s'étend bien au-delà. Donc sur le territoire d'une commune voisine, c'est la voisine qui a le centroïde le plus proche. Cette règle est :

- **indépendante de la surface** — fonctionne même quand la commune fantôme est plus petite que sa voisine ;
- **robuste aux géométries OSM cassées** — le centroïde est rarement affecté par les artefacts de bord ;
- **déterministe** — ne dépend pas de l'ordre d'énumération des communes.

### 5.4 Comblement des micro-trous

Un trou de timeline (échantillon sans commune) est comblé uniquement si :

- sa longueur ≤ `max_gap_fill_samples` (défaut 1 = 100 m) ;
- les deux voisins encadrant désignent la **même** commune.

```js
for (let i = 0; i < timeline.length; i++) {
  if (timeline[i] !== null) continue;
  let gapEnd = i;
  while (gapEnd < timeline.length && timeline[gapEnd] === null) gapEnd++;
  const gapLen = gapEnd - i;
  if (gapLen > maxGap) { i = gapEnd - 1; continue; }

  const prev = i > 0 ? timeline[i - 1] : null;
  const next = gapEnd < timeline.length ? timeline[gapEnd] : null;
  if (prev && next && prev.idx === next.idx) {
    for (let j = i; j < gapEnd; j++) timeline[j] = prev;
  }
  i = gapEnd - 1;
}
```

**Justification** : un trou d'un seul échantillon flanqué de la même commune est presque toujours un artefact de précision (frontière communale imprécise, trou OSM ponctuel). Un trou plus grand, ou flanqué de deux communes différentes, marque une vraie transition et doit être conservé pour que les deux passages soient extraits séparément.

### 5.5 Extraction des segments contigus

```js
let current = null, segStart = -1;
const rawSegments = [];
for (let i = 0; i <= timeline.length; i++) {
  const w = i < timeline.length ? timeline[i] : null;
  const wIdx = w ? w.idx : null;
  const curIdx = current ? current.idx : null;

  if (wIdx !== curIdx) {
    if (current && segStart >= 0) {
      rawSegments.push({ name: current.name, startIdx: segStart, endIdx: i - 1 });
    }
    if (w) { current = w; segStart = i; }
    else { current = null; segStart = -1; }
  }
}
```

Chaque transition crée un nouveau segment rattaché à la commune gagnante.

---

## 6. Raffinement des transitions

Pour chaque segment, on raffine l'entrée et la sortie :

```js
function refineTransition(sampleBefore, sampleAfter, feature) {
  const seg = turf.lineString([
    sampleBefore.pt.geometry.coordinates,
    sampleAfter.pt.geometry.coordinates
  ]);
  const intersections = turf.lineIntersect(seg, feature);
  if (intersections.features.length > 0) {
    // Choisir l'intersection la plus proche du point "after"
    const afterCoord = sampleAfter.pt.geometry.coordinates;
    let best = intersections.features[0].geometry.coordinates;
    let bestDist = turf.distance(turf.point(best), turf.point(afterCoord));
    for (let i = 1; i < intersections.features.length; i++) {
      const c = intersections.features[i].geometry.coordinates;
      const d = turf.distance(turf.point(c), turf.point(afterCoord));
      if (d < bestDist) { best = c; bestDist = d; }
    }
    return best;
  }
  return sampleAfter.pt.geometry.coordinates;
}
```

**Principe** : on calcule l'intersection exacte entre le petit segment de trace (≤ 100 m) et la frontière du polygone. C'est cette intersection qui donne la position précise du franchissement.

**Cas particulier** : si `turf.lineIntersect` échoue (géométrie OSM invalide) ou retourne 0 intersection, on prend le point `sampleAfter` comme approximation.

**Précision obtenue** : de l'ordre du mètre sur la position du franchissement, bien meilleure que le pas d'échantillonnage (100 m).

---

## 7. Calcul du km par interpolation locale

```js
function kmAt(sampleBefore, sampleAfter, coord) {
  const d1 = turf.distance(sampleBefore.pt, turf.point(coord), { units: 'kilometers' });
  const d2 = turf.distance(turf.point(coord), sampleAfter.pt, { units: 'kilometers' });
  const total = d1 + d2;
  if (total < 1e-9) return sampleAfter.d;
  return sampleBefore.d + (d1 / total) * (sampleAfter.d - sampleBefore.d);
}
```

**Décision critique** : ne PAS utiliser `turf.nearestPointOnLine`.

Cette fonction cherche le point le plus proche sur **toute la ligne**, ce qui est problématique sur les traces aller-retour : une trace qui repasse au même endroit à 2 km puis à 12 km verrait son point d'entrée au km 2 associé au km 12 par erreur.

L'interpolation locale est bornée par les deux échantillons encadrant la transition (≤ 100 m d'écart), ce qui garantit :

- la **monotonie** : les km d'entrée/sortie sont strictement croissants le long de la trace ;
- l'**absence d'ambiguïté** : on sait toujours de quel passage il s'agit.

**Précision** : l'interpolation linéaire entre deux points distants de 100 m donne une erreur < 5 m sur la trace.

---

## 8. Post-traitement

Trois passes appliquées **itérativement jusqu'à stabilité** (max 50 itérations) :

### 8.1 Fusion des passages courts

Tout passage dont la longueur < `min_keep_km` (défaut 0,5 km = 500 m) est absorbé dans son voisin le plus long. Les bornes du voisin sont étendues en conséquence.

```js
function mergeShortSegments(flat, minKeepKm) {
  const out = [...flat];
  let changed = true, guard = 0;
  while (changed && guard < 100) {
    changed = false; guard++;
    for (let i = 0; i < out.length; i++) {
      const seg = out[i];
      if (seg.lengthKm >= minKeepKm) continue;
      if (out.length === 1) break;

      const prev = i > 0 ? out[i - 1] : null;
      const next = i < out.length - 1 ? out[i + 1] : null;

      let targetIdx;
      if (prev && next) targetIdx = prev.lengthKm >= next.lengthKm ? i - 1 : i + 1;
      else if (prev) targetIdx = i - 1;
      else targetIdx = i + 1;

      const target = out[targetIdx];
      if (targetIdx < i) target.exitKm = seg.exitKm;
      else target.entryKm = seg.entryKm;
      target.lengthKm = target.exitKm - target.entryKm;
      out.splice(i, 1);
      changed = true;
      break;
    }
  }
  return out;
}
```

**Justification** : un passage de quelques centaines de mètres qui ne correspond qu'à un artefact (frontière OSM imprécise, micro-chevauchement) pollue la lecture. L'absorption dans le voisin le plus long préserve la continuité kilométrique.

**Effet sur la longueur totale** : inchangée. La surface absorbée est simplement réattribuée au voisin.

### 8.2 Fusion des passages contigus

Deux passages adjacents de même commune sont systématiquement recollés :

```js
function mergeContiguous(flat) {
  const out = [...flat];
  let changed = true, guard = 0;
  while (changed && guard < 100) {
    changed = false; guard++;
    for (let i = 1; i < out.length; i++) {
      if (out[i].name === out[i - 1].name) {
        out[i - 1].exitKm = out[i].exitKm;
        out[i - 1].lengthKm = out[i - 1].exitKm - out[i - 1].entryKm;
        out.splice(i, 1);
        changed = true;
        break;
      }
    }
  }
  return out;
}
```

**Justification** : un trou de timeline mal comblé (par ex. 2 échantillons consécutifs) peut produire deux segments adjacents de la même commune. Cette passe les recolle.

### 8.3 Fusion des oscillations (optionnelle)

**Désactivée par défaut** (`oscillation_ratio = 0`).

Détection des patterns alternants `A-B-A-B-...` (au moins 4 segments) où la somme du minor est < `oscillation_ratio` × somme du major. Fusion en un seul passage de la commune dominante.

```js
function mergeOscillations(flat, ratio) {
  // ... (voir implémentation)
  // Condition : count >= 4 (A-B-A-B minimum)
  // Ratio    : totalMinor / totalDominant < ratio
}
```

**Justification de la désactivation par défaut** : le pattern `A-B-A` (3 segments) est un cas légitime d'aller-retour dans un couloir (par ex. Ospedaletti → Sanremo → Ospedaletti sur la trace Menton-Poggio). Seul un pattern `A-B-A-B` (≥ 4 segments) avec des fragments courts est réellement suspect.

**Activation recommandée** : uniquement sur des traces longeant une frontière communale (rivière, route frontière), où l'on observe de fausses alternances A-B-A-B.

### 8.4 Filtre des micro-segments (avant post-traitement)

Avant le post-traitement, tout passage dont la longueur < `min_segment_km` (défaut 0,015 km = 15 m) est supprimé.

**Justification** : ces micro-passages proviennent d'artefacts de simplification ou de polygones OSM imparfaits. Le seuil de 15 m est très conservateur.

---

## 9. Analyse de complexité

Soit :

- N = nombre d'échantillons (jusqu'à 2000) ;
- M = nombre de communes candidates (typiquement 5 à 30) ;
- K = nombre de passages finaux (typiquement 5 à 30).

| Étape | Coût |
|---|---|
| Simplification | O(N_gpx) |
| BBox | O(N_simplifié) |
| Requête Overpass | O(réseau) |
| Conversion GeoJSON | O(réponse) |
| Filtre d'intersection | O(M × N_simplifié) |
| Filtre de surface | O(M log M) |
| Échantillonnage | O(N) |
| Construction timeline | O(N × M) |
| Comblement trous | O(N) |
| Extraction segments | O(N) |
| Raffinement (par passage) | O(K × log(segment)) |
| Post-traitement | O(K²) |

**Dominé par** : O(N × M) pour la construction de la timeline.

Sur une trace de 200 km : N = 2000, M = 30 → 60 000 tests `booleanPointInPolygon`. Chaque test coûte ~10 µs → ~600 ms.

En pratique, le pré-filtre bbox rejette la majorité des paires, donc le coût réel est plus proche de O(N × M_effectif) où M_effectif est le nombre de communes dont la bbox contient effectivement le point (typiquement 1 à 3).

**Mémoire** : O(N + M + K), négligeable (< 10 Mo pour les cas extrêmes).

---

## 10. Invariants et correction

**INV-1 — Monotonie de la timeline**

Les segments sont extraits par parcours linéaire de la timeline, donc ordonnés naturellement par indice croissant.

**INV-2 — Unicité des passages**

Chaque transition `A → B` crée au plus un nouveau segment pour B. Une commune traversée 2 fois produit donc 2 segments distincts.

**INV-3 — Km croissants**

Après le tri final par `entryKm`, les km d'entrée sont strictement croissants entre passages consécutifs.

**INV-4 — Cohérence des longueurs**

`longueur = exitKm - entryKm`, toujours ≥ 0. Cette propriété est garantie par l'usage de l'interpolation locale (`kmAt`) qui est bornée par les échantillons encadrants.

**INV-5 — Filtre des micro-segments**

Tout segment < `min_segment_km` est ignoré avant le rendu, garantissant l'absence de passages artificiels.

**INV-6 — Indépendance de l'ordre d'itération**

L'arbitrage par centroïde ne dépend pas de l'ordre d'énumération des communes. Le résultat est donc **déterministe** : la même trace donne toujours la même analyse.

**INV-7 — Post-traitement conservatif**

Les passes `mergeShortSegments` et `mergeContiguous` préservent la longueur totale : `Σ longueur_km` reste constant à travers les fusions.

**INV-8 — Oscillation désactivée par défaut**

Le pattern `A-B-A` (aller-retour légitime) n'est jamais fusionné quand `oscillation_ratio = 0`. Le pattern `A-B-A-B` n'est fusionné que si `oscillation_ratio > 0` et si le minor est effectivement petit.

---

## 11. Limites connues

### 11.1 Précision aux frontières

Limitée par la précision des polygones OSM. Une erreur de ± 5 m sur la frontière se traduit par ± 5 m d'erreur sur le km d'entrée/sortie. Non corrigeable côté client.

### 11.2 Traces hors des 4 pays couverts

La détection échouera si `admin_level=8` ne correspond pas au niveau communal (par exemple au Portugal, en Allemagne, en Belgique, le niveau peut varier). Une extension serait possible en modifiant la requête Overpass.

### 11.3 Traces en pleine mer

Aucune commune trouvée, le résumé indique 0 relations. Comportement attendu.

### 11.4 Zones mal cartographiées dans OSM

Certaines zones rurales ou frontalières peuvent avoir des polygones incomplets ou manquants. Aucun contournement possible côté client.

### 11.5 Détection multi-niveaux

Impossible en l'état : seul `admin_level=8` est interrogé. Une extension permettrait de récupérer simultanément communes (`8`), départements (`6`), régions (`4`).

### 11.6 Cache absent

Chaque analyse refait une requête Overpass complète. Sur une zone dense (agglomération parisienne), la requête peut ramener 100+ relations, ce qui augmente le temps de traitement et le risque de HTTP 504.

### 11.7 Oscillations frontalières

Le paramètre `oscillation_ratio` peut être activé manuellement sur des traces longeant une frontière communale. Une valeur trop élevée (0.4+) peut fusionner de vrais aller-retour `A-B-A`, ce qui est sémantiquement incorrect.

### 11.8 Passages très courts (< 100 m)

Le filtre `min_keep_km` (défaut 500 m) absorbe les passages courts dans leur voisin. Une commune légitimement traversée sur 200 m serait perdue. C'est un compromis assumé : les passages de cette taille sont presque toujours des artefacts.

### 11.9 Trace multi-segments

Si le GPX contient plusieurs `<trkseg>`, ils sont aplatis (concaténation dans l'ordre du document). Une discontinuité entre deux segments (par ex. pause avec saut de plusieurs kilomètres) n'est pas détectée et produit une trace continue artificielle.

### 11.10 Déterminisme

Le résultat est déterministe pour un même jeu de paramètres, mais peut varier légèrement si l'extrait OSM évolue entre deux analyses (ajout, suppression ou modification de communes dans OSM).

---

**Fin du document.**