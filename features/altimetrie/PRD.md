# PRD — Analyseur de profil altimétrique GPX

**Nom de code** : GPX Climb Analyzer
**Version** : 3.6
**Date** : 2026-09-24
**Statut** : Spécification produit — application web monopage (SPA)
**Auteur** : [à compléter]
**Destinataire** : développeur front-end, product owner

---

## 1. Contexte et objectif

### 1.1 Contexte

Les cyclistes, randonneurs et traileurs utilisent des traces GPX pour préparer ou analyser leurs sorties. Les traces proviennent soit de relevés GPS terrain (montres, téléphones), soit — et c'est le cas principal supporté — d'**éditeurs de trace** (Openrunner, Garmin Connect, Komoot) qui génèrent les points à partir d'un algorithme de simplification et d'un MNT (Modèle Numérique de Terrain, typiquement SRTM).

Trois problèmes majeurs apparaissent :

1. **Irrégularité d'espacement** : les points sont densément regroupés dans les virages et clairsemés dans les lignes droites (facteur 40–150× observé sur trace Openrunner)
2. **Glitchs d'altitude isolés** : artefacts du capteur GPS ou du MNT produisant un pic de quelques dizaines de mètres sur 1–2 points, amplifié par la dérivée (pente) jusqu'à ±25 % et produisant une fausse montée
3. **Plateaux MNT** : séries d'altitudes identiques correspondant aux cellules SRTM de 30 m, créant des marches d'escalier

### 1.2 Objectif

Fournir un outil web autonome permettant :

- de charger une trace GPX (drop-zone persistante),
- d'afficher un profil altimétrique détaillé,
- de **rééchantillonner par moyenne** à pas fixe avant tout filtrage,
- d'**éliminer les glitchs d'altitude isolés** (filtre anti-pic avec fenêtre en mètres),
- de **corriger les plateaux MNT** (interpolation linéaire),
- de détecter automatiquement les montées significatives selon des critères configurables,
- d'**ajuster manuellement** les bornes par drag & drop direct,
- d'**insérer de nouvelles montées** à n'importe quel endroit,
- de **fusionner** les montées trop proches,
- de **valider strictement** les bornes finales après raffinement et fusion,
- de segmenter par catégorie de pente avec code couleur,
- d'afficher un **diagnostic de configuration** en temps réel.

### 1.3 Proposition de valeur

| Axe | Bénéfice utilisateur |
|---|---|
| Robustesse signal | Rééchantillonnage par moyenne + anti-pic en mètres → comportement identique quelle que soit la trace |
| Transparence | Tous les paramètres en mètres, diagnostic visuel en direct |
| Édition naturelle | Drag & drop des marqueurs sur le graphique |
| Anti-fragmentation | Fusion automatique des montées coupées par un faux-plat |
| Anti-régression | Validation stricte post-raffinement avec repli sur sous-segment |
| Anti-glitch | Filtre anti-pic ciblé, ne modifie que les points aberrants |
| Confidentialité | 100 % côté navigateur, aucun envoi sur un serveur |
| Richesse | Analyse multi-niveaux : segmentation + montées significatives |
| Cohérence | Anti-chevauchement, anti-zones-blanches, catégorisation par fenêtre |
| Prévisibilité | Fenêtre anti-pic indépendante du Pas (v3.6) |

---

## 2. Utilisateurs cibles

| Profil | Usage principal | Attentes |
|---|---|---|
| Cycliste route / gravel | Analyser une étape avant de la rouler | Identifier les cols, D+, pente moyenne, ajuster les bornes |
| Randonneur / traileur | Évaluer la difficulté d'un parcours | Voir les portions raides, D+ cumulé |
| Organisateur d'événement | Vérifier un tracé | Ajouter des montées non détectées automatiquement |
| Data analyst / développeur | Explorer et valider les données GPX | Paramétrer finement, comprendre les effets |

---

## 3. Périmètre fonctionnel

### 3.1 Chargement du fichier

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-01 | Chargement par glisser-déposer d'un fichier `.gpx` | Must |
| F-02 | Chargement par clic sur la zone de dépôt | Must |
| F-03 | Support des fichiers `.txt` contenant du XML GPX | Must |
| F-04 | Gestion d'erreur : fichier non-XML ou sans altitude | Must |
| F-05 | Support multi-segments `<trkseg>` (concaténation des points) | Should |
| F-06 | Support des métadonnées `<name>` pour titre de trace | Nice |
| F-07 | Drop-zone persistante après chargement (mode compact) | Must |
| F-08 | Affichage du nombre de points bruts et resamplés sur la drop-zone | Must |

### 3.2 Prétraitement du signal d'altitude

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-10 | Calcul de la distance cumulée (formule de Haversine) | Must |
| F-11 | Correction des plateaux GPS/MNT (`plateauCorrect`) | Must |
| **F-12** | **Rééchantillonnage par moyenne (fenêtre ≥ 75 m)** | **Must** |
| **F-13** | **Filtre anti-pic Hampel — fenêtre en mètres (50–500, défaut 200)** | **Must** |
| F-14 | Filtre médian paramétrable en mètres | Must |
| F-15 | Savitzky-Golay ordre 2 paramétrable en mètres | Must |
| F-16 | Lissage gaussien paramétrable en σ (mètres) | Should |
| F-17 | Lissage ancré forçant le passage par les extrema détectés | Must |
| F-18 | Double SG optionnel (lissage renforcé) | Nice |
| F-19 | Pente de catégorisation par fenêtre glissante | Must |
| F-20 | Écrêtage des pentes affichées à ±25 % | Must |
| F-21 | Stats brutes (D+, D−, altitudes min/max) sur données GPX d'origine, doublons ignorés | Must |

