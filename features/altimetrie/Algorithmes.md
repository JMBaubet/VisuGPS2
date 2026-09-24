# Document Algorithmes — GPX Climb Analyzer

**Version** : 3.6
**Date** : 2026-09-24
**Public** : développeur, mainteneur, auditeur technique
**Portée** : description exhaustive de tous les algorithmes utilisés dans l'application

---

## Table des matières

1. [Distance cumulée (Haversine)](#1-distance-cumulée-haversine)
2. [Rééchantillonnage par moyenne](#2-rééchantillonnage-par-moyenne)
3. [Correction des plateaux GPS/MNT](#3-correction-des-plateaux-gpsmnt)
4. [Anti-pic : fenêtre / Anti-pic : seuil](#4-anti-pic--fenêtre--anti-pic--seuil)
5. [Statistiques de la trace brute](#5-statistiques-de-la-trace-brute)
6. [Moyenne mobile](#6-moyenne-mobile)
7. [Savitzky-Golay](#7-savitzky-golay)
8. [Lissage gaussien](#8-lissage-gaussien)
9. [Filtre médian](#9-filtre-médian)
10. [Lissage ancré](#10-lissage-ancré)
11. [Pente instantanée](#11-pente-instantanée)
12. [Fenêtre de calcul pente](#12-fenêtre-de-calcul-pente)
13. [Écrêtage des pentes](#13-écrêtage-des-pentes)
14. [Détection par hystérésis](#14-détection-par-hystérésis)
15. [Construction des candidats](#15-construction-des-candidats)
16. [Sous-segment optimal](#16-sous-segment-optimal)
17. [Argmin / Argmax](#17-argmin--argmax)
18. [Recherche binaire sur distance](#18-recherche-binaire-sur-distance)
19. [Catégorisation par pente](#19-catégorisation-par-pente)
20. [Longueur min. pente](#20-longueur-min-pente)
21. [Segmentation par catégorie](#21-segmentation-par-catégorie)
22. [Enrichissement des segments](#22-enrichissement-des-segments)
23. [Contrainte anti-chevauchement](#23-contrainte-anti-chevauchement)
24. [Recherche extrema](#24-recherche-extrema)
25. [Écart min. entre montées](#25-écart-min-entre-montées)
26. [Validation finale stricte (v3.1)](#26-validation-finale-stricte-v31)
27. [Insertion contextuelle](#27-insertion-contextuelle)
28. [Édition par drag & drop](#28-édition-par-drag--drop)
29. [Diagnostic de configuration](#29-diagnostic-de-configuration)
30. [Pipeline complet de détection](#30-pipeline-complet-de-détection)
31. [Pipeline de reconstruction](#31-pipeline-de-reconstruction)
32. [Tableau récapitulatif des complexités](#32-tableau-récapitulatif-des-complexités)
33. [Choix de conception et alternatives écartées](#33-choix-de-conception-et-alternatives-écartées)

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

## 2. Rééchantillonnage par moyenne

### Objet

Convertir une série de points irrégulièrement espacés en une série à **pas fixe** (en mètres), en **moyennant** les points bruts tombant dans une fenêtre centrée sur chaque point cible. Cette étape est **fondamentale** : elle garantit que tous les filtres index-based se comportent comme des filtres distance-based, et elle atténue le bruit haute fréquence avant les étapes suivantes.

### Motivation

Les traces GPX (terrain ou éditeur) ont une densité locale très variable :
- Trace GPS : 3 m en montée raide, 20 m en descente rapide
- Trace éditeur Openrunner : 5 m dans les lacets, 800 m en ligne droite

Un filtre `savitzkyGolay(ele, win=60)` appliqué directement :
- En zone dense → couvre 300 m
- En zone clairsemée → couvre **48 000 m**

→ Le **même paramètre produit un effet 100× différent** selon la zone. Le rééchantillonnage élimine ce biais.

De plus, avec l'interpolation linéaire (v3.4 et antérieur), le bruit haute fréquence était conservé tel quel quand le Pas était petit. La **moyenne** (v3.5+) agit comme un filtre passe-bas implicite, ce qui rend le comportement du pipeline **cohérent quelle que soit la valeur du Pas**.

### Formule de la fenêtre

```
avgWin = clamp(stepM, RESAMPLE_AVG_MIN_M, 3 × stepM)
```

avec `RESAMPLE_AVG_MIN_M = 75` m.

| Pas | Fenêtre de moyennage | Points moyennés (typique) |
|---|---|---|
| 5 m | 15 m | ~3 |
| 25 m | 75 m | ~3–4 |
| 50 m | 75 m | ~1.5–2 |
| 100 m | 100 m | ~1 |
| 200 m | 200 m | ~1 |

### Pseudocode

```
FONCTION resampleUniform(distRaw, eleRaw, stepM):
  n ← longueur(distRaw)
  totalM ← distRaw[n−1]
  m ← ⌊totalM / stepM⌋ + 1

  avgWin ← clamp(stepM, RESAMPLE_AVG_MIN_M, 3 × stepM)
  halfW ← avgWin / 2
  startPtr ← 0

  dist[0..m−1], ele[0..m−1] ← tableaux vides

  POUR k DE 0 À m−1:
    dCenter ← k × stepM
    dist[k] ← dCenter
    lo ← dCenter − halfW
    hi ← dCenter + halfW

    // Avancer le pointeur jusqu'au premier point ≥ lo
    TANT QUE startPtr < n − 1 ET distRaw[startPtr] < lo:
      startPtr ← startPtr + 1

    // Moyenner les points dans [lo, hi]
    sumEle ← 0
    count ← 0
    POUR i DE startPtr À n−1:
      SI distRaw[i] ≥ hi: SORTIR
      SI distRaw[i] ≥ lo:
        sumEle ← sumEle + eleRaw[i]
        count ← count + 1

    SI count > 0:
      ele[k] ← sumEle / count
    SINON:
      // Fallback : interpolation (uniquement si stepM < pas brut)
      jj ← 0
      TANT QUE jj < n − 2 ET distRaw[jj+1] < dCenter:
        jj ← jj + 1
      d0 ← distRaw[jj]
      d1 ← distRaw[jj+1]
      SI d1 − d0 < 1e−6:
        ele[k] ← eleRaw[jj]
      SINON:
        t ← (dCenter − d0) / (d1 − d0)
        ele[k] ← eleRaw[jj] + (eleRaw[jj+1] − eleRaw[jj]) × t

  RETOURNER { dist, ele, stepM, avgWinM: avgWin }
```

### Complexité

- Temps : **O(n + m)** où m = totalM / stepM
- Espace : **O(m)**

### Choix du pas

| Type de trace | Pas conseillé |
|---|---|
| GPS terrain haute fréquence | 15–25 m |
| Trace éditeur (MNT 30 m) | **50 m** |
| Trace très longue (> 100 km) | 75–100 m |

**Défaut v3.6** : **50 m**.

### Avantages par rapport à l'interpolation

- Atténue le bruit haute fréquence dès le rééchantillonnage.
- Rend le comportement du pipeline **cohérent quelle que soit la valeur du Pas**.
- Améliore l'efficacité du filtre Hampel en aval (moins de points aberrants à corriger).
- Simplifie la configuration : l'utilisateur n'a plus besoin de « compenser » un Pas fin par un filtre médian.

### Inconvénients

- Légère perte de fidélité sur les très petites variations (< avgWin)
- Sur-échantillonnage possible si stepM < pas médian de la trace

---

## 3. Correction des plateaux GPS/MNT

### Objet

Détecter les séries d'altitudes quasi-identiques (typiques des marches d'escalier du MNT SRTM ou des capteurs GPS qui « gèlent ») et les remplacer par une interpolation linéaire.

### Principe

Sur une trace Openrunner/SRTM, les altitudes sont entières : `1211, 1211, 1211, 1210, 1208...`. Tant qu'on reste dans la même cellule MNT (~30 m × 30 m), l'altitude est constante. En sortant, elle saute d'un coup.

### Pseudocode

```
FONCTION plateauCorrect(ele, minRun, tolerance):
  n ← longueur(ele)
  SI n < 3 OU minRun < 2: RETOURNER { out: copie(ele), count: 0 }

  out ← copie(ele)
  count ← 0
  i ← 0

  TANT QUE i < n:
    // Étendre la série quasi-identique
    j ← i + 1
    TANT QUE j < n ET |ele[j] − ele[i]| ≤ tolerance:
      j ← j + 1

    runLen ← j − i

    // Interpoler si série assez longue ET interne
    SI runLen ≥ minRun ET i > 0 ET j < n:
      eStart ← ele[i−1]
      eEnd   ← ele[j]
      span   ← j − (i−1)
      POUR k DE i À j−1:
        t ← (k − (i−1)) / span
        out[k] ← eStart + (eEnd − eStart) × t
      count ← count + 1

    i ← j

  RETOURNER { out, count }
```

### Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### Notes

- **S'applique avant le rééchantillonnage** (sur les données brutes)
- Optionnel, contrôlé par `plateauRun` (0 = désactivé)
- **Défaut v3.6** : `plateauRun = 6`, `tolerance = 1.0 m`
- Après rééchantillonnage à 50 m, les marches MNT de 30 m sont diluées → la correction devient souvent inutile

---

## 4. Anti-pic : fenêtre / Anti-pic : seuil

### Objet

Éliminer les **glitchs d'altitude isolés** (capteur GPS ou artefact MNT) **sans altérer le relief normal**. Le filtre Hampel est un filtre médian **conditionnel** : il ne remplace un point que si son écart à la médiane locale dépasse un seuil.

### Motivation

Un glitch isolé (1–2 points déviant de 20–50 m) produit une pente instantanée aberrante (±25 %) qui déclenche une fausse détection de montée. Les filtres classiques :
- **Médiane** : corrige mais lisse tout le relief
- **Savitzky-Golay** : ne corrige pas un pic isolé (ordre 2 le préserve)
- **Gaussien** : dilue mais n'élimine pas

Le filtre Hampel est **chirurgical** : il ne modifie que les points aberrants.

### Principe

Pour chaque point `i` :
1. Calculer la **médiane** d'une fenêtre glissante `[i − half, i + half]` où `half = (winPts − 1) / 2`
2. Si `|ele[i] − median| > thresholdM` → remplacer par la médiane
3. Sinon → laisser tel quel

### Conversion mètres → points

La fenêtre est exprimée en **mètres** dans l'interface, mais le filtre Hampel opère en **points**. La conversion est effectuée en interne :

```
winPts = max(3, round(windowM / stepM) | 1)
```

- `round(...)` arrondit au plus proche
- `| 1` force un nombre impair (Hampel requiert une fenêtre symétrique)
- `max(3, ...)` garantit un minimum de 3 points (sinon Hampel dégénère)

**Exemples** :

| Pas | windowM | winPts calculé | Couverture effective |
|---|---|---|---|
| 25 m | 100 m | 4 → 5 | ±50 m |
| 25 m | 200 m | 8 → 9 | ±100 m |
| 25 m | 400 m | 16 → 17 | ±200 m |
| 50 m | 100 m | 2 → 3 | ±50 m |
| 50 m | 200 m | 4 → 5 | ±100 m |
| 50 m | 400 m | 8 → 9 | ±200 m |
| 100 m | 200 m | 2 → 3 | ±100 m |
| 100 m | 400 m | 4 → 5 | ±200 m |

### Pseudocode

```
FONCTION despikeAltitude(ele, winPts, thresholdM):
  n ← longueur(ele)
  SI n < 3 OU winPts < 3 OU thresholdM ≤ 0:
    RETOURNER { out: copie(ele), count: 0 }

  SI winPts pair: winPts ← winPts + 1
  half ← ⌊winPts / 2⌋
  out ← copie(ele)
  buf ← tableau(2*half + 1)
  count ← 0

  POUR i DE 0 À n−1:
    k ← 0
    POUR j DE −half À +half:
      idx ← max(0, min(n−1, i+j))
      buf[k++] ← ele[idx]
    trier(buf)
    median ← buf[half]
    SI |ele[i] − median| > thresholdM:
      out[i] ← median
      count ← count + 1

  RETOURNER { out, count }

// [v3.6] Wrapper : convertit mètres → points puis appelle despikeAltitude
FONCTION applyDespike(ele, windowM, thresholdM, stepM):
  winPts ← max(3, round(windowM / stepM) | 1)
  RETOURNER despikeAltitude(ele, winPts, thresholdM)
```

### Complexité

- Temps : **O(n × winPts × log winPts)** (dominé par le tri de la fenêtre à chaque point)
- Espace : **O(winPts)** pour le buffer

Pour winPts = 5, on trie 5 éléments par point → coût négligeable.

### Choix des paramètres

| Paramètre | Défaut | Plage | Signification |
|---|---|---|---|
| `windowM` | 200 | 50–500 | Largeur de la fenêtre en **mètres** |
| `thresholdM` | 15 | 0–100 | Seuil de détection en mètres |

`thresholdM = 0` désactive complètement le filtre.

### Table des fenêtres effectives

La fenêtre effective en mètres dépend de l'arrondi au multiple de Pas. Si `windowM / stepM < 3`, le plancher de 3 points élargit la fenêtre.

| Pas | windowM | Couverture réelle |
|---|---|---|
| 25 m | 100 m | 125 m (±62) |
| 25 m | 200 m | 225 m (±112) |
| 25 m | 400 m | 425 m (±212) |
| 50 m | 200 m | 250 m (±125) |
| 50 m | 400 m | 450 m (±225) |
| 100 m | 200 m | 300 m (±150) |
| 100 m | 500 m | 500 m (±250) |

Un pic dont la largeur dépasse la fenêtre effective ne sera pas corrigé. C'est voulu : un pic large n'est probablement pas un glitch mais un vrai relief.

### Pourquoi Hampel vs médian classique

| Critère | Médian classique | Hampel |
|---|---|---|
| Modifie le relief normal | Oui (lissage global) | **Non** |
| Élimine un pic isolé | Oui | Oui |
| Préserve les extrema | Non | **Oui** |
| Effet sur les vraies montées | Adoucit | **Aucun** |
| Effet d'écrêtage des bosses | Marqué | **Sélectif** |

### Exemples d'effet

**Cas 1 — Glitch isolé (corrigé)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [1180, 1182, 1183, 1250, 1185, 1186, 1187]
                          ^^^^^ pic aberrant (écart = 67 m > 15 m)
Après :  [1180, 1182, 1183, 1183, 1185, 1186, 1187]
                          ^^^^^ remplacé par la médiane
```

**Cas 2 — Vrai relief (préservé)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [1180, 1182, 1183, 1190, 1198, 1205, 1210]
                          ^^^^^ vrai relief (écart = 7 m < 15 m)
Après :  [1180, 1182, 1183, 1190, 1198, 1205, 1210]
                          ^^^^^ inchangé
```

**Cas 3 — Progression rapide (préserve la pente)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [1000, 1010, 1020, 1030, 1040, 1050, 1060]
                                ^^^^^ écart de 30 m à la médiane
                                      (médiane = 1000) → correction ?
Après :  [1000, 1010, 1020, 1030, 1040, 1050, 1060]
                                ^^^^^ inchangé
```

Attendez — dans ce cas 3, la médiane de la fenêtre `[1000, 1010, 1020, 1030, 1040]` est 1020, et `|1030 − 1020| = 10 < 15` → pas corrigé. ✅

**Cas 4 — Vraie rupture (correctement traitée)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [500, 510, 520, 530, 2000, 540, 550]
                          ^^^^^^^ chute de 1470 m (glitch majeur)
Après :  [500, 510, 520, 530, 530, 540, 550]
                                ^^^^^ remplacé par la médiane
```

### Notes

- Appliqué **après le rééchantillonnage**, **avant tout autre filtrage**
- Les pics de nature large (> winPts) ne sont pas corrigés — c'est voulu
- Un seuil trop bas (< 5 m) peut écrêter des bosses réelles → diagnostic avertit
- Une fenêtre < 2 × Pas rend l'arrondi trop grossier → diagnostic avertit
- **Ordre important** : plateaux → resampling → anti-pic → médiane → SG. Un anti-pic appliqué trop tôt (avant resampling) serait moins efficace car la fenêtre en points varie avec la densité.

### Interaction avec les autres filtres

| Filtre | Interaction |
|---|---|
| Filtre médian | **Redondant** — si l'anti-pic est actif, désactiver le filtre médian (paramètre `medianWindowM = 0`) |
| Correction plateaux | Complémentaire — s'applique avant (données brutes) |
| Savitzky-Golay | Complémentaire — SG traite le bruit diffus, l'anti-pic traite les glitchs ponctuels |
| Gaussien | Complémentaire — adoucit après anti-pic |

---

## 5. Statistiques de la trace brute

### Objet

Calculer les statistiques globales **sur les données GPX d'origine**, en ignorant les doublons GPS.

### Pseudocode

```
FONCTION computeRawStats(distRaw, eleRawOrig):
  // D+ / D− en ignorant les doublons (< 1 m)
  dPlus ← 0, dMoins ← 0
  POUR i DE 1 À n−1:
    dDist ← distRaw[i] − distRaw[i−1]
    SI dDist < 1.0: CONTINUER
    d ← eleRawOrig[i] − eleRawOrig[i−1]
    SI d > 0: dPlus ← dPlus + d
    SINON:    dMoins ← dMoins − d

  // Distribution des pas
  gaps ← [toutes les distances > 0.01 m]
  trier(gaps)
  gapMin ← gaps[0]
  gapMed ← gaps[⌊len/2⌋]
  gapMax ← gaps[len−1]
  gapMean ← moyenne(gaps)

  // Altitudes min/max
  eleMin ← min(eleRawOrig)
  eleMax ← max(eleRawOrig)

  RETOURNER { n, dPlus, dMoins, gapMin, gapMed, gapMax, gapMean,
              eleMin, eleMax, doublons: n − length(gaps) }
```

### Complexité

- Temps : **O(n log n)** (dominé par le tri des gaps)
- Espace : **O(n)**

### Notes

- Les statistiques sont **calculées une seule fois au chargement**
- Elles sont indépendantes du resampling et des paramètres
- Elles alimentent les cartes statistiques (D+ brut, altitudes min/max)

---

## 6. Moyenne mobile

### Objet

Lisser une série par moyenne glissante centrée. Utilisée pour la pente.

### Pseudocode

```
FONCTION movingAverage(arr, win):
  n ← longueur(arr)
  SI win < 2 OU n < 2: RETOURNER copie(arr)

  half ← ⌊win / 2⌋
  prefix[0] ← 0
  POUR i DE 0 À n−1: prefix[i+1] ← prefix[i] + arr[i]

  res ← nouveau tableau(n)
  POUR i DE 0 À n−1:
    lo ← max(0, i − half)
    hi ← min(n−1, i + half)
    res[i] ← (prefix[hi+1] − prefix[lo]) / (hi − lo + 1)

  RETOURNER res
```

### Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### Notes

- Conversion mètres → points : `winPts = round(winM / stepM)`, forcée impaire
- Après resampling uniforme, une fenêtre de N points couvre **exactement** N × stepM mètres partout

---

## 7. Savitzky-Golay

### Objet

Lisser en ajustant localement un polynôme d'ordre 2 sur une fenêtre glissante. **Préserve mieux les extrema** qu'une moyenne mobile de même largeur.

### Formule (ordre 2)

Pour chaque point i, on calcule une combinaison linéaire pondérée des voisins :
```
coefs[k] = 3m² + 3m − 1 − 5k²   pour k ∈ [−m, +m]
```
où `m = (win − 1) / 2`.

### Pseudocode

```
FONCTION savitzkyGolay(arr, win, order=2):
  n ← longueur(arr)
  SI win < 5 OU n < 5: RETOURNER copie(arr)
  SI win pair: win ← win + 1

  half ← ⌊win / 2⌋
  m ← half
  coefs[k+half] ← 3m² + 3m − 1 − 5k²   pour k ∈ [−half, +half]

  out ← nouveau tableau(n)
  POUR i DE 0 À n−1:
    sum ← 0, wsum ← 0
    POUR j DE −half À +half:
      idx ← i + j
      SI idx hors bornes: CONTINUER
      c ← coefs[j + half]
      sum  ← sum  + c × arr[idx]
      wsum ← wsum + c
    out[i] ← sum / wsum

  RETOURNER out
```

### Complexité

- Temps : **O(n × win)**
- Espace : **O(n)**

### Notes

- Utilisé comme **lisseur principal** de l'altitude (remplace la moyenne mobile en v2.8+)
- Ordre 2 = préservation des courbures locales, idéal pour les extrema
- Sur données resamplées, `win` en points ≈ `fenetreAffichageM / stepM`
- **Inopérant si `fenetreAffichageM / stepM < 5`** — d'où l'importance du diagnostic de cohérence

---

## 8. Lissage gaussien

### Objet

Convolution avec un noyau gaussien tronqué à ±3σ. Lisse fortement les transitions.

### Pseudocode

```
FONCTION gaussianFilter(arr, sigma):
  n ← longueur(arr)
  SI sigma ≤ 0 OU n < 2: RETOURNER copie(arr)

  half ← ⌈3σ⌉
  kernel[k+half] ← exp(−k²/(2σ²))   pour k ∈ [−half, +half]
  normaliser(kernel)   // somme = 1

  out ← nouveau tableau(n)
  POUR i DE 0 À n−1:
    sum ← 0, wsum ← 0
    POUR j DE −half À +half:
      idx ← i + j
      SI idx hors bornes: CONTINUER
      w ← kernel[j + half]
      sum  ← sum  + w × arr[idx]
      wsum ← wsum + w
    out[i] ← sum / wsum

  RETOURNER out
```

### Complexité

- Temps : **O(n × σ)**
- Espace : **O(n)**

### Notes

- Appliqué **après le Savitzky-Golay, avant l'ancrage**
- σ en mètres converti en σ en points : `sigmaPts = sigmaM / stepM`
- Défaut v3.6 : σ = 75 m (≈ 1.5 point à stepM=50)
- **Inopérant si σM / stepM < 0.5** — d'où le warning diagnostic

---

## 9. Filtre médian

### Objet

Remplacer chaque valeur par la médiane de son voisinage. Élimine les pics isolés.

### Pseudocode

```
FONCTION medianFilter(arr, win):
  n ← longueur(arr)
  SI win < 3 OU n < 3: RETOURNER copie(arr)
  SI win pair: win ← win + 1

  half ← ⌊win / 2⌋
  out ← nouveau tableau(n)
  POUR i DE 0 À n−1:
    buf ← [arr[max(0, i−half) .. min(n−1, i+half)]]
    trier(buf)
    out[i] ← buf[⌊len/2⌋]

  RETOURNER out
```

### Complexité

- Temps : **O(n × win log win)** (à cause du tri par point)
- Espace : **O(win)**

### Notes

- **Première étape** du pipeline de filtrage (avant SG)
- Défaut v3.6 : `medianWindowM = 0` (désactivé)
- **Depuis v3.3, l'anti-pic Hampel rend le filtre médian largement redondant**. Recommandation : le désactiver si anti-pic actif

---

## 10. Lissage ancré

### Objet

Lisser en forçant la courbe à passer **exactement** par certains points (les extrema détectés). Résout le problème du décalage du sommet vers la descente.

### Principe

1. Calculer un lissage de base (SG + gaussien)
2. Pour chaque ancre `a`, calculer `corr[a] = raw[a] − base[a]`
3. Appliquer une correction linéaire par morceaux entre ancres

### Pseudocode

```
FONCTION anchoredMovingAverage(raw, anchorIdxs, stepM,
                               fenetreAffichageM, sigmaM, doubleSG):
  n ← longueur(raw)
  winPts ← max(5, round(fenetreAffichageM / stepM) | 1)

  // Lissage de base
  base ← savitzkyGolay(raw, winPts, 2)
  SI sigmaM > 0:
    sigmaPts ← sigmaM / stepM
    base ← gaussianFilter(base, sigmaPts)

  // Ancres = {0, n−1} ∪ anchorIdxs valides
  anchors ← trier({0, n−1, i ∈ anchorIdxs : 0 < i < n−1})

  result ← copie(base)

  // Correction linéaire entre chaque paire d'ancres
  POUR k DE 0 À |anchors| − 2:
    a ← anchors[k], b ← anchors[k+1]
    SI b − a < 2: CONTINUER
    corrA ← raw[a] − base[a]
    corrB ← raw[b] − base[b]
    POUR i DE a À b:
      t ← (i − a) / (b − a)
      result[i] ← base[i] + corrA × (1 − t) + corrB × t

  // Double SG optionnel
  SI doubleSG:
    winDouble ← max(31, round(fenetreAffichageM × 2 / stepM) | 1)
    final ← savitzkyGolay(result, winDouble, 2)
    // Préserver les valeurs exactes aux ancres
    POUR chaque a DANS anchors: final[a] ← result[a]
    RETOURNER final

  RETOURNER result
```

### Complexité

- Temps : **O(n + m × winDouble)** où m = nb d'ancres
- Espace : **O(n)**

### Propriétés mathématiques

- **Exactitude aux ancres** : `result[a] = base[a] + corrA = raw[a]` par construction
- **Continuité** : entre deux ancres, la correction est une interpolation linéaire, donc continue
- **Lissage préservé** : la courbe reste lisse à l'intérieur de chaque intervalle (la correction est linéaire, donc n'introduit pas d'oscillation)

### Notes

- **Effet visuel** : la pente devient exactement nulle au sommet d'une ancre, ce qui résout le problème « la pente continue de monter après le sommet »
- Le choix des ancres est crucial. Si elles sont placées à chaque changement de catégorie, la courbe est très contrainte

---

## 11. Pente instantanée

### Objet

Calculer la pente locale à chaque point, en % (variation d'altitude / distance).

### Formule

Pour le point `i`, on utilise les points voisins `i−1` et `i+1` :

```
pente[i] = (ele[i+1] − ele[i−1]) / (dist[i+1] − dist[i−1]) × 100
```

### Pseudocode

```
FONCTION computeSlope(ele, dist, minD=0.05):
  n ← longueur(ele)
  s ← tableau(n, 0)

  POUR i DE 0 À n−1:
    i1 ← max(0, i − 1)
    i2 ← min(n − 1, i + 1)
    d ← dist[i2] − dist[i1]
    SI d > minD: s[i] ← (ele[i2] − ele[i1]) / d × 100

  RETOURNER s
```

### Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### Notes

- `minD = 0.05 m` évite les divisions par zéro quand deux points sont trop proches
- **Différence centrée** plutôt que différence avant : réduit le décalage de phase
- Très sensible au bruit GPS — d'où l'importance de l'anti-pic en amont

---

## 12. Fenêtre de calcul pente

**Nom UI** : Fenêtre de calcul pente · **Clé technique** : `fenetreCatM` · **Défaut v3.6** : 100 m

### Objet

Calculer la pente **moyenne sur une portion de N mètres** centrée sur chaque point. Utilisée pour la catégorisation par couleur.

### Pseudocode

```
FONCTION computeSlopeWindow(ele, dist, winM):
  n ← longueur(ele)
  s ← tableau(n, 0)

  POUR i DE 0 À n−1:
    // Étendre vers la gauche
    lo ← i
    TANT QUE lo > 0 ET dist[i] − dist[lo−1] ≤ winM: lo ← lo − 1

    // Étendre vers la droite
    hi ← i
    TANT QUE hi < n−1 ET dist[hi+1] − dist[i] ≤ winM: hi ← hi + 1

    // Pente sur la fenêtre
    dDist ← dist[hi] − dist[lo]
    SI dDist > 0.5: s[i] ← (ele[hi] − ele[lo]) / dDist × 100

  RETOURNER s
```

### Complexité

- Temps : **O(n × w)** où w = nombre moyen de points dans la fenêtre
- Espace : **O(n)**

### Notes

- **Fenêtre effective** entre winM et 2×winM selon l'alignement
- Après resampling à 50 m, w ≈ 2 pour winM=100 m

---

## 13. Écrêtage des pentes

### Objet

Limiter les pentes affichées à ±25 % pour éviter les pics verticaux dus au bruit.

### Pseudocode

```
FONCTION clipSlope(slope, maxPct=25):
  out ← tableau(|slope|)
  POUR i DE 0 À |slope|−1:
    v ← slope[i]
    out[i] ← max(−maxPct, min(maxPct, v))
  RETOURNER out
```

### Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### Notes

- Constante `SLOPE_DISPLAY_MAX_PCT = 25`
- Utilisé pour : `slopeBase`, `slopeRaw`, `slopeDisplay`
- L'axe Y du graphe de pente est fixé à ±27 % pour cohérence
- **N'affecte PAS la détection** — c'est un écrêtage affichage uniquement

---

## 14. Détection par hystérésis

### Objet

Détecter les portions de montée avec **hystérésis** (seuil d'entrée positif, seuil de sortie négatif).

### Pseudocode

```
FONCTION detectEvents(slope, seuil):
  events ← []
  inClimb ← FAUX

  POUR i DE 0 À n−1:
    SI NON inClimb ET slope[i] > +seuil:
      events.ajouter({ idx: i, type: 'start' })
      inClimb ← VRAI
    SINON SI inClimb ET slope[i] < −seuil:
      events.ajouter({ idx: i, type: 'summit' })
      inClimb ← FAUX

  // Si on termine en montée, fermer le dernier événement
  SI inClimb:
    events.ajouter({ idx: n−1, type: 'summit' })

  RETOURNER events
```

### Complexité

- Temps : **O(n)**
- Espace : **O(k)** (k = nb d'événements)

### Notes

- Seuil par défaut : 0.5 %
- Hystérésis (+0.5 / −0.5) évite les oscillations autour du seuil
- **Peut fragmenter une montée** si un faux-plat descend temporairement sous −0.5 % → d'où la fusion (§25)

---

## 15. Construction des candidats

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

  SI prev < n − 1:
    segs.ajouter({startIdx: prev, endIdx: n−1, key: cur})

  RETOURNER filtrer(segs, s → s.key = 'climb')
```

### Complexité

- Temps : **O(k)**
- Espace : **O(k)**

---

## 16. Sous-segment optimal

### Objet

Dans un candidat `[startIdx, endIdx]`, trouver le **plus long sous-segment** qui satisfait les critères (longueur, pente moyenne). C'est l'algorithme de validation.

### Critères de validation

```
seuil(L) = 3 % si L ≥ 1000 m
         = 4 % si 750 ≤ L < 1000
         = 5 % si 500 ≤ L < 750
         = 6 % si 400 ≤ L < 500
         = 7 % si 300 ≤ L < 400
         = ∞   sinon (rejeté)
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

- Sans sous-échantillonnage : **O(n²)**
- Avec sous-échantillonnage (step adaptatif) : **O(800²) ≈ 640 000** opérations max par candidat
- Espace : **O(n)**

### Notes

- La stratégie privilégie le **plus long** sous-segment valide
- Cette fonction est appelée **deux fois** par candidat : sur `eleBase` (validation intermédiaire) et sur `eleDisplay` (validation finale)
- Si aucun sous-segment valide n'existe, la fonction retourne `null` et la montée est rejetée

---

## 17. Argmin / Argmax

### Objet

Trouver l'indice du **minimum** (resp. maximum) d'un tableau numérique dans un intervalle `[lo, hi]`.

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

- Temps : **O(hi − lo + 1)**
- Espace : **O(1)**

---

## 18. Recherche binaire sur distance

### Objet

Trouver l'indice du **premier point** dont la distance cumulée est ≥ une valeur cible.

### Pseudocode

```
FONCTION findIdxAtDist(dist, targetM):
  lo ← 0
  hi ← longueur(dist) − 1

  TANT QUE lo < hi:
    mid ← (lo + hi) / 2  (division entière)
    SI dist[mid] < targetM: lo ← mid + 1
    SINON: hi ← mid

  RETOURNER lo
```

### Complexité

- Temps : **O(log n)**
- Espace : **O(1)**

---

## 19. Catégorisation par pente

### Objet

Associer une **catégorie de pente** à une valeur de pente en %.

### Grille (8 catégories)

| ID | Libellé | Plage |
|---|---|---|
| dh3 | Descente raide | p < −8 |
| dh2 | Descente | −8 ≤ p < −3 |
| dh1 | Faux plat descendant | −3 ≤ p < −1 |
| pl | Plat | −1 ≤ p < +1 |
| up1 | Faux plat montant | +1 ≤ p < +3 |
| up2 | Montée | +3 ≤ p < +7 |
| up3 | Montée raide | +7 ≤ p < +10 |
| up4 | Montée très raide | p ≥ +10 |

### Pseudocode

```
FONCTION categorize(slope):
  POUR chaque cat DANS CATEGORIES (dans l'ordre):
    SI slope ≥ cat.min ET slope < cat.max: RETOURNER cat.id
  RETOURNER 'pl'
```

### Complexité

- Temps : **O(c)** où c = nombre de catégories (8). En pratique constant → **O(1)**
- Espace : **O(1)**

### Notes

- Les bornes utilisent `<` en borne haute : une pente de +3 % exactement tombe dans `up1`, pas dans `up2`
- La symétrie descente/montée n'est pas parfaite

---

## 20. Longueur min. pente

**Nom UI** : Longueur min. pente · **Clé technique** : `longueurMin` · **Défaut v3.6** : 200 m

### Objet

Supprimer les segments dont la longueur est inférieure à un seuil (par défaut 200 m) en les absorbant dans leur **voisin le plus long**.

### Algorithme (2 passes)

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

      leftLen  ← (i > 0) ? dist[raw[i−1].endIdx] − dist[raw[i−1].startIdx] : −1
      rightLen ← (i < n−1) ? dist[raw[i+1].endIdx] − dist[raw[i+1].startIdx] : −1

      SI rightLen ≥ leftLen:
        raw[i+1].startIdx ← s.startIdx
      SINON:
        raw[i−1].endIdx ← s.endIdx

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

- Temps : **O(s²)** dans le pire cas, **O(s)** en pratique
- Espace : **O(s)**

---

## 21. Segmentation par catégorie

### Objet

Découper la trace en segments contigus selon la catégorie de pente de chaque point, **sans laisser de zone blanche**.

### Principe clé

Le **point frontière** (là où la catégorie change) appartient à la fois au segment précédent et au segment suivant.

### Pseudocode

```
FONCTION segmentByCategory(slopeForCat, dist, ele, minLen):
  // 1. Catégoriser chaque point
  cat[i] ← categorize(slopeForCat[i])

  // 2. Découper en segments contigus
  raw ← []
  start ← 0

  POUR i DE 1 À n:
    SI i = n OU cat[i] ≠ cat[start]:
      endIdx ← (i = n) ? i − 1 : i    // ← point frontière partagé
      raw.ajouter({startIdx: start, endIdx: endIdx, key: cat[start]})
      start ← i

  // 3. Fusionner les segments trop courts
  RETOURNER mergeShortSegments(raw, dist, minLen)
    .map(s → enrichSegment(s, dist, ele, slopeForCat))
```

### Complexité

- Temps : **O(n)** pour la catégorisation + **O(s²)** pour la fusion
- Espace : **O(n)**

### Notes

- La somme des longueurs des segments après fusion **égale exactement la distance totale**

---

## 22. Enrichissement des segments

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

  RETOURNER {...s, startDist, endDist, length, dPlus, avgSlope,
              altStart: ele[s.startIdx], altEnd: ele[s.endIdx]}
```

### Complexité

- Temps : **O(len)**
- Espace : **O(1)**

### Notes

- La **pente moyenne** est pondérée par la distance, pas par le nombre de points

---

## 23. Contrainte anti-chevauchement

### Objet

Garantir que les montées éditées manuellement ne se chevauchent jamais.

### Pseudocode (édition numérique)

```
FONCTION appliquerContraintes(userClimbs):
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

- Temps : **O(m log m)** pour le tri + **O(m)** pour la contrainte
- Espace : **O(m)**

### Application temps réel pendant le drag

```
FONCTION applyDragConstraint(handle, km, climb):
  totalKm ← dist[fin] / 1000
  minLen  ← 0.05    // 50 m minimum
  minGap  ← 0.001   // 1 m entre montées

  sorted ← trier(STATE.userClimbs par startKm)
  pos ← index de climb dans sorted
  prevClimb ← (pos > 0) ? sorted[pos−1] : null
  nextClimb ← (pos < |sorted|−1) ? sorted[pos+1] : null

  lo ← (prevClimb) ? prevClimb.endKm + minGap : 0
  hi ← (nextClimb) ? nextClimb.startKm − minGap : totalKm

  SI handle = 'valley':
    RETOURNER clamp(km, lo, climb.endKm − minLen)
  SINON:
    RETOURNER clamp(km, climb.startKm + minLen, hi)
```

---

## 24. Recherche extrema

**Nom UI** : Recherche extrema · **Clé technique** : `refinePct` · **Défaut v3.6** : 60 %

### Objet

Limiter la distance maximale de recherche de la vraie vallée / du vrai sommet lors de l'étape de raffinement.

### Formule

```
effectiveRefine = clamp(
  refinePct% × candidate.length,
  REFINE_MIN_M,
  REFINE_MAX_M
)
```

avec :
- `refinePct` : paramètre exposé (défaut 60 %)
- `REFINE_MIN_M = 1500` m
- `REFINE_MAX_M = 8000` m

### Table d'exemples

| Longueur candidat | refinePct=60% | Après clamp |
|---|---|---|
| 300 m | 180 m | **1500 m** |
| 1 km | 600 m | **1500 m** |
| 3 km | 1800 m | **1800 m** |
| 6 km | 3600 m | **3600 m** |
| 10 km | 6000 m | **6000 m** |
| 20 km | 12000 m | **8000 m** |

### Application

```
hiIdx ← findIdxAtDist(dist, min(dist[n−1], dist[c.endIdx] + effectiveRefine))
newEnd ← argmaxRange(eleBase, c.endIdx, hiIdx)

safetyLow ← findIdxAtDist(dist, max(0, dist[c.startIdx] − effectiveRefine))
loIdx ← max(lastEnd + 1, safetyLow)
newStart ← argminRange(eleBase, loIdx, c.startIdx)
```

### Complexité

- Temps : **O(1)** pour le clamp + **O(range)** pour l'argmin/argmax

### Notes

- **Adaptatif** : les longues montées ont plus de latitude pour chercher leur vrai sommet/creux
- **Borné** : les montées courtes ne partent pas chercher un sommet à 10 km
- La valeur effective est affichée par montée dans la colonne Extrema

---

## 25. Écart min. entre montées

**Nom UI** : Écart min. entre montées · **Clé technique** : `gapMinM` · **Défaut v3.6** : 250 m

### Objet

Fusionner deux montées détectées séparées de moins de `gapMinM`. Corrige la fragmentation due aux faux-plats dans une montée continue.

### Principe

Après le raffinement de chaque candidat, on compare avec la montée précédente :
- Si `dist[newStart] − dist[prev.endIdx] < gapMinM` → fusion
- Le nouveau sommet = **le plus haut** des deux
- Le nouveau creux = **le plus bas** entre les deux (si plus bas que le creux actuel)

### Pseudocode

```
// Dans la boucle de raffinement
SI refined.length > 0 ET gapMinM > 0:
  prev ← refined[dernier]
  gapM ← dist[newStart] − dist[prev.endIdx]

  SI gapM < gapMinM:
    // Fusion : étendre prev
    combinedEnd ← argmaxRange(eleBase, prev.endIdx, newEnd)

    // Vérifier si le creux intermédiaire est plus bas
    midValley ← argminRange(eleBase, prev.endIdx, newStart)
    newPrevStart ← prev.startIdx
    SI eleBase[midValley] < eleBase[prev.startIdx]:
      newPrevStart ← midValley
      anchors[dernier − 1] ← midValley    // maj ancre vallée

    dbgMerge(`Montée #${refined.length} étendue : écart ${gapM} m < ${gapMinM} m`)

    prev.startIdx    ← newPrevStart
    prev.endIdx      ← combinedEnd
    prev.candidateLen ← prev.candidateLen + candidateLen
    prev.refineM     ← max(prev.refineM, effectiveRefine)
    prev._merged     ← (prev._merged || 0) + 1

    anchors[dernier] ← combinedEnd
    lastEnd ← combinedEnd
    CONTINUER    // ne pas ajouter de nouvelle montée

// Sinon : ajout normal
anchors.push(newStart, newEnd)
refined.push({ startIdx: newStart, endIdx: newEnd, ..., _merged: 0 })
lastEnd ← newEnd
```

### Complexité

- Temps : **O(m)** pour l'ensemble des fusions
- Espace : **O(1)** par fusion

### Notes

- `gapMinM = 0` → désactive complètement
- **Défaut v3.6** : 250 m
- Le compteur `mergedCount` est affiché avec l'indicateur `⊕`
- **Conséquence** : les bornes finales peuvent avoir une pente moyenne plus faible que la montée initiale → d'où la validation stricte (§26)

---

## 26. Validation finale stricte (v3.1)

### Objet

Garantir qu'**aucune montée affichée n'est sous son seuil de pente**, même après fusion et raffinement.

### Le bug corrigé

**Avant v3.1** :
```javascript
const sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx);
if (!sub) continue;
if (sub.avgSlope < minSlopeForLength(sub.length)) continue;
// ↑ validation sur `sub` (le sous-segment)
autoClimbs.push({
  startKm: dist[c.startIdx] / 1000,   // ← mais bornes = bornes raffinées !
  endKm: dist[c.endIdx] / 1000
});
```

Résultat : un segment raffiné de 7300 m contenant un sous-segment valide de 2000 m à 3.5 % passait la validation, puis affichait **2.8 % sur ses propres 7300 m**.

### Le correctif

Validation **sur les bornes finales** d'abord, puis repli sur sous-segment si échec.

### Pseudocode

```
FONCTION validerEtReplier(refined, dist, eleDisplay):
  autoClimbs ← []
  rejectedCount ← 0
  fallbackCount ← 0

  POUR chaque c DANS refined:
    finalStart ← c.startIdx
    finalEnd   ← c.endIdx

    // Calculs sur bornes finales
    length   ← dist[finalEnd] − dist[finalStart]
    dPlus    ← somme des Δele > 0
    avgSlope ← dPlus / length × 100
    minSlope ← minSlopeForLength(length)

    // Échec ?
    SI length < 300 OU avgSlope < minSlope:
      sub ← findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx)

      SI sub === null:
        dbgWarn(`Rejeté : km ... (${avgSlope} % < ${minSlope} %)`)
        rejectedCount ← rejectedCount + 1
        CONTINUER

      dbgFallback(`Repli km ... → ...`)
      finalStart ← sub.startIdx
      finalEnd   ← sub.endIdx
      fallbackCount ← fallbackCount + 1

    autoClimbs.push({
      startKm: dist[finalStart] / 1000,
      endKm: dist[finalEnd] / 1000,
      refineM: c.refineM,
      candidateLen: c.candidateLen,
      mergedCount: c._merged || 0,
      fallback: (finalStart ≠ c.startIdx) OU (finalEnd ≠ c.endIdx)
    })

  RETOURNER autoClimbs
```

### Complexité

- Temps : **O(m × 800²)** (dominé par les appels à findBestSubClimb sur échec)
- Espace : **O(m)**

### Notes

- L'indicateur `⤺` signale une montée dont les bornes ont été repliées
- Le log debug trace chaque repli en détail
- Garantit l'invariant : **pour toute montée affichée, `avgSlope ≥ seuil(length)`**

---

## 27. Insertion contextuelle

### Règles d'affichage

| Position | Condition | Constante |
|---|---|---|
| Avant #1 | `climbs[0].startDist ≥ 1 km` | `INSERT_EDGE_MIN_KM = 1.0` |
| Entre #i et #i+1 | `gap ≥ 200 m` | `INSERT_BETWEEN_MIN_KM = 0.2` |
| Après #N | `totalDist − lastEnd ≥ 1 km` | `INSERT_EDGE_MIN_KM` |
| Aucune montée | Toujours affiché | — |

### Algorithme d'insertion

```
FONCTION insertInGap(gapStartKm, gapEndKm):
  gapLen ← gapEndKm − gapStartKm
  SI gapLen < INSERT_BETWEEN_MIN_KM: REJETER

  targetLen ← max(0.1, min(INSERT_DEFAULT_LEN_KM, gapLen × 0.8))
  center ← (gapStartKm + gapEndKm) / 2
  start  ← max(gapStartKm + 0.01, center − targetLen / 2)
  end    ← min(gapEndKm − 0.01, center + targetLen / 2)

  SI end ≤ start: REJETER

  userClimbs.push({
    startKm: start, endKm: end,
    autoStartKm: null, autoEndKm: null,
    refineM: null, candidateLen: null,
    mergedCount: 0, fallback: false
  })
```

### Complexité

- Temps : **O(m)** pour le calcul du gap et l'insertion
- Espace : **O(1)** par insertion

### Notes

- La montée insérée **n'a pas de `refineM`** → colonne Extrema affiche `—` en gris italique
- Les boutons d'insertion apparaissent **entre les lignes** du tableau
- Un clic sur un bouton d'insertion **ne déclenche pas** la sélection de la ligne

---

## 28. Édition par drag & drop

### Architecture de l'interception

`chartjs-plugin-zoom` attache ses listeners sur le `canvas` en phase **cible**. Pour intercepter avant, on écoute au **window en phase capture** :

```
Phase capture  : window → ... → canvas   ← notre listener
Phase cible    : canvas                   ← Chart.js + plugin-zoom
Phase bullage  : canvas → ... → window
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
  xS ← scales.x.getPixelForValue(climb.endKm)
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
  panOriginalState ← { elev: ..., slope: ... }
  elevChart.pan.enabled ← FAUX
  slopeChart.pan.enabled ← FAUX
  elevChart.tooltip.enabled ← FAUX
  slopeChart.tooltip.enabled ← FAUX

  SUR pointermove (capture) :
    cx, cy ← coordonnées souris/tactile
    px ← cx − rect.left
    km ← scales.x.getValueForPixel(px)

    constrainedKm ← applyDragConstraint(handle, km, climb)   // §23
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

### Complexité par frame

- Position : **O(1)**
- `findHandleAt` : **O(1)**
- `updatePreviewMetrics` : **O(range)** (borné par la longueur de la montée en cours)
- `chart.draw()` : **O(points affichés)** ≈ 4000

En pratique : ~5–10 ms par frame, largement sous les 16 ms d'une frame à 60 fps

### Notes

- **Aucun rebuild complet** pendant le drag : seuls le canvas et la ligne du tableau sont mis à jour
- **Rebuild complet** uniquement au `mouseup` (via `commitDrag`)
- Le tooltip est **désactivé pendant le drag** (v2.1) pour ne pas masquer les courbes significatives

---

## 29. Diagnostic de configuration

### Objet

Afficher en temps réel l'état de cohérence des paramètres et les **valeurs effectives** en points.

### Principe

Pour chaque paramètre en mètres, la valeur effective en points = `paramètre / stepM`. Le diagnostic vérifie que cette valeur reste dans une plage optimale.

### Fonction `computeDiagnostics`

```javascript
function computeDiagnostics(params) {
  const {
    stepM, despikeWindowM, despikeThresholdM,
    fenetreAffichageM, fenetrePenteM, fenetreCatM,
    seuilMontee, longueurMin, gapMinM, refinePct,
    medianWindowM, gaussianSigmaM, plateauRun
  } = params;

  // Valeurs effectives en points
  const lissageAltPts   = Math.max(1, Math.round(fenetreAffichageM / stepM));
  const lissagePentePts = Math.max(1, Math.round(fenetrePenteM / stepM));
  const couleurPts      = Math.max(1, Math.round(fenetreCatM / stepM));
  const medianPts       = medianWindowM > 0
                          ? Math.max(1, Math.round(medianWindowM / stepM))
                          : 0;
  const gaussianPts     = gaussianSigmaM > 0 ? gaussianSigmaM / stepM : 0;

  // [v3.6] Anti-pic : fenêtre effective en points
  const despikeActive = despikeThresholdM > 0;
  const despikeWinPtsEffective = despikeActive
    ? Math.max(3, Math.round(despikeWindowM / stepM) | 1)
    : 0;
  const despikeCovM = despikeWinPtsEffective * stepM;
  const despikeExpandRatio = despikeActive && despikeWindowM > 0
    ? despikeCovM / despikeWindowM
    : 1;

  // Ratios et dérivés
  const ratioAltPente = fenetrePenteM > 0
                        ? fenetreAffichageM / fenetrePenteM
                        : 0;

  const diag = {};

  // ... pour chaque paramètre :
  //   { label, effective, optimal, target, status, msg }
  // status ∈ { green, orange, red, grey }

  // Règles de cohérence
  const warnings = [];
  if (lissageAltPts < 5) {
    warnings.push({ level: 'red',
      msg: `Lissage altitude : fenêtre de ${lissageAltPts} pts (min 5).` });
  }
  if (lissagePentePts < 3) {
    warnings.push({ level: 'red',
      msg: `Lissage pente : fenêtre de ${lissagePentePts} pts (min 3).` });
  }
  if (ratioAltPente < 1.5) {
    warnings.push({ level: 'red',
      msg: `Ratio altitude/pente = ${ratioAltPente.toFixed(1)}× (recommandé ≥ 2).` });
  }
  if (medianWindowM > 0 && medianPts < 3) {
    warnings.push({ level: 'orange',
      msg: `Filtre médian : fenêtre de ${medianPts} pt(s).` });
  }
  // [v3.6] Warnings spécifiques anti-pic (fenêtre en mètres)
  if (despikeActive && despikeWindowM < 2 * stepM) {
    warnings.push({ level: 'orange',
      msg: `Anti-pic : fenêtre < 2 × Pas, la fenêtre effective sera élargie.` });
  } else if (despikeActive && despikeExpandRatio > 1.5) {
    warnings.push({ level: 'orange',
      msg: `L'arrondi gonfle l'anti-pic : fenêtre de ${despikeWindowM} m à ${despikeCovM} m.` });
  }
  if (despikeActive && despikeThresholdM < 5) {
    warnings.push({ level: 'orange',
      msg: `Anti-pic : seuil ${despikeThresholdM} m très bas.` });
  }

  return { diag, warnings };
}
```

### Statuts

| Statut | Signification | Exemple |
|---|---|---|
| 🟢 green | Optimal | Fenêtre SG ≥ 10 pts |
| 🟠 orange | Limite / avertissement | Fenêtre SG entre 5 et 10 pts |
| 🔴 red | Incohérence critique | Fenêtre SG < 5 pts |
| ⚪ grey | Désactivé / non applicable | Filtre médian = 0 |

### Complexité

- Temps : **O(1)** (calculs sur 14 paramètres)
- Espace : **O(1)**

### Notes

- Le diagnostic est recalculé à chaque `input` sur un champ (debounce 250 ms)
- Les warnings sont affichés dans un panneau dédié
- Le statut global (🟢/🟠/🔴) résume l'état : vert si aucun warning, orange si au moins un orange, rouge si au moins un rouge

### Valeurs optimales (v3.6)

| Paramètre | Optimal (pts) | Optimal (m, Pas=50) |
|---|---|---|
| Lissage altitude | 10–40 | 500–2000 |
| Lissage pente | 3–10 | 150–500 |
| Fenêtre de calcul pente | 3–15 | 150–750 |
| Filtre médian | désactivé ou ≥ 3 | 0 ou ≥ 150 |
| Lissage gaussien | 0.5–5 | 25–250 |
| Anti-pic : fenêtre | — | 150–300 m |
| Anti-pic : seuil | — | 15–25 m |

---

## 30. Pipeline complet de détection

### Pseudocode

```
FONCTION computeAuto(dist, eleRaw, params):
  n ← longueur(eleRaw)

  // === ÉTAPE 1 : filtre médian (optionnel) ===
  SI medianWindowM > 0:
    winPts ← round(medianWindowM / stepM) | 1
    eleMedian ← medianFilter(eleRaw, winPts)
  SINON:
    eleMedian ← eleRaw

  // === ÉTAPE 2 : Savitzky-Golay ===
  winSGPts ← round(fenetreAffichageM / stepM) | 1
  eleBase ← savitzkyGolay(eleMedian, winSGPts, 2)

  // === ÉTAPE 3 : pente lissée ===
  winSlopePts ← round(fenetrePenteM / stepM)
  slopeBase ← clipSlope(
    movingAverage(computeSlope(eleBase, dist), winSlopePts),
    25
  )

  // === ÉTAPE 4 : hystérésis ===
  events ← detectEvents(slopeBase, seuilMontee)
  candidates ← buildClimbCandidates(events, n)

  // === ÉTAPE 5 : sous-segment optimal ===
  candidatesWithSub ← []
  POUR chaque c DANS candidates:
    sub ← findBestSubClimb(dist, eleBase, c.startIdx, c.endIdx)
    SI sub ≠ null: candidatesWithSub.push(sub)

  // === ÉTAPE 6 : raffinement + FUSION ===
  anchors ← [], refined ← [], lastEnd ← −1, mergeCount ← 0

  POUR chaque c DANS candidatesWithSub:
    candidateLen ← c.length
    effectiveRefine ← clamp(refinePct × candidateLen / 100, 1500, 8000)

    // Sommet
    hiIdx ← findIdxAtDist(dist, dist[c.endIdx] + effectiveRefine)
    newEnd ← argmaxRange(eleBase, c.endIdx, hiIdx)
    SI newEnd ≤ c.startIdx OU newEnd ≤ lastEnd: CONTINUER

    // Creux
    safetyLow ← findIdxAtDist(dist, dist[c.startIdx] − effectiveRefine)
    loIdx ← max(lastEnd + 1, safetyLow)
    newStart ← argminRange(eleBase, loIdx, c.startIdx)
    SI newStart ≥ newEnd: CONTINUER

    // Fusion (v3.0)
    SI refined.length > 0 ET gapMinM > 0:
      prev ← refined[dernier]
      gapM ← dist[newStart] − dist[prev.endIdx]
      SI gapM < gapMinM:
        combinedEnd ← argmaxRange(eleBase, prev.endIdx, newEnd)
        midValley ← argminRange(eleBase, prev.endIdx, newStart)
        newPrevStart ← (eleBase[midValley] < eleBase[prev.startIdx])
                       ? midValley : prev.startIdx
        SI newPrevStart ≠ prev.startIdx:
          anchors[dernier−1] ← midValley

        prev.startIdx = newPrevStart
        prev.endIdx = combinedEnd
        prev._merged += 1
        anchors[dernier] = combinedEnd
        lastEnd = combinedEnd
        mergeCount++
        CONTINUER

    anchors.push(newStart, newEnd)
    refined.push({ startIdx: newStart, endIdx: newEnd,
                   refineM: effectiveRefine, candidateLen,
                   _merged: 0 })
    lastEnd ← newEnd

  // === ÉTAPE 7 : lissage ancré final ===
  eleDisplay ← anchoredMovingAverage(eleRaw, anchors, stepM,
                                     fenetreAffichageM, gaussianSigmaM,
                                     smoothExtra)

  // === ÉTAPE 8 : VALIDATION STRICTE (v3.1) ===
  autoClimbs ← []
  POUR chaque c DANS refined:
    finalStart ← c.startIdx, finalEnd ← c.endIdx
    length ← dist[finalEnd] − dist[finalStart]
    dPlus ← Σ max(0, Δele) sur [finalStart, finalEnd]
    avgSlope ← dPlus / length × 100
    minSlope ← minSlopeForLength(length)

    SI length < 300 OU avgSlope < minSlope:
      sub ← findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx)
      SI sub === null:
        rejeter
      finalStart ← sub.startIdx
      finalEnd ← sub.endIdx
      fallback ← true

    autoClimbs.push({
      startKm: dist[finalStart] / 1000,
      endKm: dist[finalEnd] / 1000,
      refineM: c.refineM,
      candidateLen: c.candidateLen,
      mergedCount: c._merged || 0,
      fallback: (finalStart ≠ c.startIdx) OU (finalEnd ≠ c.endIdx)
    })

  RETOURNER { autoClimbs }
```

### Pipeline complet depuis le chargement

```
FONCTION analyze(content, parameters):
  track ← parseGpx(content)
  n ← length(track.points)

  distRaw ← cumsum(haversine)                        // §1
  eleRawOrig ← track.points.map(p → p.ele)

  rawStats ← computeRawStats(distRaw, eleRawOrig)   // §5

  SI parameters.plateauRun > 0:
    eleForResample ← plateauCorrect(eleRawOrig,      // §3
                                    plateauRun,
                                    tolerance)
  SINON:
    eleForResample ← eleRawOrig

  // [v3.5] Rééchantillonnage par moyenne
  { dist, ele, avgWinM } ← resampleUniform(distRaw,  // §2
                                           eleForResample,
                                           stepM)

  // [v3.6] Anti-pic : conversion m → pts
  eleFinal ← ele
  SI parameters.despikeThresholdM > 0:
    winPts ← max(3, round(despikeWindowM / stepM) | 1)
    { out, count } ← despikeAltitude(ele, winPts,   // §4
                                     despikeThresholdM)
    eleFinal ← out

  { autoClimbs } ← computeAuto(dist, eleFinal, params)  // pipeline ci-dessus

  RETOURNER { track, distRaw, eleRawOrig, dist, eleFinal,
              stepM, avgWinM, rawStats, autoClimbs, despikeCount }
```

### Complexité globale

Soit n = nombre de points, m = nombre de candidats.

| Étape | Complexité |
|---|---|
| Filtre médian | O(n × win log win) |
| Savitzky-Golay | O(n × winSG) |
| Pente lissée | O(n) |
| Hystérésis | O(n) |
| Sous-segments (× m) | O(m × 800²) amorti |
| Raffinement (× m) | O(m × w) |
| Fusion | O(m) |
| Lissage ancré | O(n + m × winDouble) |
| Re-validation | O(m × 800²) |
| **Total** | **O(n × winSG + m × 800²)** |

En pratique, m ≈ 10 et 800² ≈ 640 000, donc ~6 millions d'opérations. Sur un navigateur moderne, **< 1 seconde** pour n = 5 000 points.

---

## 31. Pipeline de reconstruction

### Objet

Recalculer entièrement l'état affiché à partir de `STATE.userClimbs` (état éditable par l'utilisateur). Appelé après chaque édition, insertion, suppression, ou changement de paramètre.

### Pseudocode

```
FONCTION rebuild():
  SI STATE.dist = null OU STATE.eleRaw = null: RETOURNER null

  params ← getParams()

  // === ÉTAPE 1 : Contraintes anti-chevauchement ===
  sorted ← appliquerContraintes(STATE.userClimbs)

  // === ÉTAPE 2 : Ancrage ===
  anchors ← []
  resolved ← []

  POUR chaque c DANS sorted:
    startIdx ← findIdxAtDist(dist, c.startKm × 1000)
    endIdx   ← findIdxAtDist(dist, c.endKm × 1000)

    SI startIdx ≥ endIdx:
      resolved.push({ ..., invalid: true })
      CONTINUER

    anchors.push(startIdx, endIdx)
    resolved.push({ ..., invalid: false })

  // === ÉTAPE 3 : Lissages ===
  eleDisplay ← anchoredMovingAverage(eleRaw, anchors, stepM,
                                     fenetreAffichageM, gaussianSigmaM,
                                     smoothExtra)
  slopeRaw   ← clipSlope(computeSlope(MA(eleRaw, 5), dist), 25)
  slopeSemi  ← computeSlope(eleDisplay, dist)
  slopeDisplay ← clipSlope(MA(slopeSemi, winSlopePts), 25)
  slopeForCat  ← computeSlopeWindow(eleDisplay, dist, fenetreCatM)

  // === ÉTAPE 4 : Métriques par montée ===
  climbs ← []
  POUR chaque { startIdx, endIdx, ref, invalid } DANS resolved:
    SI invalid: climbs.push({ invalid, length: 0, ... })
    SINON:
      dPlus ← Σ max(0, ΔeleDisplay)
      length ← dist[endIdx] − dist[startIdx]
      avgSlope ← dPlus / length × 100
      climbs.push({ ..., length, dPlus, avgSlope,
                    mergedCount: ref.mergedCount || 0,
                    fallback: ref.fallback || false })

  // === ÉTAPE 5 : Segmentation ===
  catSegments ← segmentByCategory(slopeForCat, dist, eleDisplay, longueurMin)

  // === ÉTAPE 6 : Marqueurs ===
  markers ← []
  POUR chaque c DANS climbs (non invalid):
    markers.push({ dist: c.startDist, type: 'valley', climbIdx })
    markers.push({ dist: c.endDist,   type: 'summit', climbIdx })

  RETOURNER { dist, eleRaw, eleDisplay, slopeRaw, slopeDisplay,
              slopeForCat, catSegments, climbs, markers }
```

### Complexité

| Étape | Complexité |
|---|---|
| Contraintes | O(m log m) |
| Ancrage | O(m) + O(m log n) |
| Lissage ancré | O(n + m × winDouble) |
| Pente fenêtrée | O(n × w) |
| Métriques | O(n) |
| Segmentation | O(n + s²) |
| **Total** | **O(n × w)** dominé par la pente fenêtrée |

Avec n = 5 000 et w = 100, ~500 000 opérations → **< 100 ms**.

---

## 32. Tableau récapitulatif des complexités

| Algorithme | Temps | Espace | Fréquence d'appel |
|---|---|---|---|
| Haversine (par paire) | O(1) | O(1) | n−1 fois |
| Distance cumulée | O(n) | O(n) | 1 fois |
| **resampleUniform (moyenne)** | **O(n + m)** | **O(m)** | **1 fois** |
| **plateauCorrect** | **O(n)** | **O(n)** | **1 fois (optionnel)** |
| **despikeAltitude** | **O(n × w × log w)** | **O(w)** | **1 fois** |
| **despikeWindowMToPts** | **O(1)** | **O(1)** | **1 fois par recalcul** |
| **computeRawStats** | **O(n log n)** | **O(n)** | **1 fois** |
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
| **Fusion montées** | **O(m)** | **O(1)** | **m fois** |
| **Validation stricte** | **O(m × 800²)** | **O(m)** | **1 fois** |
| **Diagnostic** | **O(1)** | **O(1)** | **à chaque input** |
| **Insertion contextuelle** | **O(m)** | **O(1)** | **à la demande** |
| **Drag & drop** | **O(points affichés)** | **O(1)** | **par frame** |
| Anti-chevauchement | O(m log m) | O(m) | 1 fois |
| Sous-échantillonnage | O(n) | O(4000) | 1 fois |

Légende :
- n : nombre de points bruts
- m : nombre de points après resampling (≈ n × pas moyen / stepM)
- s : nombre de segments de catégorie
- w : nombre moyen de points dans une fenêtre glissante ou fenêtre anti-pic
- k : nombre d'événements (transitions pente)

---

## 33. Choix de conception et alternatives écartées

### 33.1 Haversine vs Vincenty

| Critère | Haversine | Vincenty |
|---|---|---|
| Précision | ±0.5 % (sphérique) | ±0.001 % (ellipsoïdal) |
| Vitesse | Très rapide | 10× plus lent |
| Complexité | 5 lignes | 30+ lignes |
| **Retenu** | ✅ | ❌ |

### 33.2 Différence centrée vs différence avant

| Critère | Centrée | Avant |
|---|---|---|
| Décalage de phase | Aucun | +1 point |
| Lissage naturel | Oui (3 pts) | Non |
| **Retenu** | ✅ | ❌ |

### 33.3 Pente instantanée vs pente fenêtrée pour la catégorisation

| Critère | Instantanée | Fenêtrée |
|---|---|---|
| Stabilité visuelle | Faible (taches) | Élevée |
| Fidélité locale | Haute | Moyenne |
| Coût calcul | O(n) | O(n × w) |
| **Retenu courbe** | ✅ | ❌ |
| **Retenu couleur** | ❌ | ✅ |

### 33.4 Argmin/argmax vs marche avec tolérance

| Critère | Argmin/argmax | Marche tolérance |
|---|---|---|
| Robustesse au bruit | Élevée | Faible |
| Déterminisme | Toujours | Dépend de la tolérance |
| **Retenu** | ✅ | ❌ |

### 33.5 Garde-fou absolu vs pourcentage vs hybride

| Critère | Absolu (V1) | % pur | **Hybride (v2)** |
|---|---|---|---|
| Simplicité | ✅✅ | ✅ | ⚠️ |
| Adaptabilité à l'échelle | ❌ | ✅ | ✅✅ |
| Prévisibilité | ✅✅ | ⚠️ | ✅ |
| Robustesse cas extrêmes | ✅ | ❌ | ✅✅ |
| **Retenu** | ❌ | ❌ | ✅ |

### 33.6 Édition numérique vs édition graphique (drag & drop)

| Critère | Champs km | Drag & drop |
|---|---|---|
| Précision | ✅✅ | ⚠️ |
| Naturalité | ❌ | ✅✅ |
| Accessibilité clavier | ✅ | ⚠️ (ajoutée) |
| Complexité implémentation | ✅ | ⚠️ |
| **Retenu** | ✅ (conservé) | ✅ (mode principal) |

Les deux modes coexistent.

### 33.7 Interception DOM vs hooks Chart.js pour le drag

| Critère | beforeEvent (Chart.js) | **Capture window (v0.5)** |
|---|---|---|
| Fonctionne avec plugin-zoom | ❌ (conflit) | ✅ |
| Ordre garanti | ❌ | ✅ |
| Complexité | ✅ | ⚠️ |
| **Retenu** | ❌ | ✅ |

### 33.8 Filtres index-based vs distance-based

| Critère | Index-based | **Distance-based (retenu)** |
|---|---|---|
| Simplicité | ✅ | ⚠️ (nécessite resampling) |
| Cohérence sur trace irrégulière | ❌ | ✅ |
| Paramètres en mètres | ❌ | ✅ |
| Prévisibilité | ❌ | ✅ |
| **Retenu** | ❌ | ✅ |

### 33.9 Resampling vs filtres pondérés par distance

| Critère | **Resampling (retenu)** | Filtres pondérés |
|---|---|---|
| Coût | ✅ O(n) une fois | ❌ O(n × w) à chaque filtre |
| Compatibilité avec filtres existants | ✅ Directe | ❌ Réécriture |
| SG compatible | ✅ | ❌ (nécessite LWPR) |
| Doublons GPS | ✅ Absorbés | ⚠️ Pondération à gérer |
| **Retenu** | ✅ | ❌ |

### 33.10 Fusion par distance vs augmentation du seuil

| Critère | **gapMinM (retenu)** | seuilMontee élevé |
|---|---|---|
| Ratage des vraies montées douces | ✅ Évité | ❌ Possible |
| Prévisibilité | ✅ | ⚠️ Dépend de la trace |
| **Retenu** | ✅ | ❌ |

### 33.11 Validation finale : rejet vs repli

| Critère | Rejet sec | **Repli ⤺ (retenu)** |
|---|---|---|
| Perte d'information | ❌ Complète | ✅ Partielle |
| Cohérence avec l'utilisateur | ⚠️ Difficile à expliquer | ✅ Log détaillé |
| **Retenu** | ❌ | ✅ |

### 33.12 Anti-pic Hampel vs médian classique (v3.3)

| Critère | Médian classique | **Hampel (retenu)** |
|---|---|---|
| Modifie le relief normal | Oui | **Non** |
| Élimine un pic isolé | Oui | Oui |
| Préserve les extrema | Non | **Oui** |
| Effet sur les vraies montées | Adoucit | **Aucun** |
| Effet d'écrêtage des bosses | Marqué | **Sélectif** |
| Complexité | O(n × win log win) | O(n × win log win) |
| **Retenu** | ❌ | ✅ |

**Raison** : le filtre Hampel est **sélectif** — il ne remplace que les points qui dévient fortement. Il s'insère dans le pipeline sans affecter les autres étapes.

### 33.13 Anti-pic : seuil en mètres vs en %

| Critère | **Seuil en mètres (retenu)** | Seuil en % |
|---|---|---|
| Prévisibilité | ✅ | ⚠️ |
| Indépendance du relief | ✅ | ❌ |
| **Retenu** | ✅ | ❌ |

**Raison** : un glitch de 20 m reste un glitch de 20 m, qu'il soit sur une plaine ou en montagne.

### 33.14 Anti-pic : position dans le pipeline

| Ordre | Efficacité |
|---|---|
| Avant resampling | ❌ Fenêtre en points varie avec densité |
| **Après resampling (retenu)** | ✅ Fenêtre constante après conversion |
| Après SG | ❌ SG a déjà ancré le glitch dans la courbe |

### 33.15 Diagnostic visuel vs paramètres seuls

| Critère | **Diagnostic (v3.2)** | Paramètres seuls |
|---|---|---|
| Compréhension utilisateur | ✅ Élevée | ❌ Faible |
| Complexité UI | ⚠️ Ajout panneau | ✅ Simple |
| Coût calcul | ✅ O(1) par paramètre | — |
| **Retenu** | ✅ | ❌ |

### 33.16 Rééchantillonnage : interpolation vs moyenne (v3.5)

| Critère | Interpolation | **Moyenne (retenu v3.5)** |
|---|---|---|
| Fidélité au détail brut | Élevée | Moyenne |
| Atténuation bruit haute fréquence | Nulle | **Élevée** |
| Efficacité Hampel en aval | Faible à petit Pas | **Élevée** |
| Cohérence du pipeline selon Pas | Faible | **Élevée** |
| Complexité | O(n + m) | O(n + m) |
| **Retenu** | ❌ | ✅ |

**Raison** : avec l'interpolation linéaire (avant v3.5), plus le Pas est petit, plus on capture de bruit brut haute fréquence. La moyenne agit comme un filtre passe-bas implicite (fenêtre `clamp(stepM, 75, 3 × stepM)`), ce qui rend le comportement du pipeline **cohérent quelle que soit la valeur du Pas**.

### 33.17 Anti-pic : fenêtre en points vs en mètres (v3.6)

| Critère | Fenêtre en points | **Fenêtre en mètres (retenu v3.6)** |
|---|---|---|
| Cohérence avec les autres paramètres | ❌ | ✅ |
| Comportement indépendant du Pas | ❌ | ✅ |
| Prévisibilité pour l'utilisateur | ⚠️ (largeur physique variable) | ✅ |
| Plage utile | 3–15 pts (≈ 75–1500 m selon Pas) | **50–500 m** |
| Conversion nécessaire | Non | Oui (m → points) |
| Complexité | O(1) | O(1) (conversion) |

**Retenu** : fenêtre en mètres, avec conversion interne `winPts = max(3, round(windowM / stepM) | 1)`.

**Raison** : la fenêtre Hampel était le seul paramètre à ne pas être exprimé en mètres. Cela rendait son comportement imprévisible — sa largeur physique variait d'un facteur 40 entre Pas = 5 m et Pas = 200 m. En l'exprimant en mètres, on garantit un comportement identique quelle que soit la trace.

**Exemple concret** : avec l'ancien système à 15 pts (maximum), un utilisateur à Pas = 25 m ne pouvait couvrir que 375 m, alors qu'à Pas = 100 m il pouvait couvrir 1500 m. Cette asymétrie empêchait le réglage intuitif de l'anti-pic.

---

**Fin du document Algorithmes v3.6**