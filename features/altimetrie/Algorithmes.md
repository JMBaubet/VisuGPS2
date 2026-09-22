# Document Algorithmes — GPX Climb Analyzer

**Version** : 2.3
**Date** : 2026-09-21
**Public** : développeur, mainteneur, auditeur technique
**Portée** : description exhaustive de tous les algorithmes utilisés dans l'application

---

## Table des matières

1. [Distance cumulée (Haversine)](#1-distance-cumulée-haversine)
2. [Moyenne mobile (Moving Average)](#2-moyenne-mobile-moving-average)
3. [Lissage ancré (Anchored Moving Average)](#3-lissage-ancré-anchored-moving-average)
4. [Pente instantanée (différence centrée)](#4-pente-instantanée-différence-centrée)
5. [Pente par fenêtre glissante (Slope Window)](#5-pente-par-fenêtre-glissante-slope-window)
6. [Détection par hystérésis](#6-détection-par-hystérésis)
7. [Construction des candidats de montée](#7-construction-des-candidats-de-montée)
8. [Recherche du sous-segment optimal](#8-recherche-du-sous-segment-optimal)
9. [Argmin / Argmax sur intervalle](#9-argmin--argmax-sur-intervalle)
10. [Recherche binaire sur distance](#10-recherche-binaire-sur-distance)
11. [Catégorisation par pente](#11-catégorisation-par-pente)
12. [Fusion des segments courts](#12-fusion-des-segments-courts)
13. [Segmentation par catégorie (segments contigus)](#13-segmentation-par-catégorie-segments-contigus)
14. [Enrichissement des segments](#14-enrichissement-des-segments)
15. [Contrainte anti-chevauchement](#15-contrainte-anti-chevauchement)
16. [Sous-échantillonnage](#16-sous-échantillonnage)
17. [Garde-fou hybride de refinage (v2)](#17-garde-fou-hybride-de-refinage-v2)
18. [Insertion contextuelle de montée (v2.2)](#18-insertion-contextuelle-de-montée-v22)
19. [Édition directe par drag & drop (v0.5)](#19-édition-directe-par-drag--drop-v05)
20. [Pipeline complet de détection](#20-pipeline-complet-de-détection)
21. [Pipeline de reconstruction](#21-pipeline-de-reconstruction)
22. [Tableau récapitulatif des complexités](#22-tableau-récapitulatif-des-complexités)

---

## 1. Distance cumulée (Haversine)

### Objet

Calculer la distance orthodromique (à la surface d'une sphère) entre deux points GPS successifs, puis la cumuler pour obtenir la distance totale parcourue à chaque point de la trace.

### Formule

```
R  = 6 371 000 m (rayon terrestre moyen)
φ1, φ2 : latitudes en radians
λ1, λ2 : longitudes en radians
Δφ = φ2 − φ1
Δλ = λ2 − λ1

a = sin²(Δφ/2) + cos(φ1) × cos(φ2) × sin²(Δλ/2)
c = 2 × atan2(√a, √(1 − a))
d = R × c
```

### Pseudocode

```
FONCTION haversine(lat1, lon1, lat2, lon2):
  R ← 6371000
  φ1 ← lat1 × π / 180
  φ2 ← lat2 × π / 180
  Δφ ← (lat2 − lat1) × π / 180
  Δλ ← (lon2 − lon1) × π / 180
  a ← sin(Δφ/2)² + cos(φ1) × cos(φ2) × sin(Δλ/2)²
  RETOURNER 2 × R × atan2(√a, √(1 − a))

FONCTION calculerDistances(points):
  dist[0] ← 0
  POUR i DE 1 À n−1:
    d ← haversine(points[i−1].lat, points[i−1].lon,
                  points[i].lat,   points[i].lon)
    dist[i] ← dist[i−1] + d
  RETOURNER dist
```

### Complexité

- Temps : **O(n)** sur le nombre de points
- Espace : **O(n)** pour stocker les distances cumulées

### Notes

- L'utilisation de `atan2` plutôt que `asin` évite les erreurs numériques pour les faibles distances.
- L'approximation sphérique est suffisante pour des traces de quelques centaines de km (erreur < 0.5 % vs ellipsoïde WGS84).
- Alternative non retenue : formule de Vincenty (ellipsoïde), plus précise mais 10× plus lente sans bénéfice pratique.

---

## 2. Moyenne mobile (Moving Average)

### Objet

Lisser une série numérique (altitude, pente) pour éliminer le bruit GPS. Utilisé intensivement à plusieurs étapes du pipeline.

### Algorithme

Moyenne glissante centrée sur une fenêtre de taille `win` (impair).

```
FONCTION movingAverage(arr, win):
  n ← longueur(arr)
  SI win < 2 OU n < 2:
    RETOURNER copie(arr)

  half ← ⌊win / 2⌋

  // Préfixe cumulatif pour O(1) par point
  prefix[0] ← 0
  POUR i DE 0 À n−1:
    prefix[i+1] ← prefix[i] + arr[i]

  res ← nouveau tableau de taille n
  POUR i DE 0 À n−1:
    lo ← max(0, i − half)
    hi ← min(n − 1, i + half)
    res[i] ← (prefix[hi+1] − prefix[lo]) / (hi − lo + 1)

  RETOURNER res
```

### Complexité

- Temps : **O(n)** (grâce au préfixe cumulatif)
- Espace : **O(n)**

### Notes

- **Effet de bord** : sur les bords du tableau, la fenêtre est tronquée et la moyenne porte sur moins de points. Cela crée un effet d'atténuation de la pente au début/fin de la trace (voir §3 pour la correction).
- La taille de la fenêtre contrôle le compromis bruit / fidélité :
  - petit (5–20) : conserve le détail, sensible au bruit
  - moyen (30–50) : bon équilibre
  - grand (100+) : très lissé, perd les micro-variations

---

## 3. Lissage ancré (Anchored Moving Average)

### Objet

Lisser l'altitude **tout en forçant la courbe à passer exactement par certains points** (les extrema détectés). Cela évite le décalage du sommet vers la descente et corrige l'effet de bord au début de la trace.

### Principe

1. Calculer une moyenne mobile classique `base`
2. Pour chaque ancre `a`, calculer la correction `corr[a] = raw[a] − base[a]`
3. Appliquer une correction linéaire par morceaux entre ancres successives
4. Résultat : `result[a] = raw[a]` exactement, et la courbe reste lisse entre les ancres

### Pseudocode

```
FONCTION anchoredMovingAverage(raw, anchorIdxs, win):
  n ← longueur(raw)
  base ← movingAverage(raw, win)

  // Construire la liste d'ancres : index 0, n−1 et les ancres fournies
  set ← {0, n−1} ∪ {i ∈ anchorIdxs : 0 < i < n−1}
  anchors ← trier(set par ordre croissant)

  result ← copie(base)

  // Correction linéaire entre chaque paire d'ancres consécutives
  POUR k DE 0 À longueur(anchors) − 2:
    a ← anchors[k]
    b ← anchors[k+1]
    SI b − a < 2: CONTINUER

    corrA ← raw[a] − base[a]
    corrB ← raw[b] − base[b]

    POUR i DE a À b:
      t ← (i − a) / (b − a)
      result[i] ← base[i] + corrA × (1 − t) + corrB × t

  RETOURNER result
```

### Complexité

- Temps : **O(n + m)** où m = nombre d'ancres
- Espace : **O(n)**

### Propriétés mathématiques

- **Exactitude aux ancres** : `result[a] = base[a] + corrA = raw[a]` par construction.
- **Continuité** : entre deux ancres, la correction est une interpolation linéaire, donc continue.
- **Lissage préservé** : la courbe reste lisse à l'intérieur de chaque intervalle (la correction est linéaire, donc n'introduit pas d'oscillation).

### Notes

- Le choix des ancres est crucial. Si elles sont placées à chaque changement de catégorie, la courbe est très contrainte. Si elles sont placées uniquement sur les sommets/vallées détectés, la courbe reste souple.
- **Effet visuel** : la pente devient exactement nulle au sommet d'une ancre, ce qui résout le problème « la pente continue de monter après le sommet ».

---

## 4. Pente instantanée (différence centrée)

### Objet

Calculer la pente locale à chaque point de la trace, en % (variation d'altitude / distance parcourue).

### Formule

Pour le point `i`, on utilise les points voisins `i−1` et `i+1` :

```
pente[i] = (ele[i+1] − ele[i−1]) / (dist[i+1] − dist[i−1]) × 100
```

### Pseudocode

```
FONCTION computeSlope(ele, dist, minD = 0.05):
  n ← longueur(ele)
  s ← tableau de n zéros

  POUR i DE 0 À n−1:
    i1 ← max(0, i − 1)
    i2 ← min(n − 1, i + 1)
    d ← dist[i2] − dist[i1]
    SI d > minD:
      s[i] ← (ele[i2] − ele[i1]) / d × 100

  RETOURNER s
```

### Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### Notes

- `minD = 0.05 m` évite les divisions par zéro quand deux points sont trop proches.
- **Différence centrée** plutôt que différence avant (`i` → `i+1`) : réduit le décalage de phase.
- Sur une trace à 5 m de pas, cette pente est calculée sur **10 m**. Elle est donc **très sensible au bruit GPS** — c'est pourquoi elle est lissée avant affichage.

---

## 5. Pente par fenêtre glissante (Slope Window)

### Objet

Calculer la pente **moyenne sur une portion de N mètres** centrée sur chaque point. Utilisée pour la catégorisation par couleur, afin d'éviter que les raidillons éphémères ne génèrent des taches de couleur.

### Principe

Pour chaque point `i`, on cherche les bornes `[lo, hi]` telles que :
- `dist[i] − dist[lo] ≤ winM`
- `dist[hi] − dist[i] ≤ winM`

Puis on calcule :

```
pente[i] = (ele[hi] − ele[lo]) / (dist[hi] − dist[lo]) × 100
```

La fenêtre effective fait donc entre `winM` et `2 × winM` de large (selon où tombent les points).

### Pseudocode

```
FONCTION computeSlopeWindow(ele, dist, winM):
  n ← longueur(ele)
  s ← tableau de n zéros

  POUR i DE 0 À n−1:
    // Étendre vers la gauche
    lo ← i
    TANT QUE lo > 0 ET dist[i] − dist[lo−1] ≤ winM:
      lo ← lo − 1

    // Étendre vers la droite
    hi ← i
    TANT QUE hi < n−1 ET dist[hi+1] − dist[i] ≤ winM:
      hi ← hi + 1

    // Pente sur la fenêtre
    dDist ← dist[hi] − dist[lo]
    SI dDist > 0.5:
      s[i] ← (ele[hi] − ele[lo]) / dDist × 100

  RETOURNER s
```

### Complexité

- Temps : **O(n × w)** où w = nombre moyen de points dans la fenêtre. Avec un pas de 5 m et winM = 500 m, w ≈ 100, donc **O(100n)** en pratique.
- Optimisation possible : utilisation de pointeurs glissants → **O(n)** amorti.

### Notes

- **Avantage** : robuste au bruit GPS. Une pente instantanée de +15 % sur 10 m devient une pente moyenne de +4 % sur 500 m.
- **Inconvénient** : introduit un retard (décalage temporel) dans la détection des transitions.
- Le paramètre `winM` contrôle le niveau de lissage :
  - 100 m : presque la pente instantanée
  - 500 m : équilibre (permet de distinguer une vraie montée d'un raidillon)
  - 1000 m+ : vue très macro

---

## 6. Détection par hystérésis

### Objet

Identifier les portions de la trace où la pente dépasse un seuil (montées) avec **hystérésis** pour éviter les oscillations autour du seuil.

### Principe

- On entre en mode « montée » quand `slope[i] > +seuil`
- On sort du mode « montée » quand `slope[i] < −seuil`

L'utilisation d'un seuil négatif pour la sortie (au lieu de 0) empêche les micro-oscillations de créer des dizaines de micro-segments.

### Pseudocode

```
FONCTION detectEvents(slope, seuil):
  events ← liste vide
  inClimb ← FAUX

  POUR i DE 0 À n−1:
    SI NON inClimb ET slope[i] > seuil:
      events.ajouter({idx: i, type: 'start'})
      inClimb ← VRAI
    SINON SI inClimb ET slope[i] < −seuil:
      events.ajouter({idx: i, type: 'summit'})
      inClimb ← FAUX

  // Si on termine en montée, fermer le dernier événement
  SI inClimb:
    events.ajouter({idx: n−1, type: 'summit'})

  RETOURNER events
```

### Complexité

- Temps : **O(n)**
- Espace : **O(k)** où k = nombre d'événements

### Notes

- **Seuil typique** : 0.5 %. Plus le seuil est bas, plus la détection est sensible.
- **Effet de l'hystérésis** : un seuil de 0.5 % en entrée et −0.5 % en sortie signifie qu'on ignore les oscillations de ±0.5 %.

---

## 7. Construction des candidats de montée

### Objet

Transformer la liste d'événements `{start, summit}` en une liste de segments candidats (type `climb` ou `non-climb`).

### Pseudocode

```
FONCTION buildClimbCandidates(events, n):
  segs ← liste vide
  prev ← 0

  // Si le premier événement est un sommet, on commence en 'climb'
  SI longueur(events) > 0 ET events[0].type = 'summit':
    cur ← 'climb'
  SINON:
    cur ← 'non-climb'

  POUR chaque e DANS events:
    SI e.idx > prev:
      segs.ajouter({startIdx: prev, endIdx: e.idx, key: cur})

    cur ← (e.type = 'start') ? 'climb' : 'non-climb'
    prev ← e.idx

  // Segment final si nécessaire
  SI prev < n − 1:
    segs.ajouter({startIdx: prev, endIdx: n−1, key: cur})

  // Filtrer : ne garder que les montées
  RETOURNER filtrer(segs, s → s.key = 'climb')
```

### Complexité

- Temps : **O(k)** où k = nombre d'événements
- Espace : **O(k)**

### Notes

- Les segments retournés sont **bruts** : ils indiquent où la pente a dépassé le seuil, mais leurs bornes sont imprécises (décalées vers le bas de la montée).
- Le raffinement (§8, §9, §17) s'occupe de corriger ces bornes.

---

## 8. Recherche du sous-segment optimal

### Objet

Dans un candidat `[startIdx, endIdx]`, trouver le **plus long sous-segment** qui satisfait les critères (longueur, pente moyenne). C'est l'algorithme de validation.

### Principe

On utilise un **cumul de D+** pour évaluer en O(1) la pente moyenne entre deux indices quelconques, puis on énumère les paires (début, fin) de manière optimisée.

### Formule

Pour un sous-segment `[si, ei]` :
- Longueur : `L = dist[ei] − dist[si]`
- D+ : `DP = cumDPlus[ei] − cumDPlus[si]`
- Pente moyenne : `p = (DP / L) × 100`

### Critères de validation

```
seuil(L) = {
  3   si L ≥ 1000 m
  4   si 750 ≤ L < 1000
  5   si 500 ≤ L < 750
  6   si 400 ≤ L < 500
  7   si 300 ≤ L < 400
  ∞   sinon (rejeté)
}
```

### Pseudocode

```
FONCTION findBestSubClimb(dist, ele, startIdx, endIdx):
  n ← endIdx − startIdx + 1
  SI n < 2: RETOURNER null

  // Cumul de D+ (dénivelé positif uniquement)
  cumDPlus[0] ← 0
  POUR k DE 1 À n−1:
    d ← ele[startIdx + k] − ele[startIdx + k−1]
    cumDPlus[k] ← cumDPlus[k−1] + max(0, d)

  // Sous-échantillonnage pour limiter la complexité
  step ← (n > 800) ? ⌈n / 800⌉ : 1

  best ← null

  POUR si DE 0 À n−2 PAR step:
    POUR ei DE si+1 À n−1 PAR step:
      L ← dist[startIdx + ei] − dist[startIdx + si]
      SI L < 300: CONTINUER
      SI best ≠ null ET L ≤ best.length: CONTINUER

      DP ← cumDPlus[ei] − cumDPlus[si]
      p  ← (DP / L) × 100

      SI p ≥ seuil(L):
        best ← {startIdx: startIdx + si, endIdx: startIdx + ei,
                length: L, dPlus: DP, avgSlope: p}

  RETOURNER best
```

### Complexité

- Sans sous-échantillonnage : **O(n²)** où n = nombre de points dans le candidat
- Avec sous-échantillonnage (step adaptatif) : **O(800²) ≈ 640 000** opérations max par candidat, soit **O(1)** en pratique.
- Espace : **O(n)** pour le tableau cumulatif

### Notes

- La stratégie privilégie le **plus long** sous-segment valide ; en cas d'égalité de longueur, elle garde le premier trouvé.
- Cette fonction est appelée **deux fois** par candidat : une fois sur `eleBase` (validation intermédiaire) et une fois sur `eleDisplay` (validation finale).
- Si aucun sous-segment valide n'existe, la fonction retourne `null` et la montée est rejetée.

---

## 9. Argmin / Argmax sur intervalle

### Objet

Trouver l'indice du **minimum** (resp. maximum) d'un tableau numérique dans un intervalle `[lo, hi]`. Utilisé pour raffiner les bornes d'une montée.

### Pseudocode

```
FONCTION argminRange(arr, lo, hi):
  best ← lo
  v ← arr[lo]
  POUR i DE lo+1 À hi:
    SI arr[i] < v:
      v ← arr[i]
      best ← i
  RETOURNER best

FONCTION argmaxRange(arr, lo, hi):
  best ← lo
  v ← arr[lo]
  POUR i DE lo+1 À hi:
    SI arr[i] > v:
      v ← arr[i]
      best ← i
  RETOURNER best
```

### Complexité

- Temps : **O(hi − lo + 1)** (linéaire sur l'intervalle)
- Espace : **O(1)**

### Notes

- Sur un intervalle de 5000 m à pas de 5 m (1000 points), chaque appel coûte ~1000 itérations.
- Appelé typiquement 2 fois par montée : une fois pour la vallée, une fois pour le sommet.

---

## 10. Recherche binaire sur distance

### Objet

Trouver l'indice du **premier point** dont la distance cumulée est ≥ une valeur cible. Utilisé pour convertir un seuil en mètres en indices de tableau.

### Pseudocode

```
FONCTION findIdxAtDist(dist, targetM):
  lo ← 0
  hi ← longueur(dist) − 1

  TANT QUE lo < hi:
    mid ← (lo + hi) / 2  (division entière)
    SI dist[mid] < targetM:
      lo ← mid + 1
    SINON:
      hi ← mid

  RETOURNER lo
```

### Complexité

- Temps : **O(log n)**
- Espace : **O(1)**

### Précondition

Le tableau `dist` doit être **strictement croissant** (garanti par la construction cumulative).

### Notes

- Retourne l'indice du premier point avec `dist[i] ≥ targetM`.
- Si `targetM > dist[n−1]`, retourne `n−1`.

---

## 11. Catégorisation par pente

### Objet

Associer une **catégorie de pente** à une valeur de pente en %. Cette catégorie détermine la couleur du segment.

### Grille (8 catégories)

| ID | Libellé | Plage |
|---|---|---|
| dh3 | Descente raide | p < −8 |
| dh2 | Descente | −8 ≤ p < −3 |
| dh1 | Faux plat descendant | −3 ≤ p < −1 |
| pl | Plat | −1 ≤ p ≤ +1 |
| up1 | Faux plat montant | +1 < p ≤ +3 |
| up2 | Montée | +3 < p ≤ +7 |
| up3 | Montée raide | +7 < p ≤ +10 |
| up4 | Montée très raide | p > +10 |

### Pseudocode

```
FONCTION categorize(slope):
  POUR chaque cat DANS CATEGORIES (dans l'ordre):
    SI slope ≥ cat.min ET slope < cat.max:
      RETOURNER cat.id
  RETOURNER 'pl'  // fallback
```

### Complexité

- Temps : **O(c)** où c = nombre de catégories (8). En pratique constant → **O(1)**.
- Espace : **O(1)**

### Notes

- Les bornes utilisent `<` en borne haute : une pente de +3 % exactement tombe dans `up1` (faux plat montant), pas dans `up2` (montée).
- La symétrie descente/montée n'est pas parfaite : la descente raide commence à −8 %, mais la montée raide commence à +7 %. C'est un choix volontaire pour surligner les montées.

---

## 12. Fusion des segments courts

### Objet

Supprimer les segments dont la longueur est inférieure à un seuil (par défaut **250 m** depuis v2.1) en les absorbant dans leur **voisin le plus long**. Puis combiner les segments adjacents de même catégorie.

### Algorithme

Deux passes successives :

1. **Fusion itérative** : tant qu'il existe un segment trop court, l'absorber dans son voisin le plus long.
2. **Combinaison** : recoller les segments adjacents de même catégorie.

### Pseudocode

```
FONCTION mergeShortSegments(raw, dist, minLen):
  // --- Passe 1 : fusion itérative ---
  changed ← VRAI
  TANT QUE changed ET longueur(raw) > 1:
    changed ← FAUX
    POUR i DE 0 À longueur(raw) − 1:
      s ← raw[i]
      len ← dist[s.endIdx] − dist[s.startIdx]
      SI len ≥ minLen: CONTINUER

      // Longueurs des voisins
      leftLen  ← (i > 0) ? dist[raw[i−1].endIdx] − dist[raw[i−1].startIdx] : −1
      rightLen ← (i < n−1) ? dist[raw[i+1].endIdx] − dist[raw[i+1].startIdx] : −1

      // Absorption par le voisin le plus long
      SI rightLen ≥ leftLen:
        raw[i+1].startIdx ← s.startIdx   // le voisin droit absorbe
      SINON:
        raw[i−1].endIdx ← s.endIdx       // le voisin gauche absorbe

      raw.supprimer(i)
      changed ← VRAI
      SORTIR DE LA BOUCLE POUR

  // --- Passe 2 : combinaison des adjacents de même catégorie ---
  SI longueur(raw) > 1:
    res ← [raw[0]]
    POUR i DE 1 À longueur(raw) − 1:
      last ← res[dernier]
      SI last.key = raw[i].key:
        last.endIdx ← raw[i].endIdx
      SINON:
        res.ajouter(raw[i])
    RETOURNER res

  RETOURNER raw
```

### Complexité

- Temps : **O(s²)** dans le pire cas (chaque itération scanne toute la liste), mais **O(s)** en pratique car les segments courts sont rares.
- Espace : **O(s)** où s = nombre de segments

### Notes

- **Critère de choix du voisin** : le plus long est absorbé en priorité pour minimiser la distorsion.
- **Absorption silencieuse** : le segment court disparaît, sa longueur est intégrée au voisin.
- Sans cet algorithme, une trace typique produirait des centaines de micro-segments (< 50 m), illisibles sur la bande de segmentation.
- **Valeur par défaut v2.1+** : `longueurMin = 250 m` (au lieu de 100 m en v1). Les micro-segments entre 100 et 250 m sont donc désormais absorbés.

---

## 13. Segmentation par catégorie (segments contigus)

### Objet

Découper la trace en segments contigus selon la catégorie de pente de chaque point, **sans laisser de zone blanche** entre segments.

### Principe clé

Le **point frontière** (là où la catégorie change) appartient à la fois au segment précédent et au segment suivant. Cela garantit que les segments se touchent visuellement.

### Pseudocode

```
FONCTION segmentByCategory(slopeForCat, dist, ele, minLen):
  // 1. Catégoriser chaque point
  cat ← tableau de mêmes dimensions que slopeForCat
  POUR i DE 0 À n−1:
    cat[i] ← categorize(slopeForCat[i])

  // 2. Découper en segments contigus
  raw ← liste vide
  start ← 0

  POUR i DE 1 À n:
    SI i = n OU cat[i] ≠ cat[start]:
      // Le point frontière appartient aux deux segments
      endIdx ← (i = n) ? i − 1 : i
      raw.ajouter({startIdx: start, endIdx: endIdx, key: cat[start]})
      start ← i

  // 3. Fusionner les segments trop courts
  RETOURNER mergeShortSegments(raw, dist, minLen)
    .map(s → enrichSegment(s, dist, ele, slopeForCat))
```

### Détail crucial

**Avant** (avec zone blanche) :
```javascript
raw.push({ startIdx: start, endIdx: i − 1, key: cat[start] });
```
Résultat : segments `[0, 2]`, `[3, 5]` — trou entre l'indice 2 et 3.

**Après** (contigus) :
```javascript
const endIdx = (i === n) ? i − 1 : i;
raw.push({ startIdx: start, endIdx, key: cat[start] });
```
Résultat : segments `[0, 3]`, `[3, 5]` — contigus.

### Complexité

- Temps : **O(n)** pour la catégorisation + **O(s²)** pour la fusion
- Espace : **O(n)**

### Notes

- La somme des longueurs des segments après fusion **égale exactement la distance totale** (aucune perte).
- Le point frontière partagé n'introduit pas de double comptage : la longueur `dist[endIdx] − dist[startIdx]` reste correcte car `endIdx` fait partie du segment.

---

## 14. Enrichissement des segments

### Objet

Calculer pour chaque segment les métriques dérivées : longueur, D+, pente moyenne pondérée.

### Pseudocode

```
FONCTION enrichSegment(s, dist, ele, slope):
  startDist ← dist[s.startIdx]
  endDist   ← dist[s.endIdx]
  length    ← endDist − startDist

  // D+ : somme des variations positives uniquement
  dPlus ← 0
  POUR k DE s.startIdx+1 À s.endIdx:
    d ← ele[k] − ele[k−1]
    SI d > 0: dPlus ← dPlus + d

  // Pente moyenne pondérée par la distance
  weighted ← 0
  POUR k DE s.startIdx+1 À s.endIdx:
    weighted ← weighted + slope[k] × (dist[k] − dist[k−1])

  avgSlope ← (length > 0) ? (weighted / length) : 0

  RETOURNER {
    ...s,
    startDist, endDist, length, dPlus, avgSlope,
    altStart: ele[s.startIdx],
    altEnd:   ele[s.endIdx]
  }
```

### Complexité

- Temps : **O(len)** où len = nombre de points dans le segment
- Espace : **O(1)** par segment

### Notes

- Le **D+** est calculé sur l'altitude brute ou lissée selon le contexte (lissée pour l'affichage).
- La **pente moyenne** est pondérée par la distance, pas par le nombre de points. Cela évite qu'un segment à pas serré (dense en points) ne soit sur-représenté.

---

## 15. Contrainte anti-chevauchement

### Objet

Garantir que les montées éditées manuellement ne se chevauchent jamais. Appliqué avant tout calcul de reconstruction.

### Pseudocode

```
FONCTION appliquerContraintes(userClimbs):
  // 1. Trier par startKm croissant
  sorted ← trier(userClimbs, (a, b) → a.startKm − b.startKm)

  previousEndKm ← −∞

  POUR chaque c DANS sorted:
    c._constrainedStart ← FAUX
    c._constrainedEnd ← FAUX

    // Contrainte 1 : début ≥ fin de la précédente + 1 m
    SI c.startKm < previousEndKm:
      c._constrainedStart ← VRAI
      c.startKm ← previousEndKm + 0.001

    // Contrainte 2 : fin > début (au moins 100 m)
    SI c.endKm ≤ c.startKm:
      c._constrainedEnd ← VRAI
      c.endKm ← c.startKm + 0.1

    previousEndKm ← c.endKm

  RETOURNER sorted
```

### Complexité

- Temps : **O(m log m)** pour le tri + **O(m)** pour la contrainte, où m = nombre de montées
- Espace : **O(m)**

### Notes

- Le **+0.001 km (1 m)** évite les chevauchements exacts (deux montées qui se touchent au même point).
- Le **+0.1 km (100 m)** pour `endKm` évite les segments de longueur nulle ou négative.
- Les flags `_constrainedStart` et `_constrainedEnd` sont utilisés par l'UI pour afficher l'indicateur ⚠ et le fond orange.

### Application temps réel pendant le drag (v0.5+)

Lors du drag direct d'une poignée (voir §19), une contrainte **équivalente** est appliquée à chaque `mousemove`, mais uniquement sur la montée en cours d'édition :

```
FONCTION applyDragConstraint(handle, km, climb):
  totalKm ← dist[fin] / 1000
  minLen  ← 0.05    // 50 m minimum
  minGap  ← 0.001   // 1 m entre montées

  sorted ← trier(STATE.userClimbs par startKm)
  pos ← index de climb dans sorted
  prevClimb ← (pos > 0) ? sorted[pos−1] : null
  nextClimb ← (pos < |sorted|−1) ? sorted[pos+1] : null

  lo ← 0
  hi ← totalKm
  SI prevClimb: lo ← prevClimb.endKm + minGap
  SI nextClimb: hi ← nextClimb.startKm − minGap

  SI handle = 'valley':
    RETOURNER clamp(km, lo, climb.endKm − minLen)
  SINON:
    RETOURNER clamp(km, climb.startKm + minLen, hi)
```

---

## 16. Sous-échantillonnage

### Objet

Réduire le nombre de points affichés dans les graphiques Chart.js pour garantir la performance (limite : ~4 000 points par graphique).

### Algorithme

Sélection uniforme d'indices, en veillant à toujours inclure le premier et le dernier point.

### Pseudocode

```
FONCTION subSample(xKm, maxPoints = 4000):
  n ← longueur(xKm)
  SI n ≤ maxPoints: RETOURNER tous les indices

  step ← ⌈n / maxPoints⌉
  idx ← liste vide

  POUR i DE 0 À n−1 PAR step:
    idx.ajouter(i)

  // Toujours inclure le dernier point
  SI idx[dernier] ≠ n−1:
    idx.ajouter(n−1)

  RETOURNER idx
```

### Complexité

- Temps : **O(n)**
- Espace : **O(maxPoints)**

### Notes

- Le sous-échantillonnage **n'affecte pas** les calculs sous-jacents (distance, pente, D+ sont calculés sur tous les points). Il ne s'applique qu'à l'affichage.
- Les zooms successifs permettent de retrouver la finesse sur une portion donnée (Chart.js recalcule le rendu à partir des données brutes envoyées).

---

## 17. Garde-fou hybride de refinage (v2)

### Objet

Limiter la distance maximale de recherche de la vraie vallée / du vrai sommet lors de l'étape de raffinement. **Remplace** l'ancien paramètre `maxRefineM` en mètres fixes par un calcul **adaptatif par montée**.

### Principe

Pour chaque candidat détecté de longueur `candidate.length` (en mètres), on calcule :

```
effectiveRefine = clamp(
  refinePct% × candidate.length,
  REFINE_MIN_M,
  REFINE_MAX_M
)
```

avec :
- `refinePct` : paramètre **exposé à l'utilisateur** (mode avancé), défaut **60 %**
- `REFINE_MIN_M = 1500 m` : **constante interne**
- `REFINE_MAX_M = 8000 m` : **constante interne**

### Pseudocode

```
CONSTANTES:
  REFINE_MIN_M ← 1500
  REFINE_MAX_M ← 8000
  REFINE_PCT_DEFAULT ← 60

FONCTION computeEffectiveRefine(candidateLen, refinePct):
  pctValue ← refinePct × candidateLen / 100
  RETOURNER clamp(pctValue, REFINE_MIN_M, REFINE_MAX_M)
```

### Application dans le pipeline

```
POUR chaque candidate DANS candidatesWithSub:
  candidateLen ← candidate.length
  effectiveRefine ← computeEffectiveRefine(candidateLen, refinePct)

  // Recherche du vrai sommet dans [endIdx, endIdx + effectiveRefine]
  hiIdx ← findIdxAtDist(dist, min(dist[n−1], dist[c.endIdx] + effectiveRefine))
  newEnd ← argmaxRange(eleBase, c.endIdx, hiIdx)

  // Recherche de la vraie vallée dans [loIdx, startIdx]
  safetyLow ← findIdxAtDist(dist, max(0, dist[c.startIdx] − effectiveRefine))
  loIdx ← max(lastEnd + 1, safetyLow)
  newStart ← argminRange(eleBase, loIdx, c.startIdx)
```

### Complexité

- Temps : **O(1)** pour le clamp, intégré dans le raffinement global
- Espace : **O(1)**

### Notes

- **Adaptatif** : les longues montées ont plus de latitude pour chercher leur vrai sommet/creux.
- **Borné** : les montées courtes ne partent pas chercher un sommet à 10 km ; les très longues ne dévorent pas leurs voisines.
- **Prévisible** : les bornes min/max garantissent une plage raisonnable.
- **Transparence UI** : la valeur effective est affichée par montée dans la colonne « Refine » du tableau, avec un tooltip détaillant le calcul `pct% × longueur = pré-clamp → clamp → valeur finale`.

### Table d'exemples

| Longueur candidat | 60 % | Après clamp [1500, 8000] |
|---|---|---|
| 300 m | 180 m | **1500 m** |
| 1 km | 600 m | **1500 m** |
| 3 km | 1800 m | **1800 m** |
| 6 km | 3600 m | **3600 m** |
| 10 km | 6000 m | **6000 m** |
| 20 km | 12000 m | **8000 m** |

---

## 18. Insertion contextuelle de montée (v2.2)

### Objet

Permettre à l'utilisateur d'insérer une nouvelle montée **entre deux montées existantes**, ou **avant la première**, ou **après la dernière**, via des boutons intégrés directement dans le tableau. Remplace le bouton global « + Ajouter une montée » du toolbar.

### Règles d'affichage

| Position | Condition d'affichage | Constante |
|---|---|---|
| **Avant #1** | `climbs[0].startDist ≥ 1 km` | `INSERT_EDGE_MIN_KM = 1.0` |
| **Entre #i et #i+1** | `climbs[i+1].startDist − climbs[i].endDist ≥ 0.2 km` | `INSERT_BETWEEN_MIN_KM = 0.2` |
| **Après #N** | `totalDist − climbs[N−1].endDist ≥ 1 km` | `INSERT_EDGE_MIN_KM = 1.0` |
| **Aucune montée** | Toujours affiché | — |

### Algorithme d'insertion dans un gap

```
CONSTANTES:
  INSERT_DEFAULT_LEN_KM ← 1.0   // longueur cible

FONCTION insertInGap(gapStartKm, gapEndKm):
  gapLen ← gapEndKm − gapStartKm
  SI gapLen < INSERT_BETWEEN_MIN_KM: REJETER

  // Longueur cible : min(1 km, 80 % du gap), minimum 100 m
  targetLen ← max(0.1, min(INSERT_DEFAULT_LEN_KM, gapLen × 0.8))

  center ← (gapStartKm + gapEndKm) / 2
  start  ← max(gapStartKm + 0.01, center − targetLen / 2)
  end    ← min(gapEndKm − 0.01,   center + targetLen / 2)

  SI end ≤ start: REJETER

  // Créer la montée (sans garde-fou automatique)
  userClimbs.ajouter({
    startKm: start,
    endKm: end,
    autoStartKm: null,
    autoEndKm: null,
    refineM: null,
    candidateLen: null
  })

  // Sélectionner et scroller vers la nouvelle ligne
  selectedClimbIdx ← |userClimbs| − 1
  scrollIntoView(ligne)
```

### Cas particulier : aucune montée existante

```
FONCTION insertEmpty(totalKm):
  targetLen ← min(INSERT_DEFAULT_LEN_KM, max(0.05, totalKm × 0.5))
  center ← totalKm / 2
  start  ← max(0, center − targetLen / 2)
  end    ← min(totalKm, center + targetLen / 2)
  RETOURNER {start, end}
```

### Complexité

- Temps : **O(m)** pour le calcul du gap et l'insertion (m = nombre de montées)
- Espace : **O(1)** par insertion

### Notes

- La montée insérée **n'a pas de `refineM`** → colonne Refine affiche `—` en gris italique.
- Les boutons d'insertion apparaissent **entre les lignes** du tableau, comme des rangées `<tr>` dédiées.
- Un clic sur un bouton d'insertion **ne déclenche pas** la sélection de la ligne (via `stopPropagation()`).

---

## 19. Édition directe par drag & drop (v0.5)

### Objet

Permettre à l'utilisateur de déplacer **directement sur le graphique** les marqueurs de vallée et de sommet d'une montée sélectionnée, avec retour visuel temps réel et contraintes appliquées en direct.

### Architecture de l'interception

Point clé : `chartjs-plugin-zoom` attache ses listeners sur le `canvas` en phase cible. Pour intercepter le `mousedown` **avant** que le plugin zoom ne démarre le pan, il faut écouter **au niveau `window` en phase de capture**.

```
Phase de propagation DOM :
1. Capture  : window → ... → canvas   ← notre listener (intercepte)
2. Cible    : canvas                   ← Chart.js + plugin zoom
3. Bullage  : canvas → ... → window
```

Si l'événement touche une poignée :
- `e.stopImmediatePropagation()` empêche Chart.js de le voir
- `e.preventDefault()` évite la sélection de texte

### Détection de poignée

```
FONCTION findHandleAt(chart, px, py, climb):
  // Bande horizontale au-dessus du chartArea (là où sont dessinés les triangles)
  SI py < chartArea.top − 12 OU py > chartArea.top + 34: RETOURNER null

  xV ← scales.x.getPixelForValue(climb.startKm)
  xS ← scale.x.getPixelForValue(climb.endKm)
  threshold ← 14 px

  dV ← |px − xV|
  dS ← |px − xS|

  SI dV < threshold ET dV ≤ dS: RETOURNER 'valley'
  SI dS < threshold: RETOURNER 'summit'
  RETOURNER null
```

### Cycle de drag

```
FONCTION startHandleDrag(chart, handle, climb):
  draggingHandle ← handle
  dragPreview ← { handle, km: (handle='valley') ? climb.startKm : climb.endKm }

  // Désactivation explicite du pan et du tooltip
  panOriginalState ← { elev: pan.elev.enabled, slope: pan.slope.enabled }
  elevChart.pan.enabled ← FAUX
  slopeChart.pan.enabled ← FAUX
  elevChart.tooltip.enabled ← FAUX   // masque le tooltip pendant le drag
  slopeChart.tooltip.enabled ← FAUX

  SUR pointermove (capture) :
    cx, cy ← coordonnées souris/tactile
    px ← cx − rect.left
    km ← scales.x.getValueForPixel(px)

    constrainedKm ← applyDragConstraint(handle, km, climb)   // §15
    SI constrainedKm = lastKm: IGNORER (dedup)

    dragPreview ← { handle, km: constrainedKm }
    updatePreviewMetrics(handle, constrainedKm, climb)   // maj tableau
    chart.draw()   // redessine le canvas (bande jaune + poignée)

  SUR pointerup :
    commitDrag(handle, dragPreview.km, climb)   // écrit la valeur finale
    renderAll()                                  // rebuild complet

    // Restauration
    pan.enabled ← panOriginalState.*
    tooltip.enabled ← VRAI
    cursor ← 'default'
```

### Mise à jour des métriques en temps réel

```
FONCTION updatePreviewMetrics(handle, km, climb):
  idx ← index de climb dans userClimbs
  row ← DOM.querySelector(tr[data-idx=idx])
  SI row absent: RETOURNER

  // Met à jour l'input km concerné
  input ← row.querySelector(input[data-field=(handle='valley' ? 'startKm' : 'endKm')])
  input.value ← km.toFixed(3)

  // Recalcule les métriques
  startKm ← (handle='valley') ? km : climb.startKm
  endKm   ← (handle='summit') ? km : climb.endKm

  startIdx ← findIdxAtDist(dist, startKm × 1000)
  endIdx   ← findIdxAtDist(dist, endKm × 1000)

  SI startIdx ≥ endIdx:
    cellLength.textContent ← '—'
    cellDPlus.textContent ← '—'
    cellSlope.innerHTML ← '—'
    RETOURNER

  length ← dist[endIdx] − dist[startIdx]
  dPlus ← Σ max(0, ele[k] − ele[k−1])
  avgSlope ← (dPlus / length) × 100

  cellLength.textContent ← length.toFixed(0)
  cellDPlus.innerHTML ← `<strong>${dPlus.toFixed(0)}</strong>`
  cellSlope.innerHTML ← badge(avgSlope)
```

### Complexité

- Interception `mousedown` : **O(1)**
- Chaque `mousemove` : **O(1)** pour la position + **O(1)** pour `findHandleAt` + **O(range)** pour `updatePreviewMetrics` (borné par la longueur de la montée en cours)
- `chart.draw()` : **O(points affichés)** ≈ 4000
- En pratique : ~5–10 ms par frame, largement sous les 16 ms d'une frame à 60 fps

### Notes

- **Aucun rebuild complet** pendant le drag : seuls le canvas et la ligne du tableau sont mis à jour.
- **Rebuild complet** uniquement au `mouseup` (via `commitDrag`).
- Le tooltip est **désactivé pendant le drag** (v2.1) pour ne pas masquer les courbes significatives.
- Le pan et le zoom sont **désactivés pendant le drag** pour éviter tout conflit.

---

## 20. Pipeline complet de détection

### Objet

Détecter automatiquement les montées d'une trace GPX.

### Pseudocode

```
FONCTION computeAuto(rawPoints, params):
  n ← longueur(rawPoints)

  // === ÉTAPE 1 : Préparation ===
  dist ← calculerDistances(rawPoints)
  eleRaw ← [rawPoints[i].ele pour i dans 0..n−1]

  // === ÉTAPE 2 : Détection initiale ===
  eleBase   ← movingAverage(eleRaw, params.fenetreAffichage)
  slopeBase ← movingAverage(computeSlope(eleBase, dist), params.fenetrePente)
  events    ← detectEvents(slopeBase, params.seuilMontee)
  candidates ← buildClimbCandidates(events, n)

  // === ÉTAPE 3 : Sous-segment optimal sur eleBase ===
  candidatesWithSub ← liste vide
  POUR chaque c DANS candidates:
    sub ← findBestSubClimb(dist, eleBase, c.startIdx, c.endIdx)
    SI sub ≠ null:
      candidatesWithSub.ajouter(sub)

  // === ÉTAPE 4 : Raffinement des bornes (v2 : garde-fou hybride) ===
  anchors ← liste vide
  refined ← liste vide
  lastEnd ← −1

  POUR chaque c DANS candidatesWithSub:
    candidateLen ← c.length
    effectiveRefine ← computeEffectiveRefine(candidateLen, params.refinePct)   // §17

    hiIdx ← findIdxAtDist(dist, min(dist[n−1], dist[c.endIdx] + effectiveRefine))
    newEnd ← (hiIdx > c.endIdx) ? argmaxRange(eleBase, c.endIdx, hiIdx) : c.endIdx
    SI newEnd ≤ c.startIdx: CONTINUER
    SI newEnd ≤ lastEnd: CONTINUER

    naturalLow ← lastEnd + 1
    safetyLow  ← findIdxAtDist(dist, max(0, dist[c.startIdx] − effectiveRefine))
    loIdx ← max(naturalLow, safetyLow)
    newStart ← (loIdx < c.startIdx) ? argminRange(eleBase, loIdx, c.startIdx) : c.startIdx
    SI newStart ≥ newEnd: CONTINUER

    anchors.ajouter(newStart, newEnd)
    refined.ajouter({startIdx, endIdx, refineM: effectiveRefine, candidateLen})
    lastEnd ← newEnd

  // === ÉTAPE 5 : Lissage ancré ===
  eleDisplay ← anchoredMovingAverage(eleRaw, anchors, params.fenetreAffichage)
  slopeSemi  ← computeSlope(eleDisplay, dist)
  slopeDisplay ← movingAverage(slopeSemi, params.fenetrePente)

  // === ÉTAPE 6 : Re-validation sur eleDisplay ===
  autoClimbs ← liste vide
  POUR chaque c DANS refined:
    sub ← findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx)
    SI sub = null: CONTINUER
    SI sub.length < 300: CONTINUER
    SI sub.avgSlope < seuil(sub.length): CONTINUER

    autoClimbs.ajouter({
      startKm: dist[c.startIdx] / 1000,
      endKm:   dist[c.endIdx] / 1000,
      refineM: c.refineM,
      candidateLen: c.candidateLen
    })

  RETOURNER { dist, eleRaw, autoClimbs }
```

### Complexité globale

Soit n = nombre de points, m = nombre de candidats.

| Étape | Complexité |
|---|---|
| Distances | O(n) |
| Lissages | O(n) |
| Détection | O(n) |
| Sous-segments (× m) | O(m × 800²) amorti |
| Raffinement (× m) | O(m × w) avec w = points dans la fenêtre effective |
| Lissage ancré | O(n + m) |
| Re-validation | O(m × 800²) |
| **Total** | **O(n + m × 800²)** |

En pratique, m ≈ 10 et 800² ≈ 640 000, donc ~6 millions d'opérations. Sur un navigateur moderne, **< 1 seconde** pour n = 10 000 points.

---

## 21. Pipeline de reconstruction

### Objet

Recalculer entièrement l'état affiché à partir de `STATE.userClimbs` (état éditable par l'utilisateur). Appelé après chaque édition, insertion, suppression, ou changement de paramètre.

### Différences avec le pipeline de détection

| Aspect | Détection | Reconstruction |
|---|---|---|
| Entrée | `rawPoints` + params | `STATE.userClimbs` + params |
| Ancres | Calculées par argmin/argmax | Bornes utilisateur directement |
| Catégorisation | Sur `slopeDisplay` (instantanée) | Sur `slopeForCat` (fenêtrée) |
| Contraintes | Aucune | Anti-chevauchement obligatoire |

### Pseudocode

```
FONCTION rebuild():
  SI STATE.dist = null OU STATE.eleRaw = null: RETOURNER null

  params ← getParams()

  // === ÉTAPE 1 : Contraintes anti-chevauchement ===
  sorted ← appliquerContraintes(STATE.userClimbs)

  // === ÉTAPE 2 : Ancrage ===
  anchors ← liste vide
  resolved ← liste vide

  POUR chaque c DANS sorted:
    startIdx ← findIdxAtDist(dist, c.startKm × 1000)
    endIdx   ← findIdxAtDist(dist, c.endKm × 1000)

    SI startIdx ≥ endIdx:
      resolved.ajouter({startIdx, endIdx, ref: c, invalid: VRAI})
      CONTINUER

    anchors.ajouter(startIdx, endIdx)
    resolved.ajouter({startIdx, endIdx, ref: c, invalid: FAUX})

  // === ÉTAPE 3 : Lissages ===
  eleDisplay ← anchoredMovingAverage(eleRaw, anchors, params.fenetreAffichage)
  slopeRaw   ← computeSlope(movingAverage(eleRaw, 5), dist)
  slopeSemi  ← computeSlope(eleDisplay, dist)
  slopeDisplay ← movingAverage(slopeSemi, params.fenetrePente)

  // === ÉTAPE 4 : Pente de catégorisation (fenêtrée) ===
  slopeForCat ← computeSlopeWindow(eleDisplay, dist, params.fenetreCatM)

  // === ÉTAPE 5 : Métriques par montée ===
  climbs ← liste vide
  POUR chaque {startIdx, endIdx, ref, invalid} DANS resolved:
    SI invalid:
      climbs.ajouter({...invalid, length: 0, dPlus: 0, avgSlope: 0,
                      refineM: ref.refineM ?? null,
                      candidateLen: ref.candidateLen ?? null})
      CONTINUER

    dPlus ← 0
    POUR k DE startIdx+1 À endIdx:
      d ← eleDisplay[k] − eleDisplay[k−1]
      SI d > 0: dPlus ← dPlus + d

    length ← dist[endIdx] − dist[startIdx]
    avgSlope ← (length > 0) ? (dPlus / length) × 100 : 0

    climbs.ajouter({startIdx, endIdx, ref, invalid: FAUX,
                    startDist, endDist, length, dPlus, avgSlope,
                    refineM: ref.refineM ?? null,
                    candidateLen: ref.candidateLen ?? null})

  // === ÉTAPE 6 : Segmentation par catégorie ===
  catSegments ← segmentByCategory(slopeForCat, dist, eleDisplay, params.longueurMin)

  // === ÉTAPE 7 : Marqueurs ===
  markers ← liste vide
  POUR chaque c DANS climbs:
    SI c.invalid: CONTINUER
    markers.ajouter({dist: c.startDist, type: 'valley', climbIdx: index(c.ref)})
    markers.ajouter({dist: c.endDist,   type: 'summit', climbIdx: index(c.ref)})

  RETOURNER { dist, eleRaw, eleDisplay, slopeRaw, slopeDisplay,
              slopeForCat, catSegments, climbs, markers }
```

### Complexité

| Étape | Complexité |
|---|---|
| Contraintes | O(m log m) |
| Ancrage | O(m) + O(m log n) pour findIdxAtDist |
| Lissage ancré | O(n + m) |
| Pente fenêtrée | O(n × w) |
| Métriques | O(n) |
| Segmentation | O(n + s²) |
| **Total** | **O(n × w)** dominé par la pente fenêtrée |

Avec n = 10 000 et w = 100, ~1 million d'opérations → **< 200 ms**.

---

## 22. Tableau récapitulatif des complexités

| Algorithme | Temps | Espace | Fréquence d'appel |
|---|---|---|---|
| Haversine (par paire) | O(1) | O(1) | n−1 fois |
| Distance cumulée | O(n) | O(n) | 1 fois |
| Moving Average | O(n) | O(n) | 5–6 fois |
| Anchored Moving Average | O(n + m) | O(n) | 2 fois |
| Pente instantanée | O(n) | O(n) | 3 fois |
| Pente fenêtrée | O(n × w) | O(n) | 1 fois |
| Détection hystérésis | O(n) | O(k) | 1 fois |
| Construction candidats | O(k) | O(k) | 1 fois |
| Sous-segment optimal | O(800²) | O(n) | 2 × m fois |
| Argmin/Argmax | O(range) | O(1) | 2 × m fois |
| Recherche binaire | O(log n) | O(1) | 4 × m fois |
| Catégorisation | O(c) | O(1) | n fois |
| Fusion segments courts | O(s²) | O(s) | 1–2 fois |
| Segmentation contiguë | O(n) | O(s) | 1 fois |
| Enrichissement segment | O(len) | O(1) | s fois |
| **Garde-fou hybride (v2)** | **O(1)** | **O(1)** | **m fois** |
| **Insertion contextuelle (v2.2)** | **O(m)** | **O(1)** | **à la demande** |
| **Drag & drop (v0.5)** | **O(points affichés)** | **O(1)** | **par frame** |
| Anti-chevauchement | O(m log m) | O(m) | 1 fois |
| Sous-échantillonnage | O(n) | O(4000) | 1 fois |

Légende :
- n : nombre de points de la trace
- m : nombre de montées détectées
- k : nombre d'événements (transitions pente)
- s : nombre de segments de catégorie
- w : nombre moyen de points dans une fenêtre glissante

---

## 23. Choix de conception et alternatives écartées

### 23.1 Haversine vs Vincenty

| Critère | Haversine | Vincenty |
|---|---|---|
| Précision | ±0.5 % (sphérique) | ±0.001 % (ellipsoïdal) |
| Vitesse | Très rapide | 10× plus lent |
| Complexité | 5 lignes | 30+ lignes |
| **Retenu** | ✅ | ❌ |

### 23.2 Différence centrée vs différence avant

| Critère | Centrée | Avant |
|---|---|---|
| Décalage de phase | Aucun | +1 point |
| Lissage naturel | Oui (3 pts) | Non |
| **Retenu** | ✅ | ❌ |

### 23.3 Pente instantanée vs pente fenêtrée pour la catégorisation

| Critère | Instantanée | Fenêtrée |
|---|---|---|
| Stabilité visuelle | Faible (taches) | Élevée |
| Fidélité locale | Haute | Moyenne |
| Coût calcul | O(n) | O(n × w) |
| **Retenu courbe** | ✅ | ❌ |
| **Retenu couleur** | ❌ | ✅ |

### 23.4 Argmin/argmax vs marche avec tolérance

| Critère | Argmin/argmax | Marche tolérance |
|---|---|---|
| Robustesse au bruit | Élevée | Faible |
| Déterminisme | Toujours | Dépend de la tolérance |
| **Retenu** | ✅ | ❌ |

### 23.5 Garde-fou absolu vs pourcentage vs hybride (v2)

| Critère | Absolu (V1) | % pur | **Hybride (v2)** |
|---|---|---|---|
| Simplicité | ✅✅ | ✅ | ⚠️ |
| Adaptabilité à l'échelle | ❌ | ✅ | ✅✅ |
| Prévisibilité | ✅✅ | ⚠️ | ✅ |
| Robustesse cas extrêmes | ✅ | ❌ | ✅✅ |
| **Retenu** | ❌ | ❌ | ✅ |

Raison : le pourcentage pur dépend linéairement de la longueur de la montée, ce qui ne reflète pas la réalité topographique (un mur de 300 m a un creux très proche, un col de 12 km peut avoir 3 km de faux-plat avant). Le clamp avec plancher/plafond corrige ces cas extrêmes.

### 23.6 Édition numérique vs édition graphique (drag & drop)

| Critère | Champs km (V1) | Drag & drop (v0.5+) |
|---|---|---|
| Précision | ✅✅ | ⚠️ |
| Naturalité | ❌ | ✅✅ |
| Accessibilité clavier | ✅ | ⚠️ (ajoutée en v0.5) |
| Complexité implémentation | ✅ | ⚠️ |
| **Retenu** | ✅ (conservé) | ✅ (mode principal) |

Les deux modes coexistent : l'édition graphique pour l'ajustement rapide, les champs km pour la précision au mètre.

### 23.7 Interception DOM vs hooks Chart.js pour le drag

| Critère | beforeEvent (Chart.js) | **Capture window (v0.5)** |
|---|---|---|
| Fonctionne avec plugin-zoom | ❌ (conflit) | ✅ |
| Ordre garanti | ❌ | ✅ |
| Complexité | ✅ | ⚠️ |
| **Retenu** | ❌ | ✅ |

L'ordre d'appel des hooks `beforeEvent` n'est pas garanti par rapport au plugin-zoom. La capture au niveau `window` garantit l'interception avant tout traitement.

---

## 24. Cas limites et tests

| Cas | Comportement attendu |
|---|---|
| Trace à 2 points | Tous les calculs fonctionnent (trivial) |
| Trace sans dénivelé | Aucune montée détectée, segmentation = 1 catégorie |
| Trace avec un seul sommet | 1 montée détectée |
| Édition d'une montée hors trace | Bornes clampées à [0, totalDist] |
| Édition créant une montée de 0 m | Marqué invalide, longueur = 0 |
| Chevauchement manuel | Contrainte appliquée, indicateur ⚠ |
| **Refine pct = 1 %** | Toutes les montées utilisent REFINE_MIN_M (1500 m) |
| **Refine pct = 500 %** | Toutes les montées utilisent REFINE_MAX_M (8000 m) |
| **Refine pct = 60 %, montée 30 km** | Effective = 8000 m (plafond) |
| **Insertion entre 2 montées trop proches** | Bouton masqué si gap < 200 m |
| **Drag d'une poignée hors zone cliquable** | Aucun drag déclenché (fallthrough vers pan) |
| **Drag + tooltip actif** | Tooltip masqué pendant tout le drag |
| Fichier GPX avec 0 point `<ele>` | Erreur affichée, pas de chargement |
| Trace > 100 000 points | Sous-échantillonnage actif, latence < 3 s |

---

## 25. Références

- **Haversine** : Sinnott, R.W. (1984). *Virtues of the Haversine*. Sky and Telescope.
- **Moving Average** : Smith, S.W. (1997). *The Scientist and Engineer's Guide to DSP*.
- **Hysteresis** : Schmitt, O.H. (1938). *A thermionic trigger*. Journal of Scientific Instruments.
- **Argmin/Argmax** : standard en optimisation numérique.
- **Anchored Smoothing** : variante de l'interpolation linéaire contrainte.
- **DOM Event Capture** : DOM Living Standard, §2.9 (Event dispatch and DOM event flow).

---

**Fin du document Algorithmes**