### 3.3 Détection automatique des montées

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-30 | Détection par hystérésis sur la pente lissée | Must |
| F-31 | Sous-segment optimal pour validation intermédiaire | Must |
| F-32 | Extension des bornes aux extrema locaux par argmin/argmax | Must |
| F-33 | Borne basse naturelle : ne pas empiéter sur la montée précédente | Must |
| F-34 | Garde-fou hybride de refinage `clamp(pct × longueur, 1500 m, 8000 m)` | Must |
| F-35 | Fusion des montées séparées de moins de `gapMinM` | Must |
| F-36 | Validation finale stricte : re-vérification des critères sur bornes finales | Must |
| F-37 | Repli automatique sur le meilleur sous-segment valide si bornes finales échouent | Must |
| F-38 | Transparence : affichage de la valeur effective du garde-fou par montée | Must |
| F-39 | Indicateur visuel `⊕` sur les montées issues d'une fusion | Must |
| F-40 | Indicateur visuel `⤺` sur les montées issues d'un repli | Must |

### 3.4 Critères de validation d'une montée

Une montée n'est retenue que si elle satisfait l'un des couples (longueur, pente moyenne) :

| Longueur | Pente moyenne minimum |
|---|---|
| ≥ 1000 m | ≥ 3 % |
| 750–1000 m | ≥ 4 % |
| 500–750 m | ≥ 5 % |
| 400–500 m | ≥ 6 % |
| 300–400 m | ≥ 7 % |
| < 300 m | Rejetée |

Ces critères sont affichés à l'utilisateur dans l'UI mais **non modifiables** dans la version 3.x.

### 3.5 Édition manuelle des montées

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-50 | Tableau des montées avec champs éditables (km vallée, km sommet) | Must |
| F-51 | Recalcul en direct des statistiques | Must |
| F-52 | Bouton « Restaurer la valeur auto » par ligne (`↺`) | Must |
| F-53 | Bouton « Supprimer » par ligne (`✕`) | Must |
| F-54 | Insertion contextuelle entre deux montées / avant #1 / après #N | Must |
| F-55 | Bouton « Recharger depuis la détection auto » | Must |
| F-56 | Indicateur visuel « modifié » sur les lignes éditées | Should |
| F-57 | Synchronisation intelligente : les paramètres modifiés n'écrasent pas les éditions | Must |
| F-58 | Anti-chevauchement : contrainte automatique sur les bornes | Must |
| F-59 | Signal visuel `⚠` + fond orange sur les lignes contraintes | Should |
| F-60 | Édition par drag & drop direct sur le graphique | Must |
| F-61 | Sélection par radio dans le tableau (mono-sélection) | Must |
| F-62 | Désélection (reclic radio + touche Échap) | Must |
| F-63 | Contraintes anti-chevauchement temps réel pendant le drag | Must |
| F-64 | Retour visuel temps réel (métriques + canvas) pendant le drag | Must |
| F-65 | Édition clavier (flèches + Shift + Alt) | Should |

### 3.6 Segmentation par catégorie de pente

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-70 | Classification en 8 catégories | Must |
| F-71 | Fusion des segments < `longueurMin` dans un voisin | Must |
| F-72 | Segments contigus : partage du point frontière, aucune zone blanche | Must |
| F-73 | Bandes colorées en fond des graphiques | Must |
| F-74 | Bande de segmentation cliquable (zoom sur segment) | Should |
| F-75 | Tableau statistiques par catégorie | Should |
| F-76 | Catégorisation par pente moyenne sur fenêtre glissante | Must |

#### 3.6.1 Grille des catégories

| Couleur | Libellé | Plage de pente |
|---|---|---|
| `#1e40af` Bleu foncé | Descente raide | p < −8 % |
| `#3b82f6` Bleu | Descente | −8 % ≤ p < −3 % |
| `#93c5fd` Bleu clair | Faux plat descendant | −3 % ≤ p < −1 % |
| `#22c55e` Vert | Plat | −1 % ≤ p < +1 % |
| `#fde68a` Jaune | Faux plat montant | +1 % ≤ p < +3 % |
| `#f97316` Orange | Montée | +3 % ≤ p < +7 % |
| `#dc2626` Rouge | Montée raide | +7 % ≤ p < +10 % |
| `#000000` Noir | Montée très raide | p ≥ +10 % |

*Les bornes hautes sont exclusives dans le code.*

### 3.7 Visualisation

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-80 | Profil altimétrique : altitude brute + altitude lissée ancrée | Must |
| F-81 | Graphique de pente : pente brute + pente lissée (écrêtée ±25 %) | Must |
| F-82 | Zoom molette synchronisé entre les deux graphiques | Must |
| F-83 | Déplacement clic-glisser synchronisé | Must |
| F-84 | Double-clic pour réinitialiser le zoom | Should |
| F-85 | Lignes verticales : vert = vallée, rouge pointillé = sommet | Must |
| F-86 | Tooltip avec km et valeur au survol | Must |
| F-87 | Sous-échantillonnage automatique (> 4000 points) | Must |
| F-88 | Légende explicite sous la bande de segmentation | Must |
| F-89 | Toggle « Mettre en avant l'altitude brute » avec états distincts | Must |
| F-90 | Tooltip masqué pendant le drag des poignées | Must |
| F-91 | Graphe Pente repliable (replié par défaut) | Should |
| F-92 | Bande de surbrillance jaune sur la montée sélectionnée | Must |
| F-93 | Poignées triangulaires draggables (vert/rouge) sur le graphique | Must |
| F-94 | Panneau diagnostic de configuration | Must |
| F-95 | Feu tricolore sur chaque paramètre | Must |
| F-96 | Valeur effective en points sous chaque champ | Must |
| F-97 | Affichage du nombre de points corrigés par l'anti-pic dans le Debug | Must |
| **F-98** | **Affichage de la fenêtre anti-pic effective en points sous le champ** | **Must** |

