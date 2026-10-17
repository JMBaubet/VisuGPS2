# Document Algorithmes — GPX Climb Analyzer

**Version** : 3.9.2
**Date** : 2026-10-17
**Public** : développeur, mainteneur, auditeur technique
**Portée** : description exhaustive de tous les algorithmes utilisés dans l'application

---

## Table des matières

1. [Distance cumulée (Haversine)](#1-distance-cumulée-haversine)
2. [Rééchantillonnage par moyenne](#2-rééchantillonnage-par-moyenne)
3. [Correction des plateaux GPS/MNT](#3-correction-des-plateaux-gpsmnt)
4. [Anti-pic Hampel (fenêtre en mètres)](#4-anti-pic-hampel)
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
20. [Longueur min. pente (fusion de segments)](#20-longueur-min-pente)
21. [Segmentation par catégorie](#21-segmentation-par-catégorie)
22. [Enrichissement des segments](#22-enrichissement-des-segments)
23. [Contrainte anti-chevauchement](#23-contrainte-anti-chevauchement)
24. [Recherche extrema](#24-recherche-extrema)
25. [Fusion par `gapMinM`](#25-fusion-par-gapminm)
26. [Validation finale stricte](#26-validation-finale-stricte)
27. [Catégorisation non ancrée](#27-catégorisation-non-ancrée)
28. [Construction de la trace carte](#28-construction-de-la-trace-carte)
29. [Marqueurs vallée/sommet (mono et groupe)](#29-marqueurs-vallée-sommet)
30. [Zoom carte sur montée / groupe](#30-zoom-carte)
31. [Séparation d'un groupe](#31-séparation-dun-groupe)
32. [Refusion d'un groupe](#32-refusion-dun-groupe)
33. [Normalisation des groupes](#33-normalisation-des-groupes)
34. [Suppression d'une montée](#34-suppression-dune-montée)
35. [Création manuelle](#35-création-manuelle)
36. [Diagnostic de configuration](#36-diagnostic-de-configuration)
37. [Export JSON](#37-export-json)
38. [Pipeline complet](#38-pipeline-complet)
39. [Pipeline de reconstruction](#39-pipeline-de-reconstruction)
40. [Tableau récapitulatif des complexités](#40-tableau-récapitulatif-des-complexités)
41. [Choix de conception et alternatives écartées](#41-choix-de-conception)

---

## 1. Distance cumulée (Haversine)

### 1.1 Objet

Calculer la distance orthodromique (à la surface d'une sphère) entre deux points GPS successifs, puis la cumuler pour obtenir la distance totale parcourue à chaque point de la trace.

### 1.2 Motivation

Les points GPX sont fournis en coordonnées géographiques (latitude, longitude). Pour analyser une trace, il faut des distances en mètres. La formule de Haversine fournit une distance sphérique précise à 0.5 % près par rapport à l'ellipsoïde WGS84, ce qui est largement suffisant pour des traces de quelques centaines de kilomètres.

### 1.3 Formule

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

### 1.4 Pseudocode

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
  n ← longueur(points)
  dist[0] ← 0
  POUR i DE 1 À n−1:
    d ← haversine(points[i−1].lat, points[i−1].lon,
                  points[i].lat,   points[i].lon)
    dist[i] ← dist[i−1] + d
  RETOURNER dist
```

### 1.5 Complexité

- Temps : **O(n)** sur le nombre de points
- Espace : **O(n)** pour stocker les distances cumulées

### 1.6 Notes

- L'utilisation de `atan2` plutôt que `asin` évite les erreurs numériques pour les faibles distances (quand `a` est proche de 0, `asin(√a)` perd de la précision).
- L'approximation sphérique est suffisante pour des traces de quelques centaines de km.
- Alternative non retenue : formule de Vincenty (ellipsoïde), plus précise (±0.001 %) mais 10× plus lente sans bénéfice pratique.

### 1.7 Choix de conception

| Critère | Haversine | Vincenty |
|---|---|---|
| Précision | ±0.5 % (sphérique) | ±0.001 % (ellipsoïdal) |
| Vitesse | Très rapide | 10× plus lent |
| Complexité | 5 lignes | 30+ lignes |
| **Retenu** | ✅ | ❌ |

---

## 2. Rééchantillonnage par moyenne

### 2.1 Objet

Convertir une série de points irrégulièrement espacés en une série à **pas fixe** (en mètres), en **moyennant** les points bruts tombant dans une fenêtre centrée sur chaque point cible. Cette étape est **fondamentale** : elle garantit que tous les filtres index-based se comportent comme des filtres distance-based, et elle atténue le bruit haute fréquence avant les étapes suivantes.

### 2.2 Motivation

Les traces GPX (terrain ou éditeur) ont une densité locale très variable :
- **Trace GPS** : 3 m en montée raide, 20 m en descente rapide
- **Trace éditeur Openrunner** : 5 m dans les lacets, 800 m en ligne droite

Un filtre `savitzkyGolay(ele, win=60)` appliqué directement :
- En zone dense → couvre 300 m
- En zone clairsemée → couvre **48 000 m**

→ Le **même paramètre produit un effet 100× différent** selon la zone. Le rééchantillonnage élimine ce biais.

De plus, avec l'interpolation linéaire (v3.4 et antérieur), le bruit haute fréquence était conservé tel quel quand le Pas était petit. La **moyenne** (v3.5+) agit comme un filtre passe-bas implicite, ce qui rend le comportement du pipeline **cohérent quelle que soit la valeur du Pas**.

### 2.3 Formule de la fenêtre

```
avgWin = clamp(stepM, RESAMPLE_AVG_MIN_M, 3 × stepM)
```

avec `RESAMPLE_AVG_MIN_M = 75` m.

| Pas | Fenêtre de moyennage | Points moyennés (typique) |
|---|---|---|
| 5 m | 15 m (75 plancher) | ~3 |
| 25 m | 75 m | ~3–4 |
| 50 m | 75 m | ~1.5–2 |
| 100 m | 100 m | ~1 |
| 200 m | 200 m | ~1 |

### 2.4 Pseudocode

```
FONCTION resampleUniform(distRaw, eleRaw, stepM):
  n ← longueur(distRaw)
  totalM ← distRaw[n−1]
  m ← ⌊totalM / stepM⌋ + 1

  avgWin ← clamp(stepM, 75, 3 × stepM)
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

### 2.5 Complexité

- Temps : **O(n + m)** où m = `totalM / stepM`
- Espace : **O(m)**

Le pointeur `startPtr` est monotone (jamais décrémenté), donc le coût est linéaire globalement.

### 2.6 Choix du pas

| Type de trace | Pas conseillé |
|---|---|
| GPS terrain haute fréquence | 15–25 m |
| Trace éditeur (MNT 30 m) | **50 m** |
| Trace très longue (> 100 km) | 75–100 m |

**Défaut v3.9.2** : **50 m**.

### 2.7 Notes importantes

- **`totalDistSampledM = dist[m−1]`** : longueur couverte par la grille
- **`trailingGapM = totalDistRawM − totalDistSampledM`** : queue de trace ignorée (le dernier multiple de `stepM` peut être en deçà du dernier point)
- **Depuis v3.9.2**, la trace affichée sur la carte utilise les **points bruts**, pas la grille. Le `trailingGapM` n'affecte donc plus la fidélité visuelle de la carte, seulement la longueur des analyses.

### 2.8 Avantages par rapport à l'interpolation

- Atténue le bruit haute fréquence dès le rééchantillonnage
- Rend le comportement du pipeline **cohérent quelle que soit la valeur du Pas**
- Améliore l'efficacité du filtre Hampel en aval (moins de points aberrants à corriger)
- Simplifie la configuration : l'utilisateur n'a plus besoin de « compenser » un Pas fin par un filtre médian

### 2.9 Inconvénients

- Légère perte de fidélité sur les très petites variations (< avgWin)
- Sur-échantillonnage possible si `stepM` < pas médian de la trace

### 2.10 Alternatives écartées

| Critère | Interpolation | **Moyenne (retenu v3.5)** |
|---|---|---|
| Fidélité au détail brut | Élevée | Moyenne |
| Atténuation bruit HF | Nulle | **Élevée** |
| Efficacité Hampel en aval | Faible à petit Pas | **Élevée** |
| Cohérence du pipeline selon Pas | Faible | **Élevée** |
| Complexité | O(n + m) | O(n + m) |
| **Retenu** | ❌ | ✅ |

---

## 3. Correction des plateaux GPS/MNT

### 3.1 Objet

Détecter les séries d'altitudes quasi-identiques (typiques des marches d'escalier du MNT SRTM ou des capteurs GPS qui « gèlent ») et les remplacer par une interpolation linéaire.

### 3.2 Motivation

Sur une trace Openrunner/SRTM, les altitudes sont entières : `1211, 1211, 1211, 1210, 1208...`. Tant qu'on reste dans la même cellule MNT (~30 m × 30 m), l'altitude est constante. En sortant, elle saute d'un coup. Ce comportement crée des « marches » artificielles qui perturbent la dérivée (pente) et donc la détection.

### 3.3 Pseudocode

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

### 3.4 Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### 3.5 Notes

- **S'applique avant le rééchantillonnage** (sur les données brutes)
- Optionnel, contrôlé par `plateauRun` (0 = désactivé)
- **Défaut v3.9.2** : `plateauRun = 6`, `tolerance = 1.0 m`
- Après rééchantillonnage à 50 m, les marches MNT de 30 m sont diluées → la correction devient souvent inutile
- **Risque sur les vrais arrêts GPS** : si l'altitude reste constante pendant un arrêt, le filtre va créer à tort une pente artificielle

### 3.6 Choix de conception

| Critère | Correction plateaux | Pas de correction |
|---|---|---|
| Traces SRTM | ✅ Nettoie les marches | ❌ Marches visibles |
| Traces GPS propre | ✅ | ✅ |
| Vrais arrêts GPS | ❌ Crée pente artificielle | ✅ |
| **Retenu** | ✅ (optionnel) | — |

---

## 4. Anti-pic Hampel

### 4.1 Objet

Éliminer les **glitchs d'altitude isolés** (capteur GPS ou artefact MNT) **sans altérer le relief normal**. Le filtre Hampel est un filtre médian **conditionnel** : il ne remplace un point que si son écart à la médiane locale dépasse un seuil.

### 4.2 Motivation

Un glitch isolé (1–2 points déviant de 20–50 m) produit une pente instantanée aberrante (±25 %) qui déclenche une fausse détection de montée. Les filtres classiques :
- **Médiane** : corrige mais lisse tout le relief
- **Savitzky-Golay** : ne corrige pas un pic isolé (ordre 2 le préserve)
- **Gaussien** : dilue mais n'élimine pas

Le filtre Hampel est **chirurgical** : il ne modifie que les points aberrants.

### 4.3 Principe

Pour chaque point `i` :
1. Calculer la **médiane** d'une fenêtre glissante `[i − half, i + half]` où `half = (winPts − 1) / 2`
2. Si `|ele[i] − median| > thresholdM` → remplacer par la médiane
3. Sinon → laisser tel quel

### 4.4 Conversion mètres → points (v3.6)

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

### 4.5 Pseudocode

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

### 4.6 Complexité

- Temps : **O(n × winPts × log winPts)** (dominé par le tri de la fenêtre à chaque point)
- Espace : **O(winPts)** pour le buffer

Pour `winPts = 5`, on trie 5 éléments par point → coût négligeable.

### 4.7 Choix des paramètres

| Paramètre | Défaut | Plage | Signification |
|---|---|---|---|
| `windowM` | 200 | 50–500 | Largeur de la fenêtre en **mètres** |
| `thresholdM` | 15 | 0–100 | Seuil de détection en mètres |

`thresholdM = 0` désactive complètement le filtre.

### 4.8 Table des fenêtres effectives

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

### 4.9 Exemples d'effet

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

**Cas 3 — Progression régulière (préservée)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [1000, 1010, 1020, 1030, 1040, 1050, 1060]
                                ^^^^^ médiane fenêtre = 1020
                                      |1030 − 1020| = 10 < 15 → inchangé
Après :  [1000, 1010, 1020, 1030, 1040, 1050, 1060]
```

**Cas 4 — Chute majeure (corrigée)**
```
Fenêtre = 200 m, seuil = 15 m

Avant :  [500, 510, 520, 530, 2000, 540, 550]
                          ^^^^^^^ glitch majeur (écart = 1470 m)
Après :  [500, 510, 520, 530, 530, 540, 550]
                                ^^^^^ remplacé par la médiane
```

### 4.10 Notes

- Appliqué **après le rééchantillonnage**, **avant tout autre filtrage**
- Les pics de nature large (> winPts) ne sont pas corrigés — c'est voulu
- Un seuil trop bas (< 5 m) peut écrêter des bosses réelles → diagnostic avertit
- Une fenêtre < 2 × Pas rend l'arrondi trop grossier → diagnostic avertit
- **Ordre important** : plateaux → resampling → anti-pic → médiane → SG. Un anti-pic appliqué trop tôt (avant resampling) serait moins efficace car la fenêtre en points varie avec la densité.

### 4.11 Interaction avec les autres filtres

| Filtre | Interaction |
|---|---|
| Filtre médian | **Redondant** — si anti-pic actif, désactiver le filtre médian |
| Correction plateaux | Complémentaire — s'applique avant (données brutes) |
| Savitzky-Golay | Complémentaire — SG traite le bruit diffus, anti-pic traite les glitchs ponctuels |
| Gaussien | Complémentaire — adoucit après anti-pic |

### 4.12 Choix de conception

**Hampel vs médian classique** :

| Critère | Médian classique | **Hampel (retenu)** |
|---|---|---|
| Modifie le relief normal | Oui (lissage global) | **Non** |
| Élimine un pic isolé | Oui | Oui |
| Préserve les extrema | Non | **Oui** |
| Effet sur les vraies montées | Adoucit | **Aucun** |
| Effet d'écrêtage des bosses | Marqué | **Sélectif** |
| Complexité | O(n × win log win) | O(n × win log win) |
| **Retenu** | ❌ | ✅ |

**Seuil en mètres vs en %** :

| Critère | **Seuil en mètres (retenu)** | Seuil en % |
|---|---|---|
| Prévisibilité | ✅ | ⚠️ |
| Indépendance du relief | ✅ | ❌ |
| **Retenu** | ✅ | ❌ |

Un glitch de 20 m reste un glitch de 20 m, qu'il soit sur une plaine ou en montagne. Le seuil en mètres est donc plus prévisible.

**Position dans le pipeline** :

| Ordre | Efficacité |
|---|---|
| Avant resampling | ❌ Fenêtre en points varie avec la densité |
| **Après resampling (retenu)** | ✅ Fenêtre constante après conversion |
| Après SG | ❌ SG a déjà ancré le glitch dans la courbe |

**Fenêtre en points vs en mètres** :

| Critère | Fenêtre en points | **Fenêtre en mètres (retenu v3.6)** |
|---|---|---|
| Cohérence avec autres paramètres | ❌ | ✅ |
| Comportement indépendant du Pas | ❌ | ✅ |
| Prévisibilité utilisateur | ⚠️ (largeur physique variable) | ✅ |
| Plage utile | 3–15 pts (≈ 75–1500 m selon Pas) | **50–500 m** |
| **Retenu** | ❌ | ✅ |

La fenêtre Hampel était le seul paramètre à ne pas être exprimé en mètres. Cela rendait son comportement imprévisible — sa largeur physique variait d'un facteur 40 entre Pas = 5 m et Pas = 200 m. En l'exprimant en mètres, on garantit un comportement identique quelle que soit la trace.

---

## 5. Statistiques de la trace brute

### 5.1 Objet

Calculer les statistiques globales **sur les données GPX d'origine**, en ignorant les doublons GPS.

### 5.2 Motivation

Les cartes de statistiques doivent refléter la **vraie** trace GPX, indépendamment du resampling et de tous les filtres. Un utilisateur qui compare avec Strava ou Garmin veut retrouver les mêmes valeurs.

### 5.3 Pseudocode

```
FONCTION computeRawStats(distRaw, eleRawOrig):
  n ← longueur(distRaw)

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

### 5.4 Complexité

- Temps : **O(n log n)** (dominé par le tri des gaps)
- Espace : **O(n)**

### 5.5 Notes

- Les statistiques sont **calculées une seule fois au chargement**
- Elles sont indépendantes du resampling et des paramètres
- Elles alimentent les cartes statistiques (D+ brut, altitudes min/max)
- Les **doublons GPS** (< 1 m) sont exclus du D+ pour éviter de gonfler artificiellement le dénivelé (certains appareils enregistrent 2–3 points identiques en cas d'arrêt)

---

## 6. Moyenne mobile

### 6.1 Objet

Lisser une série par moyenne glissante centrée. Utilisée pour la pente.

### 6.2 Pseudocode

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

### 6.3 Complexité

- Temps : **O(n)** (grâce au tableau de préfixes)
- Espace : **O(n)**

### 6.4 Notes

- Conversion mètres → points : `winPts = round(winM / stepM)`, forcée impaire
- Après resampling uniforme, une fenêtre de N points couvre **exactement** N × stepM mètres partout
- **Sans le préfix-sum**, on serait en O(n × win). Le préfix-sum ramène à O(n).

### 6.5 Choix de conception

| Critère | Moyenne mobile | Savitzky-Golay |
|---|---|---|
| Lissage | Oui | Oui |
| Préservation des extrema | Non | Oui |
| Vitesse | O(n) | O(n × win) |
| **Usage** | Pente | Altitude |

La moyenne mobile est utilisée pour la pente car on veut un lissage plus agressif, moins soucieux de préserver les extrema (les extrema de la pente sont moins critiques que ceux de l'altitude).

---

## 7. Savitzky-Golay

### 7.1 Objet

Lisser en ajustant localement un polynôme d'ordre 2 sur une fenêtre glissante. **Préserve mieux les extrema** qu'une moyenne mobile de même largeur.

### 7.2 Motivation

Un lissage par moyenne mobile « écrase » les sommets des bosses (les extrema sont arrondis). Un lissage polynomial d'ordre 2 peut reproduire une parabole locale, ce qui préserve la courbure des extrema. C'est crucial pour la détection des sommets.

### 7.3 Formule (ordre 2)

Pour chaque point i, on calcule une combinaison linéaire pondérée des voisins :

```
coefs[k] = 3m² + 3m − 1 − 5k²   pour k ∈ [−m, +m]
```
où `m = (win − 1) / 2`.

Les coefficients sont normalisés en divisant par `Σ coefs`.

### 7.4 Pseudocode

```
FONCTION savitzkyGolay(arr, win, order=2):
  n ← longueur(arr)
  SI win < 5 OU n < 5: RETOURNER copie(arr)
  SI win pair: win ← win + 1

  half ← ⌊win / 2⌋
  m ← half
  POUR k ∈ [−half, +half]:
    coefs[k+half] ← 3m² + 3m − 1 − 5k²

  out ← nouveau tableau(n)
  POUR i DE 0 À n−1:
    sum ← 0, wsum ← 0
    POUR j DE −half À +half:
      idx ← i + j
      SI idx hors bornes: CONTINUER
      c ← coefs[j + half]
      sum  ← sum  + c × arr[idx]
      wsum ← wsum + c
    out[i] ← wsum > 0 ? sum / wsum : arr[i]

  RETOURNER out
```

### 7.5 Complexité

- Temps : **O(n × win)**
- Espace : **O(n)**

### 7.6 Notes

- Utilisé comme **lisseur principal** de l'altitude
- Ordre 2 = préservation des courbures locales, idéal pour les extrema
- Sur données resamplées, `win` en points ≈ `fenetreAffichageM / stepM`
- **Inopérant si `fenetreAffichageM / stepM < 5`** — d'où l'importance du diagnostic de cohérence
- **Recommandé : fenêtre ≥ 10 pts** pour un lissage efficace

### 7.7 Choix de conception

| Critère | Ordre 1 | Ordre 2 | Ordre 3 |
|---|---|---|---|
| Lissage | Oui | Oui | Oui |
| Préservation extrema | Faible | **Forte** | Forte |
| Complexité | O(n × win) | O(n × win) | O(n × win) |
| Stabilité | ✅ | ✅ | ⚠️ (oscillations) |
| **Retenu** | ❌ | ✅ | ❌ |

L'ordre 2 est le meilleur compromis entre lissage et préservation des extrema.

---

## 8. Lissage gaussien

### 8.1 Objet

Convolution avec un noyau gaussien tronqué à ±3σ. Lisse fortement les transitions.

### 8.2 Motivation

Après le SG, le profil peut encore avoir des transitions anguleuses. Le gaussien adoucit ces transitions de manière symétrique et isotrope (contrairement à un SG qui est polynomial).

### 8.3 Pseudocode

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
    out[i] ← wsum > 0 ? sum / wsum : arr[i]

  RETOURNER out
```

### 8.4 Complexité

- Temps : **O(n × σ)**
- Espace : **O(n)**

### 8.5 Notes

- Appliqué **après le Savitzky-Golay, avant l'ancrage**
- σ en mètres converti en σ en points : `sigmaPts = sigmaM / stepM`
- Défaut v3.9.2 : σ = 75 m (≈ 1.5 point à stepM=50)
- **Inopérant si σM / stepM < 0.5** — d'où le warning diagnostic

### 8.6 Choix de conception

| Critère | Gaussien | SG | Moyenne mobile |
|---|---|---|---|
| Symétrie | ✅ | ✅ | ✅ |
| Effet passe-bas | Fort | Moyen | Moyen |
| Coût | O(n × σ) | O(n × win) | O(n) |
| Effet sur les extrema | Écrase | Préserve | Écrase |
| **Usage** | Lissage final | Altitude principale | Pente |

Le gaussien est complémentaire du SG : SG traite le relief principal, gaussien adoucit les transitions.

---

## 9. Filtre médian

### 9.1 Objet

Remplacer chaque valeur par la médiane de son voisinage. Élimine les pics isolés.

### 9.2 Pseudocode

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

### 9.3 Complexité

- Temps : **O(n × win log win)**
- Espace : **O(win)**

### 9.4 Notes

- **Première étape** du pipeline de filtrage (avant SG)
- Défaut v3.9.2 : `medianWindowM = 0` (désactivé)
- **Depuis v3.3, l'anti-pic Hampel rend le filtre médian largement redondant**. Recommandation : le désactiver si anti-pic actif.

### 9.5 Choix de conception

| Critère | Médian | Hampel |
|---|---|---|
| Corrige pics | Oui | Oui |
| Préserve relief normal | Non | **Oui** |
| **Retenu** | ❌ | ✅ |

---

## 10. Lissage ancré

### 10.1 Objet

Lisser en forçant la courbe à passer **exactement** par certains points (les extrema détectés). Résout le problème du décalage du sommet vers la descente.

### 10.2 Motivation

Un lissage SG normal « écrase » les sommets et déplace le maximum détecté vers la droite (dans le sens de la descente). Cela crée un décalage visuel entre la position du marqueur « sommet » et le pic visuel de la courbe. Le lissage ancré force la courbe à passer exactement par les bornes calculées.

### 10.3 Principe

1. Calculer un lissage de base (SG + gaussien)
2. Pour chaque ancre `a`, calculer `corr[a] = raw[a] − base[a]`
3. Appliquer une correction linéaire par morceaux entre ancres

### 10.4 Pseudocode

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

### 10.5 Complexité

- Temps : **O(n + m × winDouble)** où m = nb d'ancres
- Espace : **O(n)**

### 10.6 Propriétés mathématiques

- **Exactitude aux ancres** : `result[a] = base[a] + corrA = raw[a]` par construction
- **Continuité** : entre deux ancres, la correction est une interpolation linéaire, donc continue
- **Lissage préservé** : la courbe reste lisse à l'intérieur de chaque intervalle (la correction est linéaire, donc n'introduit pas d'oscillation)

### 10.7 Notes

- **Effet visuel** : la pente devient exactement nulle au sommet d'une ancre, ce qui résout le problème « la pente continue de monter après le sommet »
- Le choix des ancres est crucial. Si elles sont placées à chaque changement de catégorie, la courbe est très contrainte
- **v3.8.1** : la courbe ancrée (`eleDisplay`) est utilisée pour l'**affichage** mais **pas** pour la catégorisation (voir §27)

### 10.8 Choix de conception

| Critère | SG simple | **Ancrage (retenu)** |
|---|---|---|
| Extrema bien placés | Non | **Oui** |
| Courbe lisse | Oui | Oui (par morceaux) |
| Complexité | O(n × win) | O(n + m) |
| **Retenu** | ❌ | ✅ |

---

## 11. Pente instantanée

### 11.1 Objet

Calculer la pente locale à chaque point, en % (variation d'altitude / distance).

### 11.2 Formule

Pour le point `i`, on utilise les points voisins `i−1` et `i+1` :

```
pente[i] = (ele[i+1] − ele[i−1]) / (dist[i+1] − dist[i−1]) × 100
```

### 11.3 Pseudocode

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

### 11.4 Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### 11.5 Notes

- `minD = 0.05 m` évite les divisions par zéro quand deux points sont trop proches
- **Différence centrée** plutôt que différence avant : réduit le décalage de phase
- Très sensible au bruit GPS — d'où l'importance de l'anti-pic en amont

### 11.6 Choix de conception

| Critère | Différence centrée | Différence avant |
|---|---|---|
| Décalage de phase | Aucun | +1 point |
| Lissage naturel | Oui (3 pts) | Non |
| **Retenu** | ✅ | ❌ |

---

## 12. Fenêtre de calcul pente

### 12.1 Objet

Calculer la pente **moyenne sur une portion de N mètres** centrée sur chaque point. Utilisée pour la catégorisation par couleur.

### 12.2 Pseudocode

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

### 12.3 Complexité

- Temps : **O(n × w)** où w = nombre moyen de points dans la fenêtre
- Espace : **O(n)**

### 12.4 Notes

- **Fenêtre effective** entre `winM` et `2 × winM` selon l'alignement
- Après resampling à 50 m, w ≈ 2 pour winM=100 m
- **v3.8.1** : la pente fenêtrée est calculée sur `eleBaseCat` (altitude **non ancrée**), pas sur `eleDisplay` (voir §27)

### 12.5 Choix de conception

| Critère | **Fenêtrée (retenu)** | Instantanée |
|---|---|---|
| Stabilité visuelle | Élevée | Faible (patchwork) |
| Fidélité locale | Moyenne | Haute |
| Coût | O(n × w) | O(n) |
| **Retenu couleur** | ✅ | ❌ |

---

## 13. Écrêtage des pentes

### 13.1 Objet

Limiter les pentes affichées à ±25 % pour éviter les pics verticaux dus au bruit.

### 13.2 Pseudocode

```
FONCTION clipSlope(slope, maxPct=25):
  out ← tableau(|slope|)
  POUR i DE 0 À |slope|−1:
    v ← slope[i]
    out[i] ← max(−maxPct, min(maxPct, v))
  RETOURNER out
```

### 13.3 Complexité

- Temps : **O(n)**
- Espace : **O(n)**

### 13.4 Notes

- Constante `SLOPE_DISPLAY_MAX_PCT = 25`
- Utilisé pour : `slopeBase`, `slopeRaw`, `slopeDisplay`
- L'axe Y du graphe de pente est fixé à ±27 % pour cohérence
- **N'affecte PAS la détection** — c'est un écrêtage affichage uniquement

---

## 14. Détection par hystérésis

### 14.1 Objet

Détecter les portions de montée avec **hystérésis** (seuil d'entrée positif, seuil de sortie négatif).

### 14.2 Motivation

Sans hystérésis, si la pente oscille autour de 0 %, on aurait des micro-événements en continu. L'hystérésis utilise deux seuils distincts pour éviter ces oscillations.

### 14.3 Pseudocode

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

### 14.4 Complexité

- Temps : **O(n)**
- Espace : **O(k)** (k = nb d'événements)

### 14.5 Notes

- Seuil par défaut : 0.5 %
- Hystérésis (+0.5 / −0.5) évite les oscillations autour du seuil
- **Peut fragmenter une montée** si un faux-plat descend temporairement sous −0.5 % → d'où la fusion (§25)

### 14.6 Choix de conception

| Critère | Hystérésis | Seuil unique |
|---|---|---|
| Oscillations autour de 0 | Évitées | Fréquentes |
| Fragmentation | Possible | Plus fréquente |
| **Retenu** | ✅ | ❌ |

---

## 15. Construction des candidats

### 15.1 Objet

Transformer la liste d'événements `{start, summit}` en une liste de segments candidats (type `climb` ou `non-climb`).

### 15.2 Pseudocode

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

### 15.3 Complexité

- Temps : **O(k)**
- Espace : **O(k)**

### 15.4 Notes

- Le premier événement peut être un `summit` (trace qui commence en montée)
- Le dernier événement peut être un `start` non fermé (trace qui finit en montée) — traité par le `if (inC)` de `detectEvents`

---

## 16. Sous-segment optimal

### 16.1 Objet

Dans un candidat `[startIdx, endIdx]`, trouver le **plus long sous-segment** qui satisfait les critères (longueur, pente moyenne). C'est l'algorithme de validation.

### 16.2 Motivation

Un candidat brut peut inclure des portions plates en début/fin. La validation stricte (§26) exige que la pente moyenne sur l'ensemble du segment soit ≥ seuil. Or cette contrainte peut être violée si on inclut des zones plates. On cherche donc le **plus long** sous-segment qui passe.

### 16.3 Critères de validation

```
seuil(L) = 3 % si L ≥ 1000 m
         = 4 % si 750 ≤ L < 1000
         = 5 % si 500 ≤ L < 750
         = 6 % si 400 ≤ L < 500
         = 7 % si 300 ≤ L < 400
         = ∞   sinon (rejeté)
```

### 16.4 Pseudocode

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

### 16.5 Complexité

- Sans sous-échantillonnage : **O(n²)**
- Avec sous-échantillonnage (step adaptatif) : **O(800²) ≈ 640 000** opérations max par candidat
- Espace : **O(n)**

### 16.6 Notes

- La stratégie privilégie le **plus long** sous-segment valide
- Cette fonction est appelée **deux fois** par candidat : sur `eleBase` (validation intermédiaire) et sur `eleDisplay` (validation finale)
- Si aucun sous-segment valide n'existe, la fonction retourne `null` et la montée est rejetée
- **Le sous-échantillonnage à 800** limite la complexité à ~640k opérations, acceptable en JavaScript (< 1 ms par appel sur navigateur moderne)

### 16.7 Choix de conception

| Critère | Recherche exhaustive | **Sous-échantillonnage (retenu)** |
|---|---|---|
| Précision | Parfaite | ~99 % |
| Complexité | O(n²) | O(800²) |
| Cas n=10000 | 100M ops (~500 ms) | 640k ops (~2 ms) |
| **Retenu** | ❌ | ✅ |

---

## 17. Argmin / Argmax

### 17.1 Objet

Trouver l'indice du **minimum** (resp. maximum) d'un tableau numérique dans un intervalle `[lo, hi]`.

### 17.2 Pseudocode

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

### 17.3 Complexité

- Temps : **O(hi − lo + 1)**
- Espace : **O(1)**

### 17.4 Notes

- Utilisé pour le raffinement (§24) et la fusion (§25)
- Robuste au bruit comparé à une marche avec tolérance (qui dépend d'un seuil)

### 17.5 Choix de conception

| Critère | **Argmin/argmax (retenu)** | Marche tolérance |
|---|---|---|
| Robustesse au bruit | Élevée | Faible |
| Déterminisme | Toujours | Dépend de la tolérance |
| **Retenu** | ✅ | ❌ |

---

## 18. Recherche binaire sur distance

### 18.1 Objet

Trouver l'indice du **premier point** dont la distance cumulée est ≥ une valeur cible.

### 18.2 Pseudocode

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

### 18.3 Complexité

- Temps : **O(log n)**
- Espace : **O(1)**

### 18.4 Notes

- Appelé massivement dans `computeAuto` (raffinement, fusion) et `rebuild`
- Sur n=10000, log₂(10000) ≈ 14 comparaisons par appel
- **Pas d'interpolation**, on retourne toujours un indice valide

---

## 19. Catégorisation par pente

### 19.1 Objet

Associer une **catégorie de pente** à une valeur de pente en %.

### 19.2 Grille (8 catégories)

| ID | Libellé | Plage |
|---|---|---|
| dh3 | Descente raide | p < −7 |
| dh2 | Descente | −7 ≤ p < −3 |
| dh1 | Faux plat descendant | −3 ≤ p < −1 |
| pl | Plat | −1 ≤ p < +1 |
| up1 | Faux plat montant | +1 ≤ p < +3 |
| up2 | Montée | +3 ≤ p < +7 |
| up3 | Montée raide | +7 ≤ p < +10 |
| up4 | Montée très raide | p ≥ +10 |

### 19.3 Pseudocode

```
FONCTION categorize(slope):
  POUR chaque cat DANS CATEGORIES (dans l'ordre):
    SI slope ≥ cat.min ET slope < cat.max: RETOURNER cat.id
  RETOURNER 'pl'
```

### 19.4 Complexité

- Temps : **O(c)** où c = 8 catégories. En pratique constant → **O(1)**
- Espace : **O(1)**

### 19.5 Notes

- Les bornes utilisent `<` en borne haute : une pente de +3 % exactement tombe dans `up1`, pas dans `up2`
- La symétrie descente/montée n'est pas parfaite (les seuils sont −7/−3/−1 vs +1/+3/+7/+10)

### 19.6 Évolution V4 — rendre éditables

```typescript
export interface EditableSlopeThresholds {
  descenteRaide: number;       // défaut 7
  descente: number;            // défaut 3
  fauxPlat: number;            // défaut 1
  montee: number;              // défaut 3
  monteeRaide: number;         // défaut 7
  monteeTresRaide: number;     // défaut 10
}
```

---

## 20. Longueur min. pente

### 20.1 Objet

Supprimer les segments dont la longueur est inférieure à un seuil (par défaut 200 m) en les absorbant dans leur **voisin le plus long**.

### 20.2 Motivation

Sans cette étape, la bande de segmentation serait un patchwork illisible (des micro-taches de 10 m de large). L'utilisateur veut voir les grandes tendances, pas chaque fluctuation.

### 20.3 Algorithme (2 passes)

1. **Fusion itérative** : tant qu'il existe un segment trop court, l'absorber dans son voisin le plus long.
2. **Combinaison** : recoller les segments adjacents de même catégorie.

### 20.4 Pseudocode

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

### 20.5 Complexité

- Temps : **O(s²)** dans le pire cas, **O(s)** en pratique (s = nombre de segments bruts)
- Espace : **O(s)**

### 20.6 Notes

- La fusion « vers le voisin le plus long » évite de créer des déséquilibres (un segment de 10 m absorbé par un de 50 m plutôt que par un de 3000 m)
- **Le point frontière** (là où la catégorie change) appartient aux deux segments adjacents — voir §21
- **Défaut v3.9.2** : `longueurMin = 200 m`

---

## 21. Segmentation par catégorie

### 21.1 Objet

Découper la trace en segments contigus selon la catégorie de pente de chaque point, **sans laisser de zone blanche**.

### 21.2 Principe clé

Le **point frontière** (là où la catégorie change) appartient à la fois au segment précédent et au segment suivant. Cela garantit qu'il n'y a jamais de « trou » entre deux segments de couleur différente sur la carte.

### 21.3 Pseudocode

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

### 21.4 Complexité

- Temps : **O(n)** pour la catégorisation + **O(s²)** pour la fusion
- Espace : **O(n)**

### 21.5 Notes

- La somme des longueurs des segments après fusion **égale exactement la distance totale**
- C'est cette liste (`catSegments`) qui est utilisée à la fois par la carte et par le widget profil, garantissant leur **cohérence visuelle**

### 21.6 Choix de conception

| Critère | Zones blanches | **Point frontière partagé (retenu)** |
|---|---|---|
| Cohérence visuelle | Mauvaise | **Bonne** |
| Somme des longueurs | < total | **= total** |
| Complexité | O(n) | O(n) |
| **Retenu** | ❌ | ✅ |

---

## 22. Enrichissement des segments

### 22.1 Objet

Calculer les métriques de chaque segment (D+, pente moyenne, altitudes).

### 22.2 Pseudocode

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

### 22.3 Complexité

- Temps : **O(len)**
- Espace : **O(1)**

### 22.4 Notes

- La **pente moyenne** est pondérée par la distance, pas par le nombre de points (les points sont déjà à pas fixe après resampling, donc c'est équivalent — mais la pondération par la distance reste plus précise en cas d'irrégularité résiduelle)

---

## 23. Contrainte anti-chevauchement

### 23.1 Objet

Garantir que les montées éditées manuellement ne se chevauchent jamais.

### 23.2 Pseudocode (édition numérique)

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

### 23.3 Complexité

- Temps : **O(m log m)** pour le tri + **O(m)** pour la contrainte
- Espace : **O(m)**

### 23.4 Application temps réel pendant le drag

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

### 23.5 Notes

- Les indicateurs `_constrainedStart` et `_constrainedEnd` sont affichés avec `⚠` orange dans la liste
- La contrainte est **appliquée à chaque rebuild**, garantissant la cohérence même après édition numérique des champs
- Pendant le drag, la contrainte est appliquée **en temps réel** par `applyDragConstraint`

---

## 24. Recherche extrema

### 24.1 Objet

Limiter la distance maximale de recherche de la vraie vallée / du vrai sommet lors de l'étape de raffinement.

### 24.2 Motivation

Après le sous-segment optimal, on a une bonne estimation des bornes. Mais le vrai creux/sommet peut être légèrement décalé. On cherche dans une fenêtre autour des bornes estimées, avec un rayon proportionnel à la longueur (les longues montées ont plus de latitude).

### 24.3 Formule

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

### 24.4 Table d'exemples

| Longueur candidat | refinePct=60% | Après clamp |
|---|---|---|
| 300 m | 180 m | **1500 m** |
| 1 km | 600 m | **1500 m** |
| 3 km | 1800 m | **1800 m** |
| 6 km | 3600 m | **3600 m** |
| 10 km | 6000 m | **6000 m** |
| 20 km | 12000 m | **8000 m** |

### 24.5 Application

```
hiIdx ← findIdxAtDist(dist, min(dist[n−1], dist[c.endIdx] + effectiveRefine))
newEnd ← argmaxRange(eleBase, c.endIdx, hiIdx)

safetyLow ← findIdxAtDist(dist, max(0, dist[c.startIdx] − effectiveRefine))
loIdx ← max(lastEnd + 1, safetyLow)
newStart ← argminRange(eleBase, loIdx, c.startIdx)
```

### 24.6 Complexité

- Temps : **O(1)** pour le clamp + **O(range)** pour l'argmin/argmax
- Espace : **O(1)**

### 24.7 Notes

- **Adaptatif** : les longues montées ont plus de latitude pour chercher leur vrai sommet/creux
- **Borné** : les montées courtes ne partent pas chercher un sommet à 10 km
- La valeur effective est affichée par montée dans la colonne Extrema
- **`lastEnd + 1`** : garantit qu'une montée ne peut pas empiéter sur la précédente

### 24.8 Choix de conception

| Critère | **Hybride (retenu)** | Absolu | % pur |
|---|---|---|---|
| Simplicité | ⚠️ | ✅✅ | ✅ |
| Adaptabilité à l'échelle | ✅✅ | ❌ | ✅ |
| Prévisibilité | ✅ | ✅✅ | ⚠️ |
| Robustesse cas extrêmes | ✅✅ | ✅ | ❌ |
| **Retenu** | ✅ | ❌ | ❌ |

---

## 25. Fusion par `gapMinM`

### 25.1 Objet

Fusionner deux montées détectées séparées de moins de `gapMinM`. Corrige la fragmentation due aux faux-plats dans une montée continue.

### 25.2 Motivation

L'hystérésis (§14) ferme une montée dès que la pente descend sous −seuil. Si un faux-plat traverse une longue montée, on obtient 2 montées artificielles. La fusion recolle ces fragments.

### 25.3 Principe

Après le raffinement de chaque candidat, on compare avec la montée précédente :
- Si `dist[newStart] − dist[prev.endIdx] < gapMinM` → fusion
- Le nouveau sommet = **le plus haut** des deux
- Le nouveau creux = **le plus bas** entre les deux (si plus bas que le creux actuel)
- **v3.9.0** : les bornes des sous-montées sont conservées dans `_mergedParts`

### 25.4 Pseudocode

```
// Dans la boucle de raffinement
SI refined.length > 0 ET gapMinM > 0:
  prev ← refined[dernier]
  gapM ← dist[newStart] − dist[prev.endIdx]

  SI gapM < gapMinM:
    // ─── Conservation des bornes de la sous-montée initiale ───
    SI !prev._mergedParts:
      prev._mergedParts ← [{
        startKm: dist[prev.startIdx] / 1000,
        endKm: dist[prev.endIdx] / 1000
      }]

    // ─── Calcul des nouvelles bornes ───
    combinedEnd ← argmaxRange(eleBase, prev.endIdx, newEnd)

    midValley ← argminRange(eleBase, prev.endIdx, newStart)
    newPrevStart ← prev.startIdx
    SI eleBase[midValley] < eleBase[prev.startIdx]:
      newPrevStart ← midValley
      anchors[dernier − 1] ← midValley    // maj ancre vallée

    // ─── Ajout de la nouvelle sous-montée ───
    prev._mergedParts.push({
      startKm: dist[newStart] / 1000,
      endKm: dist[newEnd] / 1000
    })

    // ─── Mise à jour de la montée fusionnée ───
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
refined.push({ startIdx: newStart, endIdx: newEnd, ..., _merged: 0, _mergedParts: null })
lastEnd ← newEnd
```

### 25.5 Complexité

- Temps : **O(m)** pour l'ensemble des fusions
- Espace : **O(k)** pour les `_mergedParts` (k = nombre de sous-montées dans une fusion)

### 25.6 Notes

- `gapMinM = 0` → désactive complètement
- **Défaut v3.9.2** : 250 m
- Le compteur `mergedCount` est affiché avec l'indicateur `⊕`
- **Conséquence** : les bornes finales peuvent avoir une pente moyenne plus faible que la montée initiale → d'où la validation stricte (§26)
- **`_mergedParts`** : le **mécanisme clé de la séparation** ultérieure (§31). Les bornes conservées sont celles **avant** extension par `argminRange`/`argmaxRange`, ce qui permet de retrouver les segments réels

### 25.7 Choix de conception

| Critère | **gapMinM (retenu)** | Seuil élevé |
|---|---|---|
| Ratage des vraies montées douces | ✅ Évité | ❌ Possible |
| Prévisibilité | ✅ | ⚠️ Dépend de la trace |
| **Retenu** | ✅ | ❌ |

---

## 26. Validation finale stricte

### 26.1 Objet

Garantir qu'**aucune montée affichée n'est sous son seuil de pente**, même après fusion et raffinement.

### 26.2 Le bug corrigé (v3.1)

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

### 26.3 Le correctif

Validation **sur les bornes finales** d'abord, puis repli sur sous-segment si échec.

### 26.4 Pseudocode

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
      mergedParts: c._mergedParts || null,      // ← v3.9.0 : préservation
      fallback: (finalStart ≠ c.startIdx) OU (finalEnd ≠ c.endIdx)
    })

  RETOURNER autoClimbs
```

### 26.5 Complexité

- Temps : **O(m × 800²)** (dominé par les appels à `findBestSubClimb` sur échec)
- Espace : **O(m)**

### 26.6 Notes

- L'indicateur `⤺` signale une montée dont les bornes ont été repliées
- Le log debug trace chaque repli en détail
- Garantit l'invariant : **pour toute montée affichée, `avgSlope ≥ seuil(length)`**
- **v3.9.0** : `_mergedParts` est préservé à travers la validation, pour permettre la séparation ultérieure

### 26.7 Choix de conception

| Critère | Rejet sec | **Repli (retenu)** |
|---|---|---|
| Perte d'information | Complète | **Partielle** |
| Cohérence avec l'utilisateur | Difficile à expliquer | **Log détaillé** |
| **Retenu** | ❌ | ✅ |

---

## 27. Catégorisation non ancrée

### 27.1 Objet

Résoudre deux bugs visuels majeurs de la v3.8 :
1. Les couleurs du profil ne correspondaient pas à celles de la carte
2. Les couleurs changeaient pendant le drag des poignées

### 27.2 Diagnostic

**Bug 1 : couleurs profil ≠ couleurs carte**
- La carte utilisait `catSegments` (segments **fusionnés** par `longueurMin`)
- Le profil coloriait point par point avec `categorize(slopeForCat[i])` — la catégorie **brute**, sans fusion
- Un mini-segment de 100 m de up1 entre deux up2 était fondu en up2 sur la carte, mais restait up1 sur le profil

**Bug 2 : couleurs qui changent au drag**
- `slopeForCat` était calculée sur `eleDisplay` (courbe **ancrée**)
- Déplacer une poignée modifiait `eleDisplay` sur toute la zone de lissage
- Les pentes changeaient → les catégories basculaient sur des tronçons entiers → clignotement colorimétrique

### 27.3 Solution

```
// Dans rebuild() :
eleDisplay ← anchoredMovingAverage(...)   // courbe affichée (ancrée)

// Au lieu d'utiliser eleDisplay pour la catégorisation :
eleBaseCat ← savitzkyGolay(eleRaw, winSGCatPts, 2)   // NON ancrée
slopeForCat ← computeSlopeWindow(eleBaseCat, dist, fenetreCatM)
catSegments ← segmentByCategory(slopeForCat, dist, eleDisplay, longueurMin)
```

### 27.4 Propriétés

- **Indépendance au drag** : les ancres (positions des poignées) n'entrent pas dans `eleBaseCat` → la catégorisation ne change pas quand on déplace une poignée
- **Cohérence carte/profil** : les deux utilisent `catSegments` (le même tableau)
- **Courbe affichée inchangée** : on continue d'afficher `eleDisplay` (ancrée, jolie)

### 27.5 Coût

Un Savitzky-Golay supplémentaire (`savitzkyGolay(eleRaw, winSGCatPts, 2)`) à chaque `rebuild()`.

- Complexité : **O(n × winSGCat)**
- Impact pratique : ~5 ms sur une trace de 10 000 points

### 27.6 Deux usages de `fenetreAffichageM`

| Usage | Altitude utilisée | Pourquoi |
|---|---|---|
| Courbe affichée | `eleDisplay` (ancrée) | Extrema bien placés |
| Catégorisation | `eleBaseCat` (non ancrée) | Stabilité au drag |

Le paramètre `fenetreAffichageM` pilote les **deux** (même fenêtre de lissage), mais appliqué à deux altitudes distinctes.

### 27.7 Choix de conception

| Critère | Ancré | **Non ancré (retenu v3.8.1)** |
|---|---|---|
| Cohérence carte/profil | ❌ Divergence | ✅ Identique |
| Stabilité au drag | ❌ Couleurs changent | ✅ Stables |
| Coût | ✅ | ⚠️ +1 SG |
| **Retenu** | ❌ | ✅ |

---

## 28. Construction de la trace carte

### 28.1 Objet

Créer une `FeatureCollection` GeoJSON pour Mapbox, avec **une `LineString` par segment de catégorie**, en utilisant les **points bruts du GPX** (v3.9.2).

### 28.2 Le bug corrigé (v3.9.2)

**Avant v3.9.2** : `latLonAtDist(STATE.dist[i])` interpolait sur la **grille resamplée** (tous les 50 m). La trace affichée était une version lissée du tracé, avec les petits lacets gommés.

**Après v3.9.2** : itération sur `STATE.rawPoints`, ce qui préserve toute la fidélité du fichier GPX.

### 28.3 Algorithme

```
FONCTION buildTraceFeatures():
  SI !data OU !rawPoints OU !distRaw: RETOURNER vide

  catSegments ← data.catSegments
  n ← rawPoints.length

  // ─── Étape 1 : catégoriser chaque point brut ───
  catOf ← new Array(n)
  segIdx ← 0
  POUR i DE 0 À n−1:
    d ← distRaw[i]
    // Avancer/reculer dans catSegments (monotone)
    TANT QUE segIdx < |catSegments|−1 ET d > catSegments[segIdx].endDist:
      segIdx ← segIdx + 1
    TANT QUE segIdx > 0 ET d < catSegments[segIdx].startDist:
      segIdx ← segIdx − 1
    catOf[i] ← catSegments[segIdx].key

  // ─── Étape 2 : grouper les points consécutifs de même catégorie ───
  features ← []
  startIdx ← 0
  POUR i DE 1 À n:
    SI i = n OU catOf[i] ≠ catOf[startIdx]:
      endInc ← (i < n) ? i : n−1   // inclure le point de transition
      coords ← [[lon, lat] pour chaque point de startIdx à endInc]
      SI |coords| ≥ 2:
        features.push({LineString, color: CAT_BY_ID[key].color})
      startIdx ← i

  RETOURNER {FeatureCollection, features}
```

### 28.4 Complexité

- Temps : **O(n)** (deux passes linéaires)
- Espace : **O(n)** pour `catOf` et les coordonnées

### 28.5 Points clés

- **Utilise `rawPoints`** (pas la grille resamplée) → la trace suit exactement le GPX
- **Continuité visuelle** : le point frontière entre deux segments est inclus dans les deux `LineString`
- **`distRaw[i]`** : distance cumulée de chaque point brut, calculée au chargement
- **Parcours monotone de `catSegments`** : `segIdx` ne recule que si nécessaire (traces strictement croissantes en distance)

### 28.6 Historique

- **v3.8 → v3.9.1** : la fonction utilisait `latLonAtDist(STATE.dist[i])` → interpolait sur la grille resamplée (perte de fidélité)
- **v3.9.2** : utilisation directe de `rawPoints` + catégorisation par distance cumulée

### 28.7 Choix de conception

| Critère | **Points bruts (retenu v3.9.2)** | Grille resamplée |
|---|---|---|
| Fidélité au GPX | ✅ | ❌ |
| Indépendance au Pas | ✅ | ❌ |
| Détail des lacets | ✅ | ⚠️ Gomme les petits |
| Coût | O(n) | O(m) (m < n) |
| **Retenu** | ✅ | ❌ |

---

## 29. Marqueurs vallée/sommet

### 29.1 Objet

Afficher sur la carte les points remarquables (vallée et sommet) de la montée sélectionnée.

### 29.2 Comportement

**Si la montée sélectionnée n'appartient pas à un groupe** :
- 2 marqueurs (un vert `valley`, un rouge `summit`)

**Si la montée appartient à un groupe** (v3.9.2) :
- **Tous** les points vallée/sommet de **tous** les membres du groupe sont affichés
- Le membre actif a `role: 'selected'` (rayon 10, contour 4)
- Les autres ont `role: 'member'` (rayon 8, contour 2.5)

### 29.3 Pseudocode

```
FONCTION updateMapMarkers():
  idx ← STATE.selectedClimbIdx
  SI idx = null: setData(vide); RETOURNER

  selected ← userClimbs[idx]
  gid ← selected.groupId

  SI gid != null:
    members ← userClimbs.filter(c → c.groupId = gid)
    POUR chaque m DANS members:
      isSelf ← (m === selected)
      ajouter valley (startKm), role = isSelf ? 'selected' : 'member'
      ajouter summit (endKm),   role = isSelf ? 'selected' : 'member'
  SINON:
    ajouter valley (selected.startKm), role='selected'
    ajouter summit (selected.endKm),   role='selected'
```

### 29.4 Style Mapbox

```json
"circle-radius": ["match", ["get", "role"], "selected", 10, 8],
"circle-color": ["match", ["get", "kind"], "valley", "#16a34a", "summit", "#dc2626", "#888"],
"circle-stroke-width": ["match", ["get", "role"], "selected", 4, 2.5]
```

### 29.5 Position des marqueurs

Via `latLonAtDist(km × 1000)`, qui interpole sur `distRaw`/`rawPoints`. Les marqueurs peuvent donc être positionnés **n'importe où** entre deux points bruts — nécessaire pendant le drag (la poignée peut être à 0.001 km près).

### 29.6 Complexité

- Temps : **O(m_groupe)** où m_groupe = nombre de membres du groupe
- Espace : **O(m_groupe)**

### 29.7 Notes

- **Le marqueur du membre actif est visuellement distinct** (plus grand, contour plus épais), pour que l'utilisateur sache quel segment il édite
- **Les autres membres du groupe** permettent de visualiser les frontières entre segments (où finit le segment #N.1, où commence le #N.2)
- **Avant v3.9.2** : seuls les 2 marqueurs de la montée sélectionnée étaient affichés, ce qui rendait la frontière invisible quand on éditait un membre de groupe

### 29.8 Choix de conception

| Critère | Mono-marqueur | **Multi-marqueurs (retenu v3.9.2)** |
|---|---|---|
| Visualisation frontières | ❌ | ✅ |
| Clarté pour groupe | ❌ | ✅ |
| Charge visuelle | Faible | ⚠️ Plus élevée |
| **Retenu** | ❌ | ✅ (pour groupes) |

---

## 30. Zoom carte

### 30.1 Objet

Recentrer la caméra Mapbox sur une montée ou un groupe.

### 30.2 Fonction `zoomMapToClimb`

```
FONCTION zoomMapToClimb(climb, duration):
  SI climb.groupId != null:
    RETOURNER zoomMapToGroup(climb.groupId, duration)

  startM ← (climb.startKm != null) ? climb.startKm × 1000 : climb.startDist
  endM   ← (climb.endKm   != null) ? climb.endKm   × 1000 : climb.endDist
  SI !isFinite(startM) OU !isFinite(endM) OU endM ≤ startM: RETOURNER

  bounds ← _computeBounds(startM, endM)
  map.fitBounds(bounds, {padding: 40, duration, pitch: 60})
```

### 30.3 Fonction `zoomMapToGroup`

```
FONCTION zoomMapToGroup(gid, duration):
  members ← userClimbs.filter(c → c.groupId = gid)
  minKm ← min(members.startKm)
  maxKm ← max(members.endKm)
  bounds ← _computeBounds(minKm × 1000, maxKm × 1000)
  map.fitBounds(bounds, {padding: 40, duration, pitch: 60})
```

### 30.4 `_computeBounds`

```
FONCTION _computeBounds(startM, endM):
  startLL ← latLonAtDist(startM)
  endLL ← latLonAtDist(endM)

  min/max sur lng et lat
  // Garantir un span minimal (~150 m)
  SI span < MAP_MIN_SPAN_DEG: élargir autour du centre

  // Marge 20 % (MAP_CLIMB_MARGIN)
  return [[minLng − padLng, minLat − padLat],
          [maxLng + padLng, maxLat + padLat]]
```

### 30.5 Complexité

- Temps : **O(k)** où k = nombre de membres du groupe
- Espace : **O(1)**

### 30.6 Notes

- **v3.8.2** : `zoomMapToClimb` accepte désormais `{startKm, endKm}` **et** `{startDist, endDist}` — corrige un bug où le clic sur une montée ne zoomait pas (car `data.climbs[i].startKm` était `undefined`)
- **v3.9.2** : dispatch automatique sur `zoomMapToGroup` si `groupId != null`
- **`MAP_MIN_SPAN_DEG = 0.0014`** : évite qu'une montée ultra-courte produise un `fitBounds` dégénéré (zoom à l'infini)

### 30.7 Choix de conception

| Critère | Zoom serré | **Marge 20 % (retenu)** |
|---|---|---|
| Contexte visible | ❌ | ✅ |
| Montée isolée | ✅ | ✅ |
| Zoom groupe vs montée | ❌ | ✅ (dispatch) |

---

## 31. Séparation d'un groupe

### 31.1 Objet

Découper une montée fusionnée en ses sous-montées d'origine, avec persistance du lien via `groupId`.

### 31.2 Motivation

Une montée fusionnée (via `gapMinM`) est **un agrégat**. L'utilisateur peut vouloir :
- Voir les segments d'origine individuellement
- Éditer une sous-montée sans impacter les autres
- Comparer les pentes moyennes par sous-montée

La séparation est donc une opération de **désagrégation**.

### 31.3 Pseudocode

```
FONCTION splitMergedClimb(mergedIdx):
  merged ← userClimbs[mergedIdx]
  SI !merged.mergedParts OU |merged.mergedParts| < 2: RETOURNER

  gid ← STATE.nextGroupId++
  userClimbs.splice(mergedIdx, 1)

  newClimbs ← merged.mergedParts.map((part, k) → {
    startKm: part.startKm,
    endKm: part.endKm,
    autoStartKm: null, autoEndKm: null,
    refineM: null, candidateLen: null,
    mergedCount: 0, mergedParts: null,
    fallback: false,
    groupId: gid,
    subIndex: k + 1
  })

  POUR chaque nc: userClimbs.push(nc)
  userClimbs.sort(par startKm)

  selectedClimbIdx ← index de newClimbs[0]
  profileMode ← 'climb'

  renderAll()
  SI MAP_STATE.ready: zoomMapToGroup(gid)
```

### 31.4 Complexité

- Temps : **O(m log m)** (dominé par le tri)
- Espace : **O(m)** pour les nouvelles montées

### 31.5 Notes

- **`groupId`** : identifiant entier incrémental depuis `STATE.nextGroupId`
- **`subIndex`** : 1, 2, 3… dans le groupe
- **Numérotation affichée** : `#N.M` (ex. `#3.1`, `#3.2`)
- **La sélection** passe au premier membre du groupe
- **Le zoom carte** couvre **l'ensemble du groupe** (pas seulement le premier membre)
- **v3.9.0** : la séparation était locale (pas de groupe persistant)
- **v3.9.1** : ajout de `groupId` + `subIndex` pour permettre la refusion

### 31.6 Choix de conception

| Critère | Séparation destructive | **Séparation avec groupe (retenu v3.9.1)** |
|---|---|---|
| Perte d'information | ✅ Définitive | ❌ Réversible |
| Refusion possible | ❌ | ✅ |
| Complexité de l'état | ✅ Simple | ⚠️ Groupe à gérer |
| **Retenu** | ❌ | ✅ |

---

## 32. Refusion d'un groupe

### 32.1 Objet

Reconstituer une montée fusionnée à partir de tous les membres d'un groupe, en préservant les **bornes actuelles** (après éventuelles éditions).

### 32.2 Motivation

Après avoir séparé une montée puis éventuellement édité les bornes des sous-montées, l'utilisateur peut vouloir revenir à une vue d'ensemble. La refusion doit **conserver les éditions** (contrairement à un simple undo).

### 32.3 Pseudocode

```
FONCTION refuseGroup(gid):
  members ← userClimbs.filter(c → c.groupId = gid).sort(par startKm)
  SI |members| < 2: RETOURNER

  startKm ← members[0].startKm
  endKm ← members[|members|−1].endKm
  mergedParts ← members.map(m → {startKm: m.startKm, endKm: m.endKm})

  // Retirer tous les membres
  userClimbs ← userClimbs.filter(c → c.groupId ≠ gid)

  // Créer la montée fusionnée
  merged ← {
    startKm, endKm,
    autoStartKm: null, autoEndKm: null,
    refineM: null, candidateLen: null,
    mergedCount: members.length − 1,
    mergedParts,
    fallback: false,
    groupId: null, subIndex: null
  }

  userClimbs.push(merged)
  userClimbs.sort(par startKm)

  selectedClimbIdx ← index de merged
  profileMode ← 'climb'
  renderAll()
```

### 32.4 Complexité

- Temps : **O(m log m)** (dominé par le tri)
- Espace : **O(m)**

### 32.5 Notes

- Les `mergedParts` reflètent les bornes **actuelles** (potentiellement éditées par drag)
- Le nouveau `mergedCount` = nombre de fusions = nombre de membres − 1
- La montée redevient **séparable** avec ✂
- **Le groupe `gid` est libéré** (pas réutilisable mais ne crée pas de conflit car `nextGroupId` est monotone)

### 32.6 Scénario complet

```
1. Détection → montée #3 fusionnée (mergedCount=2, mergedParts conservés)
2. Utilisateur clique ✂ Séparer → groupe #5 créé, #5.1 et #5.2
3. Utilisateur édite les bornes de #5.2 (drag)
4. Utilisateur clique 🔗 Refusionner → #3 reconstituée
   mergedParts = [
     { startKm: #5.1.startKm, endKm: #5.1.endKm },
     { startKm: #5.2.startKm_edité, endKm: #5.2.endKm_edité }
   ]
   mergedCount = 1
```

L'édition du point 3 est **conservée** dans la refusion.

### 32.7 Choix de conception

| Critère | Refusion avec bornes d'origine | **Refusion avec bornes éditées (retenu)** |
|---|---|---|
| Respect de l'édition utilisateur | ❌ Perdue | ✅ Conservée |
| Simplicité | ✅ | ⚠️ |
| **Retenu** | ❌ | ✅ |

---

## 33. Normalisation des groupes

### 33.1 Objet

Nettoyer les groupes orphelins (moins de 2 membres) après suppression ou édition.

### 33.2 Motivation

Un groupe à 1 membre n'a pas de sens : la numérotation `#N.1` sans `#N.2` est confuse, et le bouton 🔗 Refusionner n'aurait qu'un seul membre à fusionner (comportement dégénéré).

### 33.3 Pseudocode

```
FONCTION normalizeGroups():
  counts ← compter les occurrences de chaque groupId
  POUR chaque c DANS userClimbs:
    SI c.groupId ≠ null ET counts[c.groupId] < 2:
      c.groupId ← null
      c.subIndex ← null
```

### 33.4 Complexité

- Temps : **O(m)**
- Espace : **O(m)** (tableau `counts`)

### 33.5 Appelée

- À la fin de chaque drag de poignée (dans `onUp`)
- Après `recomputeAuto()` (changement de paramètres)
- Après `deleteClimb()` (suppression)

### 33.6 Notes

- **Ne supprime jamais de montée** — elle retire juste le marqueur de groupe
- La montée redevient **autonome** : numérotation `#N` normale, plus de bouton 🔗 Refusionner
- Silencieux (pas de log debug sauf en cas de groupe effectivement dissous)

---

## 34. Suppression d'une montée

### 34.1 Objet

Retirer une montée de la liste, en gérant proprement les groupes et la sélection.

### 34.2 Motivation

Un utilisateur peut vouloir supprimer une montée qui n'aurait pas dû être détectée (faux positif), ou qu'il a créée par erreur.

### 34.3 Pseudocode

```
FONCTION deleteClimb(idx):
  SI idx hors bornes: RETOURNER
  removed ← userClimbs[idx]

  userClimbs.splice(idx, 1)

  // ─── Renuméroter les subIndex du groupe affecté ───
  SI removed.groupId ≠ null:
    siblings ← userClimbs.filter(c → c.groupId = removed.groupId)
    siblings.sort(par startKm)
    POUR k DE 0 À |siblings|−1:
      siblings[k].subIndex ← k + 1
    SI |siblings| < 2:
      POUR chaque c DANS siblings:
        c.groupId ← null
        c.subIndex ← null

  // ─── Ajuster la sélection ───
  SI selectedClimbIdx === idx:
    selectedClimbIdx ← null
    profileMode ← 'full'
    __climbView ← null
  SINON SI selectedClimbIdx > idx:
    selectedClimbIdx ← selectedClimbIdx − 1

  renderAll()
```

### 34.4 Complexité

- Temps : **O(m log m)** (tri des siblings)
- Espace : **O(1)** (hors tri)

### 34.5 Notes

- **Renumérotation** : si on supprime #3.2 d'un groupe de 3 (#3.1, #3.2, #3.3), les restants deviennent #3.1, #3.2
- **Dissolution** : si on supprime #3.2 d'un groupe de 2 (#3.1, #3.2), #3.1 redevient #N (autonome)
- **Ajustement sélection** :
  - Si c'est la montée supprimée qui était sélectionnée → désélection
  - Si l'index sélectionné était **après** la montée supprimée → décrémente
  - Si l'index sélectionné était **avant** → inchangé

### 34.6 Cas d'usage

| Cas | Avant | Après suppression |
|---|---|---|
| Groupe à 3 membres, supprimer #3.2 | #3.1, #3.2, #3.3 | #3.1, #3.2 (renumérotés) |
| Groupe à 2 membres, supprimer #5.1 | #5.1, #5.2 | #N (dissolution) |
| Montée autonome, supprimer #2 | #1, #2, #3 | #1, #2 (ex-#3, renuméroté) |
| Supprimer la montée sélectionnée | selected = idx | selected = null |

### 34.7 Choix de conception

| Critère | Suppression simple | **Avec renumérotation (retenu)** |
|---|---|---|
| Cohérence de numérotation | ❌ Trous | ✅ Continue |
| Complexité | ✅ | ⚠️ |
| **Retenu** | ❌ | ✅ |

---

## 35. Création manuelle

### 35.1 Objet

Créer une nouvelle montée par double-clic + drag sur le widget profil en mode full.

### 35.2 Motivation

La détection automatique peut manquer certaines bosses (seuil trop haut, anti-pic trop agressif, etc.). L'utilisateur doit pouvoir **ajouter** une montée manuellement.

### 35.3 Contraintes

- ❌ **Chevauchement** avec une montée existante interdit
- ❌ **Longueur minimale** : 50 m (`MANUAL_MIN_LEN_KM = 0.05`)
- ❌ **Vallée dans une montée existante** refusée
- ✅ Double-clic sur le profil pour poser la vallée
- ✅ Clic-glisser pour définir le sommet
- ✅ Relâchement pour valider
- ✅ Échap pour annuler

### 35.4 Machine à états

```
État "idle" (défaut)
  → dblclick sur canvas (mode full, position libre)
  → état "pending" (valleyKm posée, summitKm = valleyKm)

État "pending"
  → mousedown + drag
  → état "dragging" (summitKm mis à jour à chaque mousemove)

État "dragging"
  → mouseup
  → validation :
    - summitKm > valleyKm + 50 m ?
    - isFreeRange(valleyKm, summitKm) ?
    → si OUI : addManualClimb()
    → si NON : annulation

État "pending" ou "dragging"
  → Échap
  → annulation (retour à idle)
```

### 35.5 Validation `isFreeRange`

```
FONCTION isFreeRange(aKm, bKm):
  lo ← min(aKm, bKm)
  hi ← max(aKm, bKm)
  POUR chaque c DANS userClimbs:
    SI lo < c.endKm ET hi > c.startKm: RETOURNER FAUX
  RETOURNER VRAI
```

### 35.6 Ajout

```
FONCTION addManualClimb(valleyKm, summitKm):
  SI summitKm ≤ valleyKm + 50 m: RETOURNER
  SI !isFreeRange(valleyKm, summitKm): RETOURNER

  newClimb ← {startKm: valleyKm, endKm: summitKm,
              autoStartKm: null, autoEndKm: null,
              refineM: null, candidateLen: null,
              mergedCount: 0, mergedParts: null,
              fallback: false, groupId: null, subIndex: null}

  userClimbs.push(newClimb)
  userClimbs.sort(par startKm)
  selectedClimbIdx ← index de newClimb
  profileMode ← 'climb'
  renderAll()
```

### 35.7 Complexité

- Temps : **O(m log m)** (tri après ajout)
- Espace : **O(1)**

### 35.8 Indicateurs visuels

- **Hint** dans le header du profil :
  - Mode attente : `💡 Double-cliquez sur le profil pour créer une montée`
  - Vallée posée : `🎯 Cliquez-glissez pour définir le sommet · Échap pour annuler` (vert pulsant)
  - Drag en cours : `🎯 Relâchez pour valider le sommet` (rouge)
- **Zone colorée** : verte si plage valide, rouge si chevauchement
- **Lignes verticales** : vallée (verte continue), sommet (rouge pointillé, ou rouge vif si invalide)
- **Curseur** : `crosshair` sur le canvas en mode création
- **Flash rouge** au double-clic si la vallée est dans une montée existante

### 35.9 Notes

- **`autoStartKm` et `autoEndKm` à `null`** : la montée n'est pas liée à une détection auto
- **`refineM` à `null`** : la colonne Extrema affiche `—` en gris italique
- **La nouvelle montée est sélectionnée** automatiquement et la carte zoomée dessus
- **`groupId` à `null`** : montée autonome (pas de groupe)

### 35.10 Choix de conception

| Critère | Insertion par bouton `+` | **Double-clic + drag (retenu)** |
|---|---|---|
| Précision | ⚠️ Par défaut 1 km | ✅ Contrôle total |
| Naturel | ⚠️ | ✅ |
| Découverte | ✅ | ⚠️ Hint nécessaire |
| **Retenu** | ❌ | ✅ |

---

## 36. Diagnostic de configuration

### 36.1 Objet

Afficher en temps réel l'état de cohérence des paramètres et les **valeurs effectives en points**.

### 36.2 Pseudocode (extrait)

```
FONCTION computeDiagnostics(params):
  // ─── Valeurs effectives en points ───
  lissageAltPts   ← max(1, round(fenetreAffichageM / stepM))
  lissagePentePts ← max(1, round(fenetrePenteM / stepM))
  couleurPts      ← max(1, round(fenetreCatM / stepM))
  medianPts       ← medianWindowM > 0 ? max(1, round(medianWindowM / stepM)) : 0
  gaussianPts     ← gaussianSigmaM > 0 ? gaussianSigmaM / stepM : 0

  despikeActive ← despikeThresholdM > 0
  despikeWinPtsEffective ← despikeActive ? max(3, round(despikeWindowM / stepM) | 1) : 0
  despikeCovM ← despikeWinPtsEffective × stepM
  despikeExpandRatio ← despikeActive ? despikeCovM / despikeWindowM : 1

  // ─── Ratios et dérivés ───
  ratioAltPente ← fenetreAffichageM / fenetrePenteM
  retardM ← fenetrePenteM / 2

  // ─── Pour chaque paramètre : { label, effective, optimal, target, status, msg } ───
  diag ← { ... }

  // ─── Règles de cohérence ───
  warnings ← []
  SI lissageAltPts < 5: warnings.push({level: 'red', msg: '...'})
  SI lissageAltPts < 10: warnings.push({level: 'orange', msg: '...'})
  SI lissagePentePts < 3: warnings.push({level: 'red', msg: '...'})
  SI ratioAltPente < 1.5: warnings.push({level: 'red', msg: '...'})
  SI ratioAltPente < 2: warnings.push({level: 'orange', msg: '...'})
  SI medianWindowM > 0 ET medianPts < 3: warnings.push({level: 'orange', msg: '...'})
  SI longueurMin > 0 ET longueurMin < stepM: warnings.push({level: 'orange', msg: '...'})
  SI gapMinM > 0 ET gapMinM < stepM: warnings.push({level: 'orange', msg: '...'})
  SI despikeActive ET despikeWindowM < 2 × stepM: warnings.push({level: 'orange', msg: '...'})
  SI despikeActive ET despikeThresholdM < 5: warnings.push({level: 'orange', msg: '...'})
  SI exportProfileStepM > longueurMin: warnings.push({level: 'orange', msg: '...'})

  RETOURNER { diag, warnings }
```

### 36.3 Valeurs optimales

| Paramètre | Optimal (pts) | Optimal (m, Pas=50) |
|---|---|---|
| Lissage altitude | 10–40 | 500–2000 |
| Lissage pente | 3–10 | 150–500 |
| Fenêtre de calcul pente | 3–15 | 150–750 |
| Filtre médian | désactivé ou ≥ 3 | 0 ou ≥ 150 |
| Lissage gaussien | 0.5–5 | 25–250 |
| Anti-pic : fenêtre | — | 150–300 m |
| Anti-pic : seuil | — | 15–25 m |

### 36.4 Statuts

| Statut | Signification | Couleur |
|---|---|---|
| 🟢 green | Optimal | `#16a34a` |
| 🟠 orange | Limite / avertissement | `#f59e0b` |
| 🔴 red | Incohérence critique | `#dc2626` |
| ⚪ grey | Désactivé / non applicable | `#94a3b8` |

### 36.5 Complexité

- Temps : **O(1)** (calculs sur 15 paramètres)
- Espace : **O(1)**

### 36.6 Notes

- Le diagnostic est recalculé à chaque `input` sur un champ (debounce 250 ms)
- Les warnings sont affichés dans un panneau dédié
- Le statut global (🟢/🟠/🔴) résume l'état : rouge si au moins un rouge, orange si au moins un orange, vert sinon

### 36.7 Choix de conception

| Critère | **Diagnostic (retenu v3.2)** | Paramètres seuls |
|---|---|---|
| Compréhension utilisateur | ✅ Élevée | ❌ Faible |
| Complexité UI | ⚠️ Ajout panneau | ✅ Simple |
| Coût calcul | ✅ O(1) | — |
| **Retenu** | ✅ | ❌ |

---

## 37. Export JSON

### 37.1 Objet

Produire un fichier JSON contenant les segments de catégorie et les montées, pour consommation externe (affichage cartographique Mapbox, visuels de montée, etc.).

### 37.2 Structure

```json
{
  "metadata": {
    "file": "trace.gpx",
    "exportedAt": "ISO8601",
    "version": "3.9.2",
    "totalDistanceKm": 4.28,
    "totalDistanceSampledKm": 4.25,
    "trailingGapM": 30,
    "stepM": 50,
    "totalPoints": 86,
    "segmentsCount": 4,
    "climbsCount": 1,
    "profileStepM": 50
  },
  "legend": {
    "note": "...",
    "fenetreCatM": 100,
    "longueurMin": 200,
    "categories": [...]
  },
  "segments": [...],
  "climbs": [...]
}
```

### 37.3 Nouveautés v3.9.1

Ajout des champs `groupId` et `subIndex` dans chaque `climb`.

### 37.4 Découpage `slopeProfile`

Pour chaque montée :

1. Identifier les segments globaux intersectant la montée
2. Pour chaque segment intersectant :
   - `segStart = max(seg.startDist, climbStartDist)`
   - `segEnd = min(seg.endDist, climbEndDist)`
   - `nChunks = max(1, ceil(segLen / profileStepM))`
   - `chunkLen = segLen / nChunks`
3. Pour chaque tranche :
   - `kmStart`/`kmEnd` = bornes réelles
   - `eleStartM`/`eleEndM` = altitudes lissées ancrées
   - `avgSlopePct` = pente physique D+/longueur
   - `categoryId` = **hérité du segment parent**

**Propriété garantie** : aucune tranche ne traverse une frontière de segment.

### 37.5 Contrat d'utilisation des progressions

`startProgress` et `endProgress` sont des fractions 0–1 de la **longueur réelle** de la trace (`metadata.totalDistanceKm`), directement utilisables pour construire un `line-gradient` Mapbox :

```javascript
map.addLayer({
  id: 'trace-colored',
  type: 'line',
  source: 'trace',
  paint: {
    'line-width': 5,
    'line-gradient': ['interpolate', ['linear'], ['line-progress'], ...stops]
  }
});
```

Pour éviter les dégradés progressifs entre deux stops, répéter la même couleur au début et à la fin de chaque segment.

### 37.6 Complexité

- Temps : **O(m)** pour les segments + **O(m × k)** pour les profils (k = tranches par montée)
- Espace : **O(m × k)**

### 37.7 Choix de conception

| Critère | Format plat | FeatureCollection |
|---|---|---|
| Taille | ✅ Compact | ❌ Verbeux |
| Intégration Mapbox directe | ✅ | ⚠️ Nécessite transformation |
| Lisibilité | ✅ | ⚠️ Imbriqué |
| **Retenu (v3.7.2)** | ✅ | ❌ |

---

## 38. Pipeline complet

### 38.1 Pseudocode

```
FONCTION analyze(content, params):
  track ← parseGPX(content)
  n ← longueur(track.points)

  distRaw ← cumsum(haversine)                        // §1
  eleRawOrig ← track.points.map(p → p.ele)

  rawStats ← computeRawStats(distRaw, eleRawOrig)   // §5

  // ─── Correction plateaux (optionnelle) ───
  SI params.plateauRun > 0:
    eleForResample ← plateauCorrect(eleRawOrig, ...)  // §3
  SINON:
    eleForResample ← eleRawOrig

  // ─── Rééchantillonnage par moyenne ───
  { dist, ele, avgWinM } ← resampleUniform(distRaw,   // §2
                                            eleForResample,
                                            params.stepM)

  // ─── Anti-pic Hampel ───
  eleFinal ← ele
  SI params.despikeThresholdM > 0:
    winPts ← max(3, round(params.despikeWindowM / stepM) | 1)
    { out, count } ← despikeAltitude(ele, winPts,     // §4
                                     params.despikeThresholdM)
    eleFinal ← out

  // ─── Détection ───
  { autoClimbs } ← computeAuto(dist, eleFinal, params)  // §14-26

  RETOURNER { track, distRaw, eleRawOrig, dist, eleFinal,
              stepM, avgWinM, rawStats, autoClimbs, ... }
```

### 38.2 Ordre des étapes dans `computeAuto`

1. Filtre médian (optionnel) — §9
2. Savitzky-Golay (altitude de base) — §7
3. Pente lissée + clip — §6, §11, §13
4. Hystérésis — §14
5. Candidats — §15
6. Sous-segment optimal — §16
7. **Raffinement + Fusion** — §24, §25 (avec `mergedParts`)
8. Lissage ancré final — §10
9. **Validation stricte + repli** — §26 (avec préservation de `mergedParts`)

### 38.3 Complexité globale

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

## 39. Pipeline de reconstruction

### 39.1 Objet

Recalculer entièrement l'état affiché à partir de `STATE.userClimbs` (état éditable par l'utilisateur). Appelé après chaque édition, insertion, suppression, ou changement de paramètre.

### 39.2 Pseudocode

```
FONCTION rebuild():
  SI STATE.dist = null OU STATE.eleRaw = null: RETOURNER null

  params ← getParams()

  // === ÉTAPE 1 : Contraintes anti-chevauchement ===
  sorted ← appliquerContraintes(STATE.userClimbs)   // §23

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

  // === ÉTAPE 3 : Courbe affichée (ancrée) ===
  eleDisplay ← anchoredMovingAverage(eleRaw, anchors, stepM,
                                     fenetreAffichageM, gaussianSigmaM,
                                     smoothExtra)          // §10

  // === ÉTAPE 4 : Pentes pour l'affichage ===
  slopeRaw   ← clipSlope(computeSlope(MA(eleRaw, 5), dist), 25)
  slopeSemi  ← computeSlope(eleDisplay, dist)
  slopeDisplay ← clipSlope(MA(slopeSemi, winSlopePts), 25)

  // === ÉTAPE 5 : Catégorisation NON ancrée (v3.8.1) ===
  eleBaseCat ← savitzkyGolay(eleRaw, winSGCatPts, 2)   // §27
  slopeForCat ← computeSlopeWindow(eleBaseCat, dist, fenetreCatM)

  // === ÉTAPE 6 : Métriques par montée ===
  climbs ← []
  POUR chaque { startIdx, endIdx, ref, invalid } DANS resolved:
    SI invalid: climbs.push({ invalid, length: 0, ... })
    SINON:
      dPlus ← Σ max(0, ΔeleDisplay)
      length ← dist[endIdx] − dist[startIdx]
      avgSlope ← dPlus / length × 100
      climbs.push({ ..., length, dPlus, avgSlope,
                    mergedCount: ref.mergedCount || 0,
                    fallback: ref.fallback || false,
                    groupId: ref.groupId, subIndex: ref.subIndex })

  // === ÉTAPE 7 : Segmentation ===
  catSegments ← segmentByCategory(slopeForCat, dist, eleDisplay, longueurMin)  // §21

  RETOURNER { dist, eleRaw, eleDisplay, slopeRaw, slopeDisplay,
              slopeForCat, catSegments, climbs }
```

### 39.3 Complexité

| Étape | Complexité |
|---|---|
| Contraintes | O(m log m) |
| Ancrage | O(m) + O(m log n) |
| Lissage ancré | O(n + m × winDouble) |
| Pente fenêtrée | O(n × w) |
| SG non ancré | O(n × winSGCat) |
| Métriques | O(n) |
| Segmentation | O(n + s²) |
| **Total** | **O(n × w)** dominé par la pente fenêtrée |

Avec n=5 000 et w=100, ~500 000 opérations → **< 100 ms**.

---

## 40. Tableau récapitulatif des complexités

| Algorithme | Temps | Espace | Fréquence d'appel |
|---|---|---|---|
| Haversine (par paire) | O(1) | O(1) | n−1 fois |
| Distance cumulée | O(n) | O(n) | 1 fois |
| **resampleUniform (moyenne)** | **O(n + m)** | **O(m)** | **1 fois** |
| plateauCorrect | O(n) | O(n) | 1 fois (optionnel) |
| **despikeAltitude** | **O(n × w × log w)** | **O(w)** | **1 fois** |
| computeRawStats | O(n log n) | O(n) | 1 fois |
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
| **Fusion montées** | **O(m)** | **O(m)** (mergedParts) | **m fois** |
| **Validation stricte** | **O(m × 800²)** | **O(m)** | **1 fois** |
| **buildTraceFeatures** | **O(n)** | **O(n)** | **à chaque rendu** |
| **updateMapMarkers** | **O(m_groupe)** | **O(m_groupe)** | **à chaque sélection** |
| **zoomMapToGroup** | **O(m_groupe)** | **O(1)** | **à la sélection** |
| **splitMergedClimb** | **O(m log m)** | **O(m)** | **au clic ✂** |
| **refuseGroup** | **O(m log m)** | **O(m)** | **au clic 🔗** |
| **deleteClimb** | **O(m log m)** | **O(1)** | **au clic ✕** |
| **addManualClimb** | **O(m log m)** | **O(1)** | **à la création** |
| **normalizeGroups** | **O(m)** | **O(m)** | **fin de drag** |
| Diagnostic | O(1) | O(1) | à chaque input |
| rebuild | O(n × w) | O(n) | après chaque édition |

Légende :
- n : nombre de points bruts
- m : nombre de points après resampling
- s : nombre de segments de catégorie
- w : taille moyenne d'une fenêtre glissante
- k : nombre d'événements
- m_groupe : nombre de membres d'un groupe

---

## 41. Choix de conception et alternatives écartées

### 41.1 Haversine vs Vincenty

| Critère | Haversine | Vincenty |
|---|---|---|
| Précision | ±0.5 % | ±0.001 % |
| Vitesse | Très rapide | 10× plus lent |
| Complexité | 5 lignes | 30+ lignes |
| **Retenu** | ✅ | ❌ |

### 41.2 Interpolation vs moyenne au resampling

| Critère | Interpolation | **Moyenne (v3.5)** |
|---|---|---|
| Fidélité au détail brut | Élevée | Moyenne |
| Atténuation bruit HF | Nulle | **Élevée** |
| Efficacité Hampel en aval | Faible à petit Pas | **Élevée** |
| Cohérence du pipeline selon Pas | Faible | **Élevée** |
| Complexité | O(n + m) | O(n + m) |
| **Retenu** | ❌ | ✅ |

### 41.3 Différence centrée vs différence avant

| Critère | **Centrée (retenu)** | Avant |
|---|---|---|
| Décalage de phase | Aucun | +1 point |
| Lissage naturel | Oui (3 pts) | Non |
| **Retenu** | ✅ | ❌ |

### 41.4 Pente instantanée vs pente fenêtrée pour la catégorisation

| Critère | Instantanée | **Fenêtrée (retenu)** |
|---|---|---|
| Stabilité visuelle | Faible (taches) | Élevée |
| Fidélité locale | Haute | Moyenne |
| Coût calcul | O(n) | O(n × w) |
| **Retenu couleur** | ❌ | ✅ |
| **Retenu courbe** | ✅ | ❌ |

### 41.5 Argmin/argmax vs marche avec tolérance

| Critère | **Argmin/argmax (retenu)** | Marche tolérance |
|---|---|---|
| Robustesse au bruit | Élevée | Faible |
| Déterminisme | Toujours | Dépend de la tolérance |
| **Retenu** | ✅ | ❌ |

### 41.6 Garde-fou absolu vs pourcentage vs hybride

| Critère | Absolu | % pur | **Hybride (v2)** |
|---|---|---|---|
| Simplicité | ✅✅ | ✅ | ⚠️ |
| Adaptabilité à l'échelle | ❌ | ✅ | ✅✅ |
| Prévisibilité | ✅✅ | ⚠️ | ✅ |
| Robustesse cas extrêmes | ✅ | ❌ | ✅✅ |
| **Retenu** | ❌ | ❌ | ✅ |

### 41.7 Fusion par distance vs augmentation du seuil

| Critère | **gapMinM (retenu)** | Seuil élevé |
|---|---|---|
| Ratage des vraies montées douces | ✅ Évité | ❌ Possible |
| Prévisibilité | ✅ | ⚠️ Dépend de la trace |
| **Retenu** | ✅ | ❌ |

### 41.8 Validation finale : rejet vs repli

| Critère | Rejet sec | **Repli ⤺ (retenu)** |
|---|---|---|
| Perte d'information | ❌ Complète | ✅ Partielle |
| Cohérence avec l'utilisateur | ⚠️ Difficile | ✅ Log détaillé |
| **Retenu** | ❌ | ✅ |

### 41.9 Anti-pic Hampel vs médian classique

| Critère | Médian | **Hampel (retenu)** |
|---|---|---|
| Modifie le relief normal | Oui | **Non** |
| Élimine un pic isolé | Oui | Oui |
| Préserve les extrema | Non | **Oui** |
| Effet sur les vraies montées | Adoucit | **Aucun** |
| Effet d'écrêtage des bosses | Marqué | **Sélectif** |
| **Retenu** | ❌ | ✅ |

### 41.10 Anti-pic : seuil en mètres vs en %

| Critère | **Seuil en mètres (retenu)** | Seuil en % |
|---|---|---|
| Prévisibilité | ✅ | ⚠️ |
| Indépendance du relief | ✅ | ❌ |
| **Retenu** | ✅ | ❌ |

### 41.11 Anti-pic : position dans le pipeline

| Ordre | Efficacité |
|---|---|
| Avant resampling | ❌ Fenêtre en points varie avec densité |
| **Après resampling (retenu)** | ✅ Fenêtre constante après conversion |
| Après SG | ❌ SG a déjà ancré le glitch dans la courbe |

### 41.12 Anti-pic : fenêtre en points vs en mètres

| Critère | Points | **Mètres (v3.6)** |
|---|---|---|
| Cohérence avec autres paramètres | ❌ | ✅ |
| Comportement indépendant du Pas | ❌ | ✅ |
| Prévisibilité utilisateur | ⚠️ | ✅ |
| Plage utile | 3–15 pts | **50–500 m** |
| **Retenu** | ❌ | ✅ |

### 41.13 Diagnostic visuel vs paramètres seuls

| Critère | **Diagnostic (v3.2)** | Paramètres seuls |
|---|---|---|
| Compréhension utilisateur | ✅ Élevée | ❌ Faible |
| Complexité UI | ⚠️ Ajout panneau | ✅ Simple |
| Coût calcul | ✅ O(1) | — |
| **Retenu** | ✅ | ❌ |

### 41.14 Édition numérique vs drag & drop

| Critère | Champs km | **Drag & drop** |
|---|---|---|
| Précision | ✅✅ | ⚠️ |
| Naturalité | ❌ | ✅✅ |
| Accessibilité clavier | ✅ | ⚠️ (ajoutée) |
| Complexité implémentation | ✅ | ⚠️ |
| **Retenu** | ✅ (conservé) | ✅ (mode principal) |

Les deux modes coexistent.

### 41.15 Interception DOM vs hooks Chart.js

| Critère | beforeEvent | **Capture window** |
|---|---|---|
| Fonctionne avec plugin-zoom | ❌ | ✅ |
| Ordre garanti | ❌ | ✅ |
| Complexité | ✅ | ⚠️ |
| **Retenu** | ❌ | ✅ |

### 41.16 Chart.js vs canvas 2D natif

| Critère | Chart.js | **Canvas 2D (v3.8)** |
|---|---|---|
| Contrôle du rendu | ⚠️ | ✅ Total |
| Perf sur grandes traces | ⚠️ | ✅ |
| Écosystème | ✅ | ⚠️ |
| Échelles strictement fixes | ⚠️ | ✅ |
| **Retenu** | ❌ | ✅ |

### 41.17 Catégorisation sur altitude ancrée vs non ancrée

| Critère | Ancré | **Non ancré (v3.8.1)** |
|---|---|---|
| Cohérence carte/profil | ❌ | ✅ |
| Stabilité au drag | ❌ | ✅ |
| Coût | ✅ | ⚠️ (+1 SG) |
| **Retenu** | ❌ | ✅ |

### 41.18 Filtrage des couleurs profil : point par point vs par `catSegments`

| Critère | Point par point | **Par `catSegments`** |
|---|---|---|
| Identité avec la carte | ❌ | ✅ |
| Perf | ✅ | ✅ |
| **Retenu** | ❌ | ✅ |

### 41.19 Échelle X du profil : fixe vs adaptative au conteneur

| Critère | **Fixe (v3.8.3)** | Adaptative |
|---|---|---|
| Respect strict de l'échelle | ✅ | ❌ |
| Esthétique (canvas plein) | ❌ Vide à droite | ✅ |
| **Retenu** | ✅ | ❌ |

### 41.20 Trace carte : grille resamplée vs points bruts

| Critère | **Points bruts (v3.9.2)** | Grille resamplée |
|---|---|---|
| Fidélité au GPX | ✅ | ❌ |
| Indépendance au Pas | ✅ | ❌ |
| Détail des lacets | ✅ | ⚠️ Gomme les petits |
| Coût | O(n) | O(m) |
| **Retenu** | ✅ | ❌ |

### 41.21 Groupes : fusion destructive vs préservation des `mergedParts`

| Critère | Fusion destructive | **Préservation (v3.9.0+)** |
|---|---|---|
| Séparation ultérieure | ❌ | ✅ |
| Refusion | ❌ | ✅ |
| Mémoire | ✅ Faible | ⚠️ +tab par fusion |
| **Retenu** | ❌ | ✅ |

### 41.22 Groupes : séparation sans persistance vs avec `groupId`/`subIndex`

| Critère | Sans persistance (v3.9.0) | **Avec (v3.9.1)** |
|---|---|---|
| Refusion possible | ❌ | ✅ |
| Numérotation `#N.M` | ❌ | ✅ |
| Coloration stable | ❌ | ✅ |
| **Retenu** | ❌ | ✅ |

### 41.23 Suppression : renumérotation vs laisser des trous

| Critère | **Renumérotation (v3.9.2)** | Trous |
|---|---|---|
| Cohérence numérotation | ✅ | ❌ |
| Complexité | ⚠️ | ✅ |
| **Retenu** | ✅ | ❌ |

### 41.24 Création manuelle : bouton `+` vs double-clic + drag

| Critère | Bouton `+` | **Double-clic + drag** |
|---|---|---|
| Précision | ⚠️ Par défaut 1 km | ✅ Contrôle total |
| Naturel | ⚠️ | ✅ |
| Découverte | ✅ | ⚠️ Hint nécessaire |
| **Retenu** | ❌ | ✅ |

### 41.25 Auto-load carte : sur présence de token en localStorage

| Critère | **Auto (v3.9.2)** | Manuel |
|---|---|---|
| UX immédiate | ✅ | ❌ |
| Contrôle | ⚠️ | ✅ |
| **Retenu** | ✅ | ❌ |

### 41.26 Marqueurs : mono vs multi sur groupe

| Critère | Mono | **Multi (v3.9.2)** |
|---|---|---|
| Visualisation frontières | ❌ | ✅ |
| Clarté pour groupe | ❌ | ✅ |
| Charge visuelle | ✅ Faible | ⚠️ Plus élevée |
| **Retenu** | ❌ | ✅ (pour groupes) |

### 41.27 Zoom : montée vs groupe

| Critère | **Dispatch (v3.9.2)** | Montée uniquement |
|---|---|---|
| Cohérence avec la sélection | ✅ | ❌ |
| Contexte du groupe | ✅ | ❌ |
| **Retenu** | ✅ | ❌ |

### 41.28 Profil montée : vue zoomée vs vue complète + scroll centré

| Critère | Vue zoomée | **Complète + scroll (v3.9.2)** |
|---|---|---|
| Contexte visible | ❌ | ✅ |
| Centrage automatique | ⚠️ | ✅ |
| Navigation manuelle | ❌ | ✅ |
| **Retenu** | ❌ | ✅ |

### 41.29 Échelle widget montée : 5 vs 10 m/px

| Critère | 5 m/px (v3.8) | **10 m/px (v3.9.2)** |
|---|---|---|
| Détail local | ✅ Élevé | ⚠️ Moyen |
| Vue d'ensemble | ❌ Étroite | ✅ Plus large |
| **Retenu** | ❌ | ✅ |

Le 10 m/px permet de voir la montée et son contexte immédiat (montées voisines) dans une largeur d'écran normale.

---

**Fin du document Algorithmes v3.9.2**