### 3.8 Statistiques globales

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-100 | Distance totale | Must |
| F-101 | D+ brut (calculé sur altitudes GPX d'origine, doublons ignorés) | Must |
| F-102 | D− brut | Must |
| F-103 | Altitude min / max (données d'origine) | Must |
| F-104 | Nombre de montées détectées (avec compteurs `⊕` et `⤺`) | Must |
| F-105 | D+ cumulé des montées validées | Must |
| F-106 | Distance cumulée des montées validées | Must |
| F-107 | Nombre de segments de catégorie | Should |
| F-108 | Nombre de points bruts → resamplés | Must |

---

## 4. Paramètres utilisateur

Tous les paramètres sont modifiables en temps réel. Chaque modification déclenche un recalcul.

### 4.1 Paramètres principaux (visibles)

| Nom UI | Nom technique | Plage | Défaut | Unité | Impact |
|---|---|---|---|---|---|
| Pas de rééchantillonnage | `stepM` | 2–200 | **50** | m | Espacement cible après resampling |
| **Anti-pic : fenêtre** | **`despikeWindowM`** | **50–500** | **200** | **m** | **Fenêtre Hampel** |
| Anti-pic : seuil | `despikeThresholdM` | 0–100 | **15** | m | Seuil détection glitch |
| Lissage altitude | `fenetreAffichageM` | 20–∞ | **500** | m | Douceur du profil affiché |
| Lissage pente | `fenetrePenteM` | 10–∞ | **150** | m | Douceur de la courbe de pente |
| Fenêtre de calcul pente | `fenetreCatM` | 50–∞ | **100** | m | Largeur fenêtre glissante couleur |
| Seuil détection | `seuilMontee` | 0.1–∞ | **0.5** | % | Sensibilité de la détection |
| Longueur min. pente | `longueurMin` | 0–∞ | **200** | m | Fusion micro-segments |

### 4.2 Paramètres avancés

| Nom UI | Nom technique | Plage | Défaut | Unité | Impact |
|---|---|---|---|---|---|
| Écart min. entre montées | `gapMinM` | 0–5000 | **250** | m | Fusionne les montées plus proches |
| Recherche extrema | `refinePct` | 1–500 | **60** | % | Distance max de recherche extrema |
| Filtre médian | `medianWindowM` | 0–300 | **0** | m | Élimine les pics isolés |
| Lissage gaussien | `gaussianSigmaM` | 0–500 | **75** | m | Adoucissement post-ancrage |
| Correction plateaux | `plateauRun` | 0–20 | **6** | pts | Détection des marches MNT/GPS |

### 4.3 Paramètre debug

| Nom UI | Nom technique | Plage | Défaut | Unité |
|---|---|---|---|---|
| Segments debug | `segmentLogM` | 10–2000 | **100** | m |

### 4.4 Toggles

| Nom UI | Nom technique | Défaut | Impact |
|---|---|---|---|
| Lissage renforcé (double SG) | `smoothExtra` | **ON** | Second passage SG après ancrage |
| Lissage des pentes de segment | `segmentSmooth` | **ON** | Médiane mobile à 3 sur les pentes du log debug |

### 4.5 Constantes internes (non exposées)

| Nom technique | Valeur | Rôle |
|---|---|---|
| `REFINE_MIN_M` | 1500 | Plancher garde-fou hybride |
| `REFINE_MAX_M` | 8000 | Plafond garde-fou hybride |
| `STEP_M_MIN` | 2 | Pas resampling min |
| `STEP_M_MAX` | 200 | Pas resampling max |
| `GAP_MIN_M_DEFAULT` | 250 | Fusion |
| `SLOPE_DISPLAY_MAX_PCT` | 25 | Écrêtage pentes affichées |
| `PLATEAU_TOLERANCE_M` | 1.0 | Tolérance plateaux |
| `RAW_DOUBLON_MIN_M` | 1.0 | Seuil doublon GPS |
| **`DESPIKE_WINDOW_M_DEFAULT`** | **200** | **Fenêtre anti-pic par défaut (m)** |
| **`DESPIKE_WINDOW_M_MIN`** | **50** | **Plancher fenêtre anti-pic (m)** |
| **`DESPIKE_WINDOW_M_MAX`** | **500** | **Plafond fenêtre anti-pic (m)** |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 | Seuil anti-pic |
| **`RESAMPLE_AVG_MIN_M`** | **75** | **Fenêtre minimale de moyennage (v3.5)** |
| `INSERT_EDGE_MIN_KM` | 1.0 | Espace avant/après insertion |
| `INSERT_BETWEEN_MIN_KM` | 0.2 | Espace entre montées insertion |
| `INSERT_DEFAULT_LEN_KM` | 1.0 | Longueur cible insertion |

### 4.6 Guide de réglage anti-pic

| Type de trace | Fenêtre | Seuil | Justification |
|---|---|---|---|
| Trace GPS capteur | 200 m | 20–30 m | Bruit GPS peut atteindre 30 m |
| **Trace éditeur SRTM** | **200 m** | **15 m** | **Glitch SRTM = 10–30 m** |
| Trace très propre | 100 m | 10 m | Détection fine |
| Vraies bosses larges | 200 m | 25 m | Préserver les reliefs de +25 m |
| Séries de glitchs | 300–500 m | 15–20 m | Corriger des groupes de glitches |
| Désactiver complètement | — | **0** | Comportement avant anti-pic |

### 4.7 Guide de cohérence multi-paramètres

| Règle | Formule | Conséquence si violée |
|---|---|---|
| Fenêtre SG ≥ 10 pts | `fenetreAffichageM ≥ 10 × stepM` | Lissage altitude inefficace |
| Ratio altitude/pente ≥ 2 | `fenetreAffichageM ≥ 2 × fenetrePenteM` | Pente plus lisse que l'altitude |
| Filtre médian ≥ 3 pts | `medianWindowM ≥ 3 × stepM` (ou 0) | Filtre médian inefficace |
| Gaussien σ ≥ 0.5 pt | `gaussianSigmaM ≥ 0.5 × stepM` (ou 0) | Filtre gaussien inefficace |
| Longueur min. pente ≥ Pas | `longueurMin ≥ stepM` (ou 0) | Paramètre inopérant |
| Écart min. ≥ Pas | `gapMinM ≥ stepM` (ou 0) | Paramètre inopérant |
| **Anti-pic fenêtre ≥ 2 × Pas** | **`despikeWindowM ≥ 2 × stepM`** (ou seuil = 0) | **Arrondi force une fenêtre > demandée** |
| Anti-pic seuil ≥ 5 m | `despikeThresholdM ≥ 5` (ou 0) | Risque d'écrêter des bosses |

---

## 5. Parcours utilisateur

### 5.1 Parcours nominal

1. L'utilisateur ouvre la page → zone de dépôt visible
2. Il glisse son fichier GPX → analyse automatique lancée
3. Le pipeline effectue :
   - Parse GPX → distances cumulées + altitudes brutes
   - Stats brutes (D+, D−, altitudes min/max, distribution des pas)
   - Correction plateaux (optionnelle)
   - **Rééchantillonnage par moyenne à pas fixe** (stepM)
   - **Filtre anti-pic** (fenêtre en mètres)
   - Pipeline de détection sur données nettoyées
4. Affichage :
   - Statistiques globales (avec `n pts bruts → m pts resamplés`)
   - **Panneau Diagnostic de configuration**
   - Bande de segmentation colorée
   - Profil altimétrique
   - Graphe Pente replié
   - Tableau éditable des montées avec indicateurs `⊕` et `⤺`
   - Drop-zone en mode compact
5. L'utilisateur ajuste les paramètres → recalcul en direct
6. Il sélectionne une montée → surbrillance + poignées draggables
7. Il glisse les poignées → métriques temps réel, tooltip masqué
8. Il peut insérer de nouvelles montées
9. Il peut désélectionner (reclic radio ou Échap)

### 5.2 Parcours d'erreur

| Cas | Comportement attendu |
|---|---|
| Fichier non-XML | Alerte, pas de chargement |
| GPX sans balise `<ele>` | Alerte, pas de chargement |
| Fichier très lourd (> 50 000 points) | Resampling automatique, latence acceptable |
| Édition créant un chevauchement | Contrainte automatique + `⚠` |
| Édition créant un segment invalide | Ligne `invalid`, métriques `—` |
| Drag hors zone cliquable | Fallthrough vers le pan |
| Montée sous le seuil après fusion | Repli sur sous-segment valide (`⤺`) |
| Montée sans sous-segment valide | Rejetée (log debug) |
| **Anti-pic fenêtre < 2 × Pas** | **Warning diagnostic (🟠)** |
| Anti-pic seuil < 5 m | Warning diagnostic (🟠) |

---

## 6. Architecture technique

### 6.1 Stack

| Composant | Choix | Justification |
|---|---|---|
| Langage | HTML / CSS / JavaScript vanilla | Aucun build, portable |
| Visualisation | Chart.js 4.4.1 | Léger, plugin-zoom |
| Interaction | chartjs-plugin-zoom 2.0.1 + Hammer.js 2.0.8 | Molette, pinch, pan |
| Backend | Aucun | Confidentialité, simplicité |

### 6.2 Pipeline complet (v3.6)

```
┌─────────────────────────────────────────────────────────────┐
│ CHARGEMENT                                                  │
├─────────────────────────────────────────────────────────────┤
│ 1. Parse GPX → rawPoints[]                                  │
│ 2. distRaw[] = cumsum(haversine)                            │
│ 3. eleRawOrig[] = altitudes GPX d'origine                   │
│ 4. rawStats = computeRawStats(distRaw, eleRawOrig)          │
│    → D+, D−, min/max, distribution des pas, doublons        │
│ 5. [Optionnel] eleCorrected = plateauCorrect(eleRawOrig)    │
│ 6. resampleUniform(distRaw, eleCorrected, stepM)            │
│    → dist[], ele[] (pas fixe, moyennage fenêtre ≥ 75 m)     │
│ 7. Conversion fenêtre anti-pic m → pts :                    │
│    winPts = max(3, round(despikeWindowM / stepM) | 1)       │
│    despikeAltitude(ele, winPts, thresholdM)                 │
│    → eleFinal[] (Hampel)                                    │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ DÉTECTION AUTO (computeAuto)                                │
├─────────────────────────────────────────────────────────────┤
│ 1. Filtre médian (medianWindowM → pts)                      │
│ 2. Savitzky-Golay ordre 2 (fenetreAffichageM → pts)         │
│ 3. Pente lissée = MA(computeSlope, fenetrePenteM → pts)     │
│ 4. Hystérésis (seuilMontee) → événements                    │
│ 5. Candidats = buildClimbCandidates                         │
│ 6. Sous-segment optimal par candidat                        │
│ 7. Raffinement (argmin/argmax dans ±refinePct%)             │
│ 8. FUSION (gapMinM) : étend la montée précédente            │
│ 9. Lissage ancré final (SG + gaussien + doubleSG optionnel) │
│ 10. VALIDATION STRICTE :                                    │
│     - pente moyenne sur bornes FINALES                      │
│     - si échec → repli sur findBestSubClimb (⤺)             │
│     - si aucun sous-segment → rejet                         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ REBUILD (à chaque édition)                                  │
├─────────────────────────────────────────────────────────────┤
│ 1. Contraintes anti-chevauchement                           │
│ 2. Ancrage sur les bornes utilisateur                       │
│ 3. Lissage ancré (eleDisplay)                               │
│ 4. Pentes (raw / display / cat)                             │
│ 5. Segmentation par catégorie                               │
│ 6. Marqueurs vallée/sommet                                  │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Modèle de données en mémoire

```javascript
STATE = {
  // Données brutes
  rawPoints: Array<{lat, lon, ele}>,
  distRaw: number[],
  eleRawOrig: number[],
  rawStats: {
    n, dPlus, dMoins,
    gapMin, gapMed, gapMax, gapMean,
    eleMin, eleMax, doublons
  },

  // Données resamplées + nettoyées
  dist: number[],
  eleRaw: number[],      // après resampling (moyennage) ET anti-pic
  stepM: number,
  avgWinM: number,       // [v3.5] fenêtre de moyennage utilisée
  despikeWinPtsEffective: number,  // [v3.6] fenêtre anti-pic effective en points
  eleDisplay: number[],  // altitudes lissées ancrées

  // Montées
  autoClimbs: Array<{
    startKm, endKm,
    refineM, candidateLen,
    mergedCount, fallback
  }>,
  userClimbs: Array<{
    startKm, endKm,
    autoStartKm, autoEndKm,
    refineM, candidateLen,
    mergedCount, fallback,
    _constrainedStart, _constrainedEnd
  }>,

  // UI
  fileName, selectedClimbIdx, showRawDetail,
  draggingHandle, dragPreview, panOriginalState,

  // Charts
  elevChart, slopeChart, syncing
}
```

### 6.4 Ordre du code (fichier source)

```
1.  Constantes (REFINE_*, STEP_*, GAP_MIN_*, PLATEAU_*,
                 DESPIKE_WINDOW_*, DESPIKE_THRESHOLD_*,
                 RESAMPLE_AVG_MIN_M)
2.  Debug logger (dbg, dbgInfo, dbgWarn, dbgRaw, dbgResample,
                  dbgMerge, dbgFallback, dbgPlateau, dbgDespike)
3.  Diagnostic (computeDiagnostics, updateDiagnostics)
4.  Repli/dépli graphe pente
5.  Utilitaires (haversine, movingAverage, savitzkyGolay,
                 gaussianFilter, medianFilter, computeSlope,
                 computeSlopeWindow, clipSlope, ...)
6.  resampleUniform (par moyenne, v3.5)
7.  plateauCorrect
8.  despikeWindowMToPts + despikeAltitude (v3.6)
9.  computeRawStats
10. anchoredMovingAverage
11. Catégories + critères
12. Parseur GPX
13. mergeShortSegments, enrichSegment, segmentByCategory
14. detectEvents, buildClimbCandidates, findBestSubClimb
15. computeAuto (avec FUSION + VALIDATION STRICTE)
16. STATE + getParams
17. rebuild
18. Plugins Chart.js (segmentation, markerDrag)
19. Helpers drag (applyDragConstraint, updatePreviewMetrics)
20. Helpers d'insertion contextuelle
21. logClimbSegments
22. renderAll, climbRowHtml
23. Sélection/désélection
24. Zoom/sync
25. loadFile (avec resampling par moyenne + anti-pic en mètres)
26. recomputeAuto
27. Interface (boutons, toggles, table)
28. Édition clavier
29. Interception DOM drag
30. Gestion du drag
31. Survol des poignées
32. Enregistrement des plugins
```

---

## 7. Exigences non-fonctionnelles

| Exigence | Cible |
|---|---|
| Temps de calcul initial (10 000 pts, stepM=25) | < 3 secondes |
| Temps de recalcul après édition | < 500 ms |
| Latence pendant drag (par frame) | < 16 ms (60 fps) |
| Compatibilité navigateurs | Chrome, Firefox, Safari, Edge (2 dernières versions) |
| Support mobile | Responsive, lisible sur tablette (≥ 768 px) |
| Support tactile drag | Touch events gérés |
| Confidentialité | Aucune donnée envoyée |
| Accessibilité | Contraste WCAG AA, textes ≥ 12 px |
| Persistance | Aucune (état en mémoire) |

---

## 8. Hors périmètre (V3.6)

- Carte géographique (OpenStreetMap, satellite)
- Export PDF / PNG du profil
- Sauvegarde de la configuration utilisateur (localStorage)
- Comparaison multi-tracks
- Détection automatique de « cols »
- Export CSV / JSON des montées
- Import FIT, TCX
- Édition des seuils de catégorie

---

## 9. Évolutions envisagées (V4+)

| Priorité | Fonctionnalité |
|---|---|
| P1 | Export PNG / PDF avec annotations |
| P1 | Sauvegarde/chargement de la config (localStorage) |
| P1 | Undo/Redo des éditions |
| P1 | Édition des critères de validation dans l'UI |
| P2 | Carte OSM synchronisée avec le zoom |
| P2 | Regroupement automatique en « cols » |
| P2 | Import FIT, TCX |
| P2 | Snap magnétique sur les extrema bruts pendant le drag |
| P3 | Export CSV des montées détectées |
| P3 | Traduction anglaise |
| P3 | Anti-pic adaptatif (détection automatique de la fenêtre) |

---

## 10. Risques et mitigation

| Risque | Impact | Mitigation |
|---|---|---|
| Variabilité des traces GPS | Faux positifs/négatifs | Anti-pic + double lissage + édition |
| Complexité des paramètres | Frustration utilisateur | Diagnostic visuel + valeurs par défaut |
| Trace très longue | Lenteur | Sous-échantillonnage + Float64Array |
| Effet de bord entre paramètres | Régressions | Diagnostic de cohérence + tests |
| Catégorisation trop sensible | Taches noires | Fenêtre glissante |
| Chevauchement éditions | Incohérence | Contrainte auto + indicateur |
| Conflit drag / zoom-pan | Impossible de draguer | Capture DOM + désactivation pan |
| Tooltip masquant pendant drag | Courbes cachées | Désactivation tooltip |
| Insertion gap trop étroit | Chevauchement | Bouton masqué si gap < 200 m |
| Anti-pic trop zélé | Écrêtage de vraies bosses | Diagnostic avertit si seuil < 5 m |
| Anti-pic inefficace | Glitch persiste | Diagnostic avertit si fenêtre < 2 × Pas |
| **Bruit résiduel à petit Pas** | **Signal bruité malgré Pas fin** | **Rééchantillonnage par moyenne (v3.5)** |
| **Fenêtre anti-pic dépendante du Pas** | **Comportement imprévisible** | **Fenêtre en mètres (v3.6)** |

---

## 11. Critères d'acceptation

| Réf | Critère | Méthode de test |
|---|---|---|
| CA-01 | Fichier GPX (10 000 pts) chargé et analysé en < 3 s | Chronométrage |
| CA-02 | Sur trace Morzine, les 7 montées sont détectées | Comparaison visuelle |
| CA-03 | Édition du km de vallée → mise à jour < 500 ms | Chronométrage |
| CA-04 | Boutons `↺`, `✕`, « Recharger auto » fonctionnels | Test manuel |
| CA-05 | Zoom synchronisé | Molette |
| CA-06 | Aucun crash sur fichier sans `<ele>` | Fichier corrompu |
| CA-07 | Aucune zone blanche entre segments | Zoom visuel |
| CA-08 | Chevauchement empêché + `⚠` | Édition volontaire |
| CA-09 | 8 catégories affichées | Légende |
| CA-10 | Altitude ancrée passe exactement par les extrema | Zoom |
| CA-11 | Drag & drop d'une poignée fonctionne | Test manuel |
| CA-12 | Tooltip masqué pendant drag, restauré après | Test visuel |
| CA-13 | Boutons insertion visibles aux bons endroits | Vérification |
| CA-14 | Désélection par reclic radio ou Échap | Test manuel |
| CA-15 | Drop-zone reste active après chargement | Recharger |
| CA-16 | Toggle brut : 3 états visuels | Cocher/décocher |
| CA-17 | Graphe Pente replié par défaut | Observation |
| CA-18 | Colonne Extrema affiche valeur par montée | Tableau |
| CA-19 | Aucune montée sous son seuil de pente | Post-chargement |
| CA-20 | Montées < gapMinM fusionnées avec `⊕` | Trace fragmentée |
| CA-21 | Repli sur sous-segment visible `⤺` | Trace avec fusion |
| CA-22 | Stats D+ brut cohérentes avec trace d'origine | Comparaison |
| CA-23 | Pas de doublons GPS comptés dans D+ | Trace avec duplicats |
| CA-24 | Anti-pic corrige un glitch isolé (baisse sur graphe pente) | Trace avec pic + win=200m, seuil=15m |
| CA-25 | Anti-pic ne modifie pas une vraie bosse (seuil > relief réel) | Trace + seuil=25 m |
| CA-26 | Diagnostic avertit si fenêtre < 2 × Pas ou seuil < 5 m | Régler fenêtre=50m @ Pas=50 |
| CA-27 | Rendu visuel à Pas=50 m comparable à Pas=25 m en termes de propreté | Trace GPS dense + comparaison |
| CA-28 | Diagnostic 🟢 avec les défauts v3.6 | Chargement sans modification |
| CA-29 | `avgWinM` affiché dans le log Debug | Vérification log |
| CA-30 | Renommages appliqués : `Fenêtre de calcul pente`, `Longueur min. pente` | Inspection UI |
| **CA-31** | **`Anti-pic : fenêtre` affichée en mètres dans le formulaire et le diagnostic** | **Inspection UI** |
| **CA-32** | **Fenêtre effective en points affichée sous le champ** | **Inspection UI** |
| **CA-33** | **Comportement anti-pic indépendant du Pas (test 25/50/100 m)** | **Changer Pas et vérifier** |

---

## 12. Traces de référence

| Nom | Distance | Caractéristique | Utilisation |
|---|---|---|---|
| Bormio–Mortirolo 2025 | ~110 km | Trace éditeur Openrunner, MNT SRTM | Trace principale |
| Trace GPS terrain dense | variable | Pas de 3–5 m, bruit capteur | Test haute densité |
| Trace éditeur fragmentée | 80 km | Multiples faux-plats | Test fusion |
| Glitch isolé | 40 km | 1–2 points aberrants | Test anti-pic |
| Glitch + fusion | 60 km | Glitch + fragment | Test anti-pic + fusion |
| Trace plate | 50 km | Aucune montée | Anti-faux-positifs |

---

## 13. Interface utilisateur — wireframe textuel

```
┌─────────────────────────────────────────────────────────────────┐
│   Analyse GPX — Montées éditables                               │
│  Anti-pic + rééchantillonnage par moyenne + fusion + validation.│
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │   trace.gpx · 3312 pts bruts → 2190 resamplés (pas 50 m)  │  │
│  │     — glissez un nouveau fichier ou cliquez pour changer  │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌── Paramètres ─────────────────────────────────────────────┐  │
│  │  🔍 Diagnostic de configuration           🟢 Cohérent     │  │
│  │  ─────────────────────────────────────────────────────    │  │
│  │  ● Pas ................... 50 m                          │  │
│  │  ● Anti-pic : fenêtre .... 200 m · 5 pts ≈ 250 m         │  │
│  │  ● Anti-pic : seuil ...... 15 m                          │  │
│  │  ● Lissage altitude ...... 10 points                     │  │
│  │  ● Lissage pente ......... 3 points                      │  │
│  │  ● Fenêtre de calcul pente  2 points                     │  │
│  │  ● Longueur min. pente ... 200 m                         │  │
│  │  ...                                                      │  │
│  │                                                           │  │
│  │  🩹 Anti-pic                                              │  │
│  │  Fenêtre [ 200 ] m    Seuil [ 15 ] m                     │  │
│  │  → 5 pts ≈ 250 m      Remplace les points aberrants       │  │
│  │                                                           │  │
│  │  📐 Rééchantillonnage par moyenne                         │  │
│  │  Pas [ 50 ] m                                             │  │
│  │  fenêtre de moyennage ≥ 75 m                              │  │
│  │                                                           │  │
│  │  📊 Lissages                                              │  │
│  │  Lissage altitude [ 500 ] m   Lissage pente [ 150 ] m    │  │
│  │  Fenêtre de calcul pente [ 100 ] m                       │  │
│  │                                                           │  │
│  │  🎯 Détection                                             │  │
│  │  Seuil [ 0.5 % ]  Long. min. pente [ 200 m ]             │  │
│  │  Écart min [ 250 m ]  Recherche extrema [ 60 % ]         │  │
│  │                                                           │  │
│  │  ⚙️ Avancé                                                │  │
│  │  Filtre médian [ 0 m ]   Gaussien [ 75 m ]               │  │
│  │  Plateaux [ 6 pts ]      Segments debug [ 100 m ]        │  │
│  │                                                           │  │
│  │  ☑ Lissage renforcé    ☑ Lissage pentes de segment       │  │
│  │                                                           │  │
│  │  [ ↺ Recharger auto ]                                    │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌── Statistiques ──────────────────────────────────────────┐   │
│  │ Dist. D+ brut D− brut Alt.min Alt.max Montées D+m Km pts│   │
│  │ 110   2874   2862    545     2754    3(2⊕)   1600 45 3312│   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Segmentation par catégorie ────────────────────────────┐   │
│  │  [█████████████████████████████████████████████]         │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Profil altimétrique  ☐ Mettre en avant brut ───────────┐   │
│  │  ▲drag                                             ▼drag │   │
│  │  [Graphique avec poignées + bande jaune]                 │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──  Pente (%)                                    ▶   ─────┐   │
│  │  Cliquez pour afficher                                    │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Stats catégories ───◊┐  ┌── Montées éditables ───────┐    │
│  │ Cat. | Dist | D+ | %   │  │        [ + Avant #1 ]      │    │
│  │ Orange| 12.5 | 340|4.2 │  │ ☑ #1 | 3.42 | 8.15 | ...   │    │
│  │ Rouge |  2.1 | 120|8.5 │  │        [ + Entre #1 et #2] │    │
│  │ Noir  |  0.4 |  45|12.3│  │ ☐ #2⊕| 10.02 | 15.34|...   │    │
│  │                        │  │ ☐ #3⤺| 20.5 | 24.8 | ...   │    │
│  └────────────────────────┘  └────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 14. Glossaire

| Terme | Définition |
|---|---|
| **GPX** | GPS Exchange Format — format XML standardisé pour l'échange de données GPS |
| **Vallée** | Point bas précédant une montée |
| **Sommet** | Point haut terminant une montée |
| **D+** | Dénivelé positif cumulé (somme des montées) |
| **D−** | Dénivelé négatif cumulé |
| **MNT** | Modèle Numérique de Terrain (ex. SRTM à 30 m) |
| **SRTM** | Shuttle Radar Topography Mission (NASA, 2000) |
| **Hystérésis** | Technique de détection avec seuil aller et seuil retour distincts |
| **Argmin / argmax** | Indice du minimum / maximum d'un tableau dans un intervalle |
| **Lissage ancré** | Moyenne mobile forcée à passer exactement par certains points |
| **Fenêtre glissante** | Intervalle mobile de N mètres autour d'un point de référence |
| **Pente instantanée** | Pente calculée sur 3 points consécutifs (différence centrée) |
| **Pente fenêtrée** | Pente calculée sur une portion de N mètres |
| **Sous-échantillonnage** | Réduction du nombre de points pour performance |
| **Garde-fou hybride** | `clamp(pct × longueur, min, max)` pour la recherche des extrema |
| **Drag & drop** | Édition directe des poignées vallée/sommet sur le graphique |
| **Insertion contextuelle** | Bouton `+` entre les lignes du tableau pour créer une montée |
| **Désélection** | Sortir du mode édition d'une montée (reclic radio ou Échap) |
| **Drop-zone persistante** | Zone de dépôt toujours visible, mode compact après chargement |
| **Rééchantillonnage** | Conversion d'une série irrégulière en pas fixe |
| **Rééchantillonnage par moyenne** | Chaque point de sortie = moyenne des points bruts dans une fenêtre centrée (v3.5) |
| **Fusion (`⊕`)** | Montée étendue pour englober une suivante trop proche |
| **Repli (`⤺`)** | Repli sur le meilleur sous-segment valide |
| **SG** | Savitzky-Golay, lisseur polynomial |
| **Plateau GPS/MNT** | Série d'altitudes identiques (capteur figé ou même cellule MNT) |
| **Anti-pic** | Filtre médian conditionnel — ne remplace que les points aberrants (v3.3+) |
| **Fenêtre anti-pic** | Largeur (en mètres) du voisinage inspecté (v3.6) |
| **Diagnostic** | Panneau de cohérence en temps réel des paramètres |
| **Valeur effective** | Valeur calculée en points (= paramètre en mètres ÷ stepM) |

---

## 15. Historique des versions

| Version | Date | Changements majeurs |
|---|---|---|
| 0.1 | 2026-09-20 | Prototype : profil + pente + détection basique |
| 0.2 | 2026-09-20 | Segmentation 7 catégories |
| 0.3 | 2026-09-20 | Détection stricte |
| 0.4 | 2026-09-20 | Double lissage + affichage brut vs lissé |
| 0.5 | 2026-09-20 | Ancrage sur extrema bruts |
| 0.6 | 2026-09-20 | Validation sur valeurs lissées |
| 0.7 | 2026-09-21 | Édition manuelle |
| 0.8 | 2026-09-21 | Anti-chevauchement + garde-fou fenêtre max |
| 0.9 | 2026-09-21 | 8ᵉ catégorie Montée très raide |
| 0.10 | 2026-09-21 | Catégorisation par fenêtre glissante |
| 1.0 | 2026-09-21 | Segments contigus — version stable |
| 2.0 | 2026-09-21 | Garde-fou hybride `clamp(pct × len, 1500, 8000)` + colonne Refine |
| 2.1 | 2026-09-21 | Tooltip masqué pendant drag + `longueurMin = 250` + graphe Pente repliable |
| 2.2 | 2026-09-21 | Insertion contextuelle de montées |
| 2.3 | 2026-09-21 | Drop-zone persistante + désélection + toggle brut 3 états |
| 2.8 | 2026-09-21 | Pipeline 6 étapes : médiane + plateaux + SG + gaussien + double SG |
| 2.9 | 2026-09-22 | Rééchantillonnage uniforme + paramètres en mètres |
| 3.0 | 2026-09-22 | Fusion des montées trop proches (`gapMinM`) |
| 3.1 | 2026-09-22 | Validation finale stricte + repli sur sous-segment (`⤺`) |
| 3.2 | 2026-09-23 | Diagnostic de configuration + feu tricolore + valeur effective |
| 3.3 | 2026-09-23 | Filtre anti-pic Hampel + 2 paramètres UI |
| 3.4 | 2026-09-23 | Pas par défaut 25 m · Lissage altitude 400 m · Filtre médian désactivé |
| 3.5 | 2026-09-23 | Rééchantillonnage par moyenne (fenêtre ≥ 75 m) au lieu d'interpolation |
| 3.5.1 | 2026-09-23 | Fenêtre de calcul pente 100 m · Longueur min. pente 200 m |
| 3.5.2 | 2026-09-23 | Harmonisation du vocabulaire : `Fenêtre de calcul pente`, `Longueur min. pente` |
| **3.6** | **2026-09-24** | **Anti-pic : fenêtre en mètres (50–500, défaut 200) · Pas par défaut 50 m · Lissage altitude 500 m** |

---

## 16. Annexes

### 16.1 Formule de Haversine

```
a = sin²(Δφ/2) + cos(φ1) × cos(φ2) × sin²(Δλ/2)
c = 2 × atan2(√a, √(1−a))
d = R × c    avec R = 6 371 000 m
```

### 16.2 Rééchantillonnage par moyenne (v3.5)

```
Pour chaque position cible k × stepM :
  - Calculer la fenêtre de moyennage :
      avgWin = clamp(stepM, 75 m, 3 × stepM)
  - Moyenner tous les points bruts dont la distance cumulée tombe
    dans [k × stepM − avgWin/2, k × stepM + avgWin/2]
  - Si aucun point dans la fenêtre : fallback par interpolation linéaire
```

### 16.3 Anti-pic Hampel avec fenêtre en mètres (v3.6)

```
Pour chaque point i :
  - Convertir la fenêtre en points :
      winPts = max(3, round(despikeWindowM / stepM) | 1)
  - Calculer la médiane M d'une fenêtre glissante [i − half, i + half]
    avec half = (winPts − 1) / 2
  - Si |ele[i] − M| > thresholdM : ele[i] ← M
  - Sinon : conserver ele[i]
```

**Exemples de conversion** :

| Pas | Fenêtre (m) | winPts effectif | Couverture |
|---|---|---|---|
| 50 m | 200 m | 5 | ±100 m (250 m total) |
| 50 m | 400 m | 9 | ±200 m (450 m total) |
| 25 m | 200 m | 9 | ±100 m (225 m total) |
| 100 m | 200 m | 3 | ±100 m (300 m total) |

### 16.4 Fusion par distance (v3.0)

```
Si (nouvelle montée startM − précédent endM) < gapMinM :
  - Nouveau sommet = argmax entre précédent.endIdx et nouveau.endIdx
  - Nouveau creux = argmin entre précédent.endIdx et nouveau.startIdx
                   si plus bas que précédent.startIdx
  - Étendre la montée précédente et abandonner la nouvelle
```

### 16.5 Validation finale stricte (v3.1)

```
Pour chaque montée raffinée+fusionnée :
  length = dist[c.endIdx] − dist[c.startIdx]
  avgSlope = D+ / length

  Si avgSlope < seuil(length) :
    sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx)
    Si sub ≠ null :
      → repli (⤺) : bornes = [sub.startIdx, sub.endIdx]
    Sinon :
      → rejet complet
```

### 16.6 Critères de validation

```
seuil(L) = 3 % si L ≥ 1000 m
         = 4 % si 750 ≤ L < 1000
         = 5 % si 500 ≤ L < 750
         = 6 % si 400 ≤ L < 500
         = 7 % si 300 ≤ L < 400
         = ∞   si L < 300
```

### 16.7 Valeurs par défaut v3.6

| Paramètre | Défaut | Unité |
|---|---|---|
| Pas de rééchantillonnage | **50** | m |
| Anti-pic : fenêtre | **200** | m |
| Anti-pic : seuil | **15** | m |
| Lissage altitude | **500** | m |
| Lissage pente | **150** | m |
| Fenêtre de calcul pente | **100** | m |
| Seuil détection | **0.5** | % |
| Longueur min. pente | **200** | m |
| Écart min. entre montées | **250** | m |
| Recherche extrema | **60** | % |
| Filtre médian | **0** (désactivé) | m |
| Lissage gaussien | **75** | m |
| Correction plateaux | **6** | pts |
| Segments debug | **100** | m |
| Double SG | **ON** | — |
| Lissage segments | **ON** | — |

Diagnostic attendu avec ces défauts : 🟢 **Cohérent — aucun avertissement**.

**Configuration cohérente recommandée** :

```
Pas ............................. 50 m
Anti-pic : fenêtre .............. 200 m
Anti-pic : seuil ................ 15 m
Lissage altitude ................ 500 m
Lissage pente ................... 150 m
Fenêtre de calcul pente ......... 100 m
Seuil détection ................. 0.5 %
Longueur min. pente ............. 200 m
Écart min. entre montées ........ 250 m
Recherche extrema ............... 60 %
Filtre médian ................... 0
Lissage gaussien ................ 75 m
Correction plateaux ............. 6 pts
```

**Configuration pour trace GPS dense (Pas = 25 m)** :

```
Pas ............................. 25 m
Anti-pic : fenêtre .............. 200 m
Anti-pic : seuil ................ 20 m
Lissage altitude ................ 400 m
Lissage pente ................... 150 m
Filtre médian ................... 0
Lissage gaussien ................ 75 m
```

---

**Fin du PRD v3.6**