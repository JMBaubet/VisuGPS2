# PRD — Analyseur de profil altimétrique GPX

**Nom de code** : GPX Climb Analyzer
**Version** : 3.9.2
**Date** : 2026-10-17
**Statut** : Spécification produit — application web monopage (SPA)
**Auteur** : [à compléter]
**Destinataire** : développeur front-end, product owner

---

## Table des matières

1. [Contexte et objectif](#1-contexte-et-objectif)
2. [Utilisateurs cibles](#2-utilisateurs-cibles)
3. [Périmètre fonctionnel](#3-périmètre-fonctionnel)
   - [3.1 Chargement du fichier](#31-chargement-du-fichier)
   - [3.2 Prétraitement du signal](#32-prétraitement-du-signal)
   - [3.3 Détection automatique](#33-détection-automatique-des-montées)
   - [3.4 Critères de validation](#34-critères-de-validation-dune-montée)
   - [3.5 Édition manuelle](#35-édition-manuelle-des-montées)
   - [3.6 Segmentation par catégorie](#36-segmentation-par-catégorie-de-pente)
   - [3.7 Visualisation — Carte Mapbox 3D](#37-visualisation--carte-mapbox-3d)
   - [3.8 Visualisation — Widget profil](#38-visualisation--widget-profil-altimétrique)
   - [3.9 Groupes de montées](#39-groupes-de-montées)
   - [3.10 Création manuelle](#310-création-manuelle-dune-montée)
   - [3.11 Suppression](#311-suppression-dune-montée)
   - [3.12 Statistiques](#312-statistiques-globales)
   - [3.13 Export JSON](#313-export-json)
   - [3.14 Interface — Layout 3 colonnes](#314-interface--layout-3-colonnes)
4. [Paramètres utilisateur](#4-paramètres-utilisateur)
5. [Parcours utilisateur](#5-parcours-utilisateur)
6. [Architecture technique](#6-architecture-technique)
7. [Exigences non-fonctionnelles](#7-exigences-non-fonctionnelles)
8. [Hors périmètre (V3.9.2)](#8-hors-périmètre-v392)
9. [Évolutions envisagées (V4+)](#9-évolutions-envisagées-v4)
10. [Risques et mitigation](#10-risques-et-mitigation)
11. [Critères d'acceptation](#11-critères-dacceptation)
12. [Traces de référence](#12-traces-de-référence)
13. [Interface — wireframe textuel](#13-interface--wireframe-textuel)
14. [Glossaire](#14-glossaire)
15. [Historique des versions](#15-historique-des-versions)
16. [Annexes](#16-annexes)

---

## 1. Contexte et objectif

### 1.1 Contexte

Les cyclistes, randonneurs et traileurs utilisent des traces GPX pour préparer ou analyser leurs sorties. Les traces proviennent soit de relevés GPS terrain (montres, téléphones), soit — et c'est le cas principal supporté — d'**éditeurs de trace** (Openrunner, Garmin Connect, Komoot) qui génèrent les points à partir d'un algorithme de simplification et d'un MNT (Modèle Numérique de Terrain, typiquement SRTM).

Trois problèmes majeurs apparaissent :

1. **Irrégularité d'espacement** : les points sont densément regroupés dans les virages et clairsemés dans les lignes droites (facteur 40–150× observé sur trace Openrunner)
2. **Glitchs d'altitude isolés** : artefacts du capteur GPS ou du MNT produisant un pic de quelques dizaines de mètres sur 1–2 points, amplifié par la dérivée (pente) jusqu'à ±25 % et produisant une fausse montée
3. **Plateaux MNT** : séries d'altitudes identiques correspondant aux cellules SRTM de 30 m, créant des marches d'escalier

Depuis la v3.8, l'application a été refondue autour d'un layout 3 colonnes et d'une intégration Mapbox 3D, permettant une visualisation géographique du parcours parallèlement au profil altimétrique. La v3.9.x a introduit la gestion des groupes de montées (séparation/refusion), la création manuelle et la suppression — répondant aux besoins d'édition fine exprimés par les utilisateurs avancés.

### 1.2 Objectif

Fournir un outil web autonome permettant :

- de charger une trace GPX (drop-zone persistante),
- d'afficher une **carte Mapbox 3D** avec terrain DEM, courbes de niveau optionnelles, et trace colorée par catégorie de pente,
- d'afficher un **widget profil altimétrique** en canvas 2D natif avec échelles fixes,
- de **rééchantillonner par moyenne** à pas fixe,
- d'**éliminer les glitchs d'altitude isolés** (filtre anti-pic en mètres),
- de **corriger les plateaux MNT**,
- de détecter automatiquement les montées significatives,
- d'**ajuster manuellement** les bornes par drag & drop direct sur le profil,
- de **séparer** les montées fusionnées en leurs sous-montées d'origine (avec persistance du lien de groupe),
- de **refusionner** un groupe de montées,
- d'**insérer** de nouvelles montées par double-clic + drag sur le profil,
- de **supprimer** des montées individuelles,
- de segmenter par catégorie de pente (identique entre carte et profil),
- d'afficher un **diagnostic de configuration** en temps réel,
- d'**exporter la segmentation et les montées au format JSON** pour consommation externe (affichage cartographique Mapbox, visuels de montée).

### 1.3 Proposition de valeur

| Axe | Bénéfice utilisateur |
|---|---|
| Robustesse signal | Rééchantillonnage par moyenne + anti-pic en mètres → comportement identique quelle que soit la trace |
| Transparence | Tous les paramètres en mètres, diagnostic visuel en direct, valeurs effectives en points |
| Édition naturelle | Drag & drop des marqueurs sur le profil, surbrillance carte en temps réel |
| **Vision 3D** | Carte Mapbox terrain + contours, trace colorée par pente |
| Anti-fragmentation | Fusion automatique (`⊕`) + séparation (`✂`) + refusion (`🔗`) |
| Anti-régression | Validation stricte post-raffinement avec repli sur sous-segment (`⤺`) |
| Anti-glitch | Filtre anti-pic ciblé, ne modifie que les points aberrants |
| **Fidélité carte** | Trace sur la carte = points bruts du GPX (pas la grille resamplée) |
| **Groupes persistants** | `groupId`/`subIndex`, numérotation `#N.M`, couleur stable par groupe |
| Confidentialité | 100 % côté navigateur, aucun envoi sur un serveur (sauf tuiles Mapbox) |
| Richesse | Analyse multi-niveaux : segmentation + montées significatives + carte |
| Cohérence | Anti-chevauchement, anti-zones-blanches, catégorisation par fenêtre |
| Prévisibilité | Fenêtre anti-pic indépendante du Pas (v3.6) |
| Interopérabilité | Export JSON léger, progressions 0–1 prêtes pour `line-gradient` Mapbox (v3.7) |

### 1.4 Vision produit à terme

À terme, l'outil doit devenir une **plateforme d'analyse et d'édition de traces GPX** couvrant :
- L'analyse altimétrique fine (existante)
- La préparation et la modification de traces (existante via édition des montées)
- L'export cartographique et l'intégration dans des applications tierces (existante via JSON)
- L'édition de la trace elle-même (dans le périmètre V4+)

La v3.9.2 est une étape majeure : elle introduit la carte 3D, le layout 3 colonnes, la gestion des groupes, la création manuelle et la suppression. Les prochaines versions se concentreront sur l'ergonomie (undo/redo), la persistance (localStorage), et l'interopérabilité étendue (import FIT/TCX).

---

## 2. Utilisateurs cibles

| Profil | Usage principal | Attentes |
|---|---|---|
| **Cycliste route / gravel** | Analyser une étape avant de la rouler | Identifier les cols, D+, pente moyenne, ajuster les bornes, ajouter des montées |
| **Randonneur / traileur** | Évaluer la difficulté d'un parcours | Voir les portions raides, D+ cumulé, carte 3D pour visualiser le terrain |
| **Organisateur d'événement** | Vérifier un tracé | Ajouter des montées non détectées, séparer des fusions, supprimer des faux positifs |
| **Data analyst / développeur** | Explorer et valider les données GPX | Paramétrer finement, comprendre les effets, exporter pour analyses |
| **Intégrateur / visualiseur** | Afficher la trace sur une carte tierce | Consommer un JSON avec progressions normalisées, catégories précises |

### 2.1 Personas détaillés

**Persona 1 — Marc, cycliste route amateur**
- 45 ans, utilise Openrunner pour préparer ses sorties
- Veut identifier les cols à plus de 5 %, connaître le D+ total, ajuster les bornes des montées
- Utilise principalement la liste de gauche + le profil + la carte pour la vue d'ensemble

**Persona 2 — Sophie, organisatrice de cyclosportive**
- Doit valider un parcours de 150 km avec 8 cols
- Veut vérifier que chaque col est bien détecté, ajuster les bornes, séparer les montées fusionnées
- Utilise intensivement l'édition (drag), la séparation, la suppression

**Persona 3 — Thomas, data analyst**
- Étudie des traces pour un projet de recherche
- Paramètre finement les filtres, exporte les JSON pour traitement Python
- Utilise le diagnostic pour valider la cohérence des paramètres

**Persona 4 — Léa, développeuse front-end**
- Intègre l'outil dans une SPA existante
- Consomme la Note technique pour le portage TS, le JSON exporté pour l'affichage carte
- N'utilise pas l'UI directe mais l'API et l'export

---

## 3. Périmètre fonctionnel

### 3.1 Chargement du fichier

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-01 | Chargement par glisser-déposer d'un fichier `.gpx` n'importe où dans la fenêtre | Must | v0.1 |
| F-02 | Chargement par clic sur la drop-zone toolbar | Must | v0.1 |
| F-03 | Support des fichiers `.txt` contenant du XML GPX | Must | v0.1 |
| F-04 | Gestion d'erreur : fichier non-XML ou sans altitude | Must | v0.1 |
| F-05 | Support multi-segments `<trkseg>` (concaténation des points) | Should | v0.1 |
| F-06 | Support des métadonnées `<name>` pour titre de trace | Nice | v0.1 |
| F-07 | Drop-zone persistante après chargement (mode compact) | Must | v2.3 |
| F-08 | Affichage du nombre de points bruts et resamplés sur la drop-zone | Must | v2.9 |
| F-09 | Overlay plein écran pendant le drag (feedback visuel) | Should | v3.8 |

**F-01 — Détails techniques** :
- Écoute `dragenter`, `dragover`, `dragleave`, `drop` sur `document`
- Compteur `dragCounter` pour gérer les entrées/sorties multiples
- Overlay activé si `dragCounter === 1`
- Le drop lit `e.dataTransfer.files[0]`

**F-04 — Gestion d'erreur** :
- `DOMParser` avec `"application/xml"`
- Vérification `xml.querySelector("parsererror")`
- Vérification qu'au moins un `<trkpt>` avec `<ele>` existe
- `alert()` avec message clair pour l'utilisateur

**F-07 — Drop-zone compacte** :
- Après chargement, `.toolbar-drop.loaded` est ajoutée
- Le texte affiche `nom_fichier.gpx · N pts bruts → M pts resamplés`
- Le clic ouvre toujours le sélecteur de fichier

---

### 3.2 Prétraitement du signal d'altitude

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-10 | Calcul de la distance cumulée (formule de Haversine) | Must | v0.1 |
| F-11 | Correction des plateaux GPS/MNT (`plateauCorrect`) | Must | v2.8 |
| F-12 | Rééchantillonnage par moyenne (fenêtre ≥ 75 m) | Must | v3.5 |
| F-13 | Filtre anti-pic Hampel — fenêtre en mètres (50–500, défaut 200) | Must | v3.6 |
| F-14 | Filtre médian paramétrable en mètres | Must | v2.8 |
| F-15 | Savitzky-Golay ordre 2 paramétrable en mètres | Must | v2.8 |
| F-16 | Lissage gaussien paramétrable en σ (mètres) | Should | v2.8 |
| F-17 | Lissage ancré forçant le passage par les extrema détectés | Must | v0.5 |
| F-18 | Double SG optionnel (lissage renforcé) | Nice | v2.8 |
| F-19 | Pente de catégorisation par fenêtre glissante | Must | v0.10 |
| F-20 | Écrêtage des pentes affichées à ±25 % | Must | v0.4 |
| F-21 | Stats brutes (D+, D−, altitudes min/max) sur données GPX d'origine, doublons ignorés | Must | v0.4 |
| F-22 | Conservation de la longueur réelle (`totalDistRawM`) distincte de la longueur de la grille (`totalDistSampledM`) | Must | v3.7 |
| F-23 | Mesure de la queue de trace ignorée (`trailingGapM`) | Must | v3.7 |
| **F-24** | **Catégorisation sur altitude NON ancrée (évite les changements de couleur pendant le drag)** | **Must** | **v3.8.1** |

**F-10 — Haversine** :
- Utilise `atan2` pour la stabilité numérique
- Approximation sphérique (±0.5 % vs WGS84) suffisante pour traces < 500 km
- `dist[0] = 0`, puis cumul

**F-11 — Correction plateaux** :
- Appliquée **avant** le resampling (données brutes)
- Détecte les séries de `plateauRun` points à altitude ±1 m
- Interpole linéairement entre les bornes de la série
- Désactivée si `plateauRun = 0`

**F-12 — Rééchantillonnage par moyenne** :
- Fenêtre `clamp(stepM, 75, 3 × stepM)`
- Chaque point de sortie = moyenne des points bruts dans `[d - halfW, d + halfW]`
- Fallback : interpolation linéaire si aucun point dans la fenêtre
- Retourne `avgWinM` pour le log debug

**F-13 — Anti-pic Hampel** :
- Conversion mètres → points : `winPts = max(3, round(windowM / stepM) | 1)`
- Filtre médian conditionnel : remplace uniquement si `|ele[i] - median| > thresholdM`
- Ne modifie que les points aberrants

**F-24 — Catégorisation non ancrée** :
- `eleDisplay` (ancrée) utilisée pour la courbe affichée
- `eleBaseCat` (non ancrée) utilisée pour la catégorisation
- Les deux partagent le même `fenetreAffichageM` mais avec des ancres différentes
- Coût : +1 SG par rebuild (~5 ms sur 10 000 pts)

---

### 3.3 Détection automatique des montées

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-30 | Détection par hystérésis sur la pente lissée | Must | v0.3 |
| F-31 | Sous-segment optimal pour validation intermédiaire | Must | v0.3 |
| F-32 | Extension des bornes aux extrema locaux par argmin/argmax | Must | v0.5 |
| F-33 | Borne basse naturelle : ne pas empiéter sur la montée précédente | Must | v0.5 |
| F-34 | Garde-fou hybride de refinage `clamp(pct × longueur, 1500 m, 8000 m)` | Must | v2.0 |
| F-35 | Fusion des montées séparées de moins de `gapMinM` | Must | v3.0 |
| F-36 | Validation finale stricte : re-vérification des critères sur bornes finales | Must | v3.1 |
| F-37 | Repli automatique sur le meilleur sous-segment valide si bornes finales échouent | Must | v3.1 |
| F-38 | Transparence : affichage de la valeur effective du garde-fou par montée | Must | v2.0 |
| F-39 | Indicateur visuel `⊕` sur les montées issues d'une fusion | Must | v3.0 |
| F-40 | Indicateur visuel `⤺` sur les montées issues d'un repli | Must | v3.1 |
| **F-41** | **Conservation des `mergedParts` lors de la fusion (permet la séparation ultérieure)** | **Must** | **v3.9.0** |

**F-30 — Hystérésis** :
- Entrée en mode "climb" quand `slope > +seuil`
- Sortie quand `slope < -seuil`
- Évite les oscillations autour de 0 %

**F-33 — Borne basse naturelle** :
- `loIdx = max(lastEnd + 1, safetyLow)`
- Empêche qu'une montée inclue le sommet de la précédente

**F-34 — Garde-fou hybride** :
- `effectiveRefine = clamp(refinePct × candidateLen / 100, 1500, 8000)`
- Adaptatif (proportionnel) et borné
- Valeur affichée par montée dans la colonne Extrema

**F-35 — Fusion avec `mergedParts`** :
- Si `gapM < gapMinM` : la montée précédente est **étendue**
- Les bornes de la sous-montée initiale sont conservées dans `_mergedParts`
- Les bornes de la nouvelle sous-montée sont ajoutées à `_mergedParts`
- `mergedCount` incrémenté à chaque fusion

**F-36/F-37 — Validation stricte** :
- Calcul sur les **bornes finales** (post-raffinement et fusion)
- Si échec : `findBestSubClimb` sur `eleDisplay`, repli si sous-segment valide
- Si aucun sous-segment : rejet (rare)

**F-41 — `mergedParts`** :
- Structure : `[{ startKm, endKm }, ...]`
- Conservé à travers la validation stricte
- Transmis à `userClimbs` pour permettre la séparation

---

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

Ces critères sont **affichés** à l'utilisateur dans un panneau dédié du volet de droite mais **non modifiables** dans la version 3.x. Une évolution V4 est prévue pour les rendre éditables (F-V4-03).

### 3.5 Édition manuelle des montées

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-50 | Champs éditables (km vallée, km sommet) | Must | v0.7 |
| F-51 | Recalcul en direct des statistiques | Must | v0.7 |
| F-52 | Bouton « Restaurer la valeur auto » par ligne (`↺`) | Must | v0.7 |
| F-53 | Bouton « Supprimer » par ligne (`✕`) | Must | v0.7 |
| F-54 | Insertion contextuelle entre deux montées / avant #1 / après #N | Must | v2.2 |
| F-55 | Bouton « Recharger depuis la détection auto » | Must | v0.7 |
| F-56 | Indicateur visuel « modifié » sur les lignes éditées | Should | v0.7 |
| F-57 | Synchronisation intelligente : les paramètres modifiés n'écrasent pas les éditions | Must | v0.8 |
| F-58 | Anti-chevauchement : contrainte automatique sur les bornes | Must | v0.8 |
| F-59 | Signal visuel `⚠` + fond orange sur les lignes contraintes | Should | v0.8 |
| F-60 | Édition par drag & drop direct sur le profil | Must | v3.8 |
| F-61 | Sélection par clic sur la carte montée | Must | v3.8 |
| F-62 | Désélection (clic ✕ ou re-clic sur la carte) | Must | v3.8 |
| F-63 | Contraintes anti-chevauchement temps réel pendant le drag | Must | v3.8 |
| F-64 | Retour visuel temps réel (métriques + canvas + carte) | Must | v3.8 |
| F-65 | Édition clavier (flèches + Shift + Alt) | Should | v3.7 |
| F-66 | Robustesse drag : garde-fous canvas null | Must | v3.7.1 |
| F-67 | Nettoyage des listeners orphelins | Must | v3.7.1 |
| F-68 | `renderAll` inerte pendant drag | Must | v3.7.1 |
| **F-69** | **Suppression avec renumérotation `subIndex` des groupes** | **Must** | **v3.9.2** |

**F-60 — Drag sur le profil** :
- Poignées draggables en haut du canvas (bande des 30 premiers px)
- Détection de poignée à ±14 px de la position X
- Mise à jour de `climb.startKm` ou `climb.endKm` en temps réel
- Rebuild local (canvas + liste) sans `renderAll` complet

**F-63 — Contraintes temps réel** :
- `applyDragConstraint(handle, km, climb)` :
  - Vallée : `clamp(km, lo, climb.endKm - 0.05)`
  - Sommet : `clamp(km, climb.startKm + 0.05, hi)`
- Bornes calculées sur les voisins immédiats

**F-69 — Suppression avec renumérotation** :
- Retirer la montée
- Renuméroter les `subIndex` du groupe affecté
- Dissoudre le groupe si < 2 membres
- Ajuster `selectedClimbIdx`

---

### 3.6 Segmentation par catégorie de pente

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-70 | Classification en 8 catégories | Must | v0.9 |
| F-71 | Fusion des segments < `longueurMin` dans un voisin | Must | v0.3 |
| F-72 | Segments contigus : partage du point frontière, aucune zone blanche | Must | v0.10 |
| F-73 | Bandes colorées sur la carte et le profil | Must | v3.8 |
| F-74 | Zoom sur segment par clic sur ribbon | Should | Obsolète v3.8 |
| F-75 | Tableau statistiques par catégorie | Should | Obsolète v3.8 |
| F-76 | Catégorisation par pente moyenne sur fenêtre glissante | Must | v0.10 |
| F-77 | Alignement strict des tranches de `slopeProfile` sur les segments globaux | Must | v3.7 |
| **F-78** | **Coloration du profil via `catSegments` (identité stricte avec la carte)** | **Must** | **v3.8.1** |

#### 3.6.1 Grille des catégories

| Couleur | Libellé | Plage de pente |
|---|---|---|
| `#1e40af` Bleu foncé | Descente raide | p < −7 % |
| `#3b82f6` Bleu | Descente | −7 % ≤ p < −3 % |
| `#93c5fd` Bleu clair | Faux plat descendant | −3 % ≤ p < −1 % |
| `#22c55e` Vert | Plat | −1 % ≤ p < +1 % |
| `#fde68a` Jaune | Faux plat montant | +1 % ≤ p < +3 % |
| `#f97316` Orange | Montée | +3 % ≤ p < +7 % |
| `#dc2626` Rouge | Montée raide | +7 % ≤ p < +10 % |
| `#000000` Noir | Montée très raide | p ≥ +10 % |

*Les bornes hautes sont exclusives dans le code.*

**Changement v3.7** : seuil descente raide passé de −8 % à −7 % pour symétrie avec +7 %.

**F-72 — Point frontière partagé** :
```
POUR i DE 1 À n:
  SI i = n OU cat[i] ≠ cat[start]:
    endIdx ← (i = n) ? i − 1 : i    // ← point frontière partagé
    raw.push({startIdx: start, endIdx, key: cat[start]})
    start ← i
```

Conséquence : la somme des longueurs des segments après fusion **égale exactement la distance totale**.

**F-78 — Coloration via `catSegments`** :
- La carte et le profil utilisent tous deux `data.catSegments`
- Aucune divergence possible
- Corrige le bug v3.8 où le profil coloriait point-par-point (catégorie brute, sans fusion)

---

### 3.7 Visualisation — Carte Mapbox 3D

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-80** | **Carte Mapbox GL JS v3.7+** | **Must** | **v3.8** |
| **F-81** | **Terrain 3D (`mapbox-dem`, exagération 1×)** | **Must** | **v3.8** |
| **F-82** | **Trace colorée par catégorie de pente** | **Must** | **v3.8** |
| **F-83** | **Trace = points bruts du GPX (pas la grille resamplée)** | **Must** | **v3.9.2** |
| **F-84** | **Marqueurs vallée/sommet de la montée sélectionnée** | **Must** | **v3.8** |
| **F-85** | **Multi-marqueurs pour montées groupées** | **Must** | **v3.9.2** |
| **F-86** | **Zoom sur montée sélectionnée (marge 20 %)** | **Must** | **v3.8** |
| **F-87** | **Zoom sur groupe entier si montée groupée** | **Must** | **v3.9.2** |
| **F-88** | **Bascule satellite / standard** | **Must** | **v3.8** |
| **F-89** | **Bouton recentrer sur la trace** | **Must** | **v3.8** |
| **F-90** | **Courbes de niveau optionnelles (mapbox-terrain-v2)** | **Should** | **v3.8.4** |
| **F-91** | **Champ token Mapbox avec persistance localStorage** | **Must** | **v3.8** |
| **F-92** | **Chargement automatique de la carte si token présent** | **Must** | **v3.9.2** |
| **F-93** | **Mise à jour des marqueurs en temps réel pendant le drag** | **Must** | **v3.8** |
| **F-94** | **Terrain 3D avec pitch 60°** | **Must** | **v3.8** |
| **F-95** | **Ordre des couches : trace avant contours (corrige contours invisibles)** | **Must** | **v3.8.5** |

**F-80 — Chargement paresseux** :
- `loadMapboxGL()` injecte le CSS + JS depuis le CDN
- Flag `MAP_STATE.loading` pour éviter les doubles chargements
- Attente active si un chargement est déjà en cours

**F-81 — Terrain 3D** :
- Source `mapbox.mapbox-terrain-dem-v1` (SRTM 30 m)
- `map.setTerrain({ source, exaggeration: 1 })`
- Exagération 1× (naturelle)

**F-82 — Trace colorée** :
- Une `LineString` par segment de catégorie
- `line-color: ['get', 'color']` depuis les propriétés GeoJSON
- `line-width: 6`, `line-cap: round`, `line-join: round`

**F-83 — Trace = points bruts** :
- Itération sur `STATE.rawPoints` (fichier GPX)
- Catégorisation par distance cumulée (`distRaw[i]`)
- Parcours monotone de `catSegments`
- Le point frontière est inclus dans les deux segments adjacents

**F-85 — Multi-marqueurs** :
- Si la montée sélectionnée est groupée :
  - Boucler sur tous les membres du groupe
  - Ajouter 2 marqueurs (vallée + sommet) par membre
  - `role: 'selected'` pour le membre actif (rayon 10, contour 4)
  - `role: 'member'` pour les autres (rayon 8, contour 2.5)
- Sinon : 2 marqueurs `role: 'selected'`

**F-90 — Courbes de niveau** :
- Source vectorielle `mapbox.mapbox-terrain-v2`, layer `contour`
- Deux couches :
  - `contour-lines` (fines, `#7a5733`)
  - `contour-index` (index ≥ 5, plus épaisses, `#5e3f1d`)
- Insérées **sous la trace** (via `beforeId: 'trace-line-layer'`)
- Toggle avec persistance `localStorage.showContours`

**F-92 — Auto-load** :
- Au démarrage, si `localStorage.mapboxToken` existe → `initMapboxMap(token)`
- Statut affiché : « Carte OK (auto) »

**F-95 — Ordre des couches (critique)** :
```javascript
// 1. Terrain (raster-dem + setTerrain)
// 2. Brume (setFog)
// 3. Trace (trace-line-layer) ← DOIT exister avant les contours
// 4. Marqueurs (climb-markers-circle)
// 5. Contours (contour-lines + contour-index, beforeId='trace-line-layer')
```

**Ne pas inverser** : les contours utilisent `'trace-line-layer'` comme `beforeId`. Si la trace n'existe pas encore, `addLayer` échoue silencieusement et les contours n'apparaissent jamais (bug corrigé en v3.8.5).

---

### 3.8 Visualisation — Widget profil altimétrique

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-100** | **Canvas 2D natif (remplace Chart.js)** | **Must** | **v3.8** |
| **F-101** | **Échelle X fixe : 50 m/px en mode full** | **Must** | **v3.8.3** |
| **F-102** | **Échelle X fixe : 10 m/px en mode montée** | **Must** | **v3.9.2** |
| **F-103** | **Échelle Y fixe : 10 m/px** | **Must** | **v3.8** |
| **F-104** | **Coloration par `catSegments`** | **Must** | **v3.8.1** |
| **F-105** | **Lignes verticales vallée/sommet** | **Must** | **v3.8** |
| **F-106** | **Couleur de groupe pour les membres** | **Must** | **v3.9.1** |
| **F-107** | **Scroll horizontal** | **Must** | **v3.8** |
| **F-108** | **Centrage automatique sur la montée sélectionnée** | **Must** | **v3.9.2** |
| **F-109** | **Poignées draggables (mode montée)** | **Must** | **v3.8** |
| **F-110** | **Surbrillance jaune de la montée sélectionnée** | **Must** | **v3.8** |
| **F-111** | **Bouton ✕ pour retour au mode full** | **Must** | **v3.8** |
| **F-112** | **Création manuelle par double-clic + drag** | **Must** | **v3.9.0** |
| **F-113** | **Indicateurs visuels pendant la création** | **Must** | **v3.9.0** |
| **F-114** | **Annulation création par Échap** | **Must** | **v3.9.0** |
| **F-115** | **Refus de création si chevauchement** | **Must** | **v3.9.0** |

**F-100 — Canvas 2D** :
- Remplace Chart.js utilisé jusqu'à v3.7.2
- Contrôle total du rendu (échelles fixes, scroll, drag)
- Perf supérieure sur grandes traces (> 10 000 pts)

**F-101/F-102 — Échelles fixes** :
- `PROFILE_SCALE_X_FULL = 50` m/px
- `PROFILE_SCALE_X_CLIMB = 10` m/px
- La largeur du canvas est **strictement** `(viewEndM − viewStartM) / scaleX`
- **Ne jamais agrandir** à la largeur du conteneur (bug v3.8.3)
- Sur une trace courte, le canvas peut être plus étroit que le conteneur (vide à droite)

**F-108 — Centrage automatique** :
```javascript
if (profileMode === 'climb' && !dragging && selectedClimbIdx != null) {
  const centerM = (c.startDist + c.endDist) / 2;
  const centerPx = (centerM - viewStartM) / scaleXActual;
  const targetScroll = Math.max(0, centerPx - scroll.clientWidth / 2);
  requestAnimationFrame(() => { scroll.scrollLeft = targetScroll; });
}
```
**Ne pas re-scroller** pendant un drag (état `__profileDragging`).

**F-112 — Création manuelle** :
- Double-clic sur le canvas (mode full uniquement) → pose la vallée
- Clic-glisser → définit le sommet
- Relâchement → validation + ajout si valide
- Échap → annulation

**F-115 — Refus de chevauchement** :
- Double-clic dans une montée existante → refus (flash rouge)
- Drag qui produit un chevauchement → refus au relâchement
- Longueur minimale 50 m

---

### 3.9 Groupes de montées

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-120** | **Séparation d'une montée fusionnée (✂)** | **Must** | **v3.9.0** |
| **F-121** | **Champ `groupId` persistant** | **Must** | **v3.9.1** |
| **F-122** | **Champ `subIndex` pour numérotation `#N.M`** | **Must** | **v3.9.1** |
| **F-123** | **Bandeau de groupe coloré** | **Must** | **v3.9.1** |
| **F-124** | **Bordure gauche colorée par groupe** | **Should** | **v3.9.1** |
| **F-125** | **Badge `🔗 N` dans l'en-tête** | **Must** | **v3.9.1** |
| **F-126** | **Refusion d'un groupe (🔗)** | **Must** | **v3.9.1** |
| **F-127** | **Reconstruction de `mergedParts` à la refusion** | **Must** | **v3.9.1** |
| **F-128** | **Couleur stable par `groupId` (hsl angle d'or)** | **Should** | **v3.9.1** |
| **F-129** | **Dissolution automatique des groupes à 1 membre** | **Must** | **v3.9.1** |

**F-120 — Séparation** :
- Alloue un `groupId` depuis `STATE.nextGroupId++`
- Retire la montée fusionnée
- Crée une montée pour chaque sous-montée, avec `subIndex` (1, 2, 3…)
- Tri par `startKm`
- Sélection de la première sous-montée
- Zoom carte sur tout le groupe

**F-126 — Refusion** :
- Récupère tous les membres du groupe
- Fusionne leurs bornes **actuelles** (après éventuelles éditions)
- Reconstruit `mergedParts`
- Crée une nouvelle montée fusionnée
- Dissout le groupe

**F-128 — Couleur stable** :
```javascript
function groupColor(gid) {
  const hue = (gid * 137.508) % 360;   // Angle d'or
  return `hsl(${hue.toFixed(0)}, 62%, 46%)`;
}
```
Distribution optimale des teintes (angle d'or évite les clusters).

**F-129 — Dissolution** :
- Appelée par `normalizeGroups()` à la fin des drags et après `recomputeAuto`
- Si un groupe a < 2 membres, retire `groupId` et `subIndex`

---

### 3.10 Création manuelle d'une montée

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-130** | **Double-clic sur profil pour poser la vallée** | **Must** | **v3.9.0** |
| **F-131** | **Clic-glisser pour définir le sommet** | **Must** | **v3.9.0** |
| **F-132** | **Refus si vallée dans montée existante** | **Must** | **v3.9.0** |
| **F-133** | **Refus si plage chevauche une montée existante** | **Must** | **v3.9.0** |
| **F-134** | **Longueur minimale 50 m** | **Must** | **v3.9.0** |
| **F-135** | **Insertion triée dans la liste** | **Must** | **v3.9.0** |
| **F-136** | **Sélection automatique après création** | **Must** | **v3.9.0** |
| **F-137** | **Zoom carte sur la nouvelle montée** | **Should** | **v3.9.0** |
| **F-138** | **Flash rouge si position invalide** | **Should** | **v3.9.0** |
| **F-139** | **Hint textuel contextuel** | **Should** | **v3.9.0** |

**F-130 — Double-clic** :
- Mode full uniquement
- Position libre (hors montées existantes)
- Pose `pendingNewClimb = { valleyKm: km, summitKm: km }`
- Hint passe à « 🎯 Cliquez-glissez pour définir le sommet »

**F-131 — Drag** :
- `mousedown` + `mousemove` → `newClimbDragging = true`
- `summitKm` mis à jour en temps réel
- `summitKm = max(valleyKm + 0.05, km)` (clamp minimal)

**F-133 — Refus de chevauchement** :
```javascript
function isFreeRange(aKm, bKm) {
  const lo = Math.min(aKm, bKm);
  const hi = Math.max(aKm, bKm);
  for (const c of STATE.userClimbs) {
    if (lo < c.endKm && hi > c.startKm) return false;
  }
  return true;
}
```

**F-138 — Flash rouge** :
- Bref flash rouge sur le canvas à la position invalide
- `setTimeout(() => renderProfile(), 200)`

---

### 3.11 Suppression d'une montée

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-140** | **Bouton ✕ sur chaque carte de montée** | **Must** | **v3.9.2** |
| **F-141** | **Renumérotation des `subIndex` du groupe affecté** | **Must** | **v3.9.2** |
| **F-142** | **Dissolution du groupe si < 2 membres** | **Must** | **v3.9.2** |
| **F-143** | **Ajustement de `selectedClimbIdx`** | **Must** | **v3.9.2** |

**F-140 — Bouton ✕** :
- Positionné en `position: absolute; top: 8px; right: 8px`
- Toujours visible (sans hover)
- Style rouge discret

**F-143 — Ajustement sélection** :
- Si la montée supprimée était sélectionnée → désélection + retour au mode full
- Si l'index sélectionné > index supprimé → décrémente
- Sinon inchangé

---

### 3.12 Statistiques globales

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-150 | Distance totale (basée sur `totalDistRawM`) | Must | v0.1 |
| F-151 | D+ brut (sur altitudes GPX d'origine, doublons ignorés) | Must | v0.4 |
| F-152 | D− brut | Must | v0.4 |
| F-153 | Altitude min / max | Must | v0.1 |
| F-154 | Nombre de montées détectées (avec compteurs `⊕` et `⤺`) | Must | v0.1 |
| F-155 | D+ cumulé des montées validées | Must | v0.4 |
| F-156 | Distance cumulée des montées validées | Must | v0.4 |
| F-157 | Nombre de segments de catégorie | Should | v0.3 |
| F-158 | Nombre de points bruts → resamplés | Must | v2.9 |

**F-150 — Distance totale** :
- Basée sur `totalDistRawM` (longueur réelle)
- Différente de `totalDistSampledM` (longueur de la grille)

**F-151/F-152 — D+/D− bruts** :
- Calculés sur les altitudes GPX d'origine
- Les doublons GPS (< 1 m) sont ignorés pour ne pas gonfler artificiellement le D+
- Peut différer légèrement de Strava/Garmin (méthodes différentes)

**F-158 — Points bruts → resamplés** :
- Affiché dans une carte statistique
- Format : `3312 (→ 2190)`

---

### 3.13 Export JSON

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| F-160 | Export JSON par bouton | Must | v3.7 |
| F-161 | Liste des segments (start/end km + progressions + pente + D+) | Must | v3.7 |
| F-162 | Liste des montées (bornes, progressions, métriques) | Must | v3.7 |
| F-163 | `slopeProfile` par montée, aligné sur les segments | Must | v3.7 |
| F-164 | Granularité `exportProfileStepM` (10–500 m) | Must | v3.7 |
| F-165 | `metadata` : `totalDistanceKm` + `totalDistanceSampledKm` | Must | v3.7 |
| F-166 | `trailingGapM` exposé | Must | v3.7 |
| F-167 | Section `legend` avec catégories | Must | v3.7 |
| F-168 | Format plat (pas de FeatureCollection) | Must | v3.7.2 |
| **F-169** | **`groupId` et `subIndex` dans chaque `climb`** | **Must** | **v3.9.1** |

#### 3.13.1 Structure du JSON exporté

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
    "categories": [
      { "id": "dh3", "label": "Descente raide", "minPct": null, "maxPct": -7 },
      { "id": "dh2", "label": "Descente", "minPct": -7, "maxPct": -3 },
      ...
    ]
  },
  "segments": [
    {
      "index": 0,
      "startKm": 0,
      "endKm": 0.3,
      "startProgress": 0,
      "endProgress": 0.06982,
      "categoryId": "up1",
      "avgSlopePct": 2.15,
      "dPlusM": 5
    }
  ],
  "climbs": [
    {
      "index": 0,
      "groupId": null,
      "subIndex": null,
      "startKm": 0,
      "endKm": 3.95,
      "startProgress": 0,
      "endProgress": 0.91933,
      "dPlusM": 284,
      "avgSlopePct": 7.67,
      "altStartM": 503.1,
      "altEndM": 786.9,
      "mergedCount": 0,
      "fallback": false,
      "slopeProfile": [
        {
          "kmStart": 0.25,
          "kmEnd": 0.3,
          "eleStartM": 503.1,
          "eleEndM": 506.8,
          "avgSlopePct": 7.42,
          "categoryId": "up2"
        }
      ],
      "slopeProfileStepM": 50
    }
  ]
}
```

#### 3.13.2 Contrat d'utilisation des progressions

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

#### 3.13.3 Contrat d'alignement du `slopeProfile`

Chaque tranche de `slopeProfile` appartient **intégralement** à un seul segment global. Sa `categoryId` est donc **strictement identique** à celle affichée par la carte.

Deux modes de découpage sont possibles selon `exportProfileStepM` :

- Si `exportProfileStepM` divise la longueur du segment : tranches régulières
- Sinon : les tranches sont réparties uniformément sur la longueur du segment (ex. segment de 300 m → `[150][150]` avec `profileStepM=200`)

Le choix de longueurs uniformes par segment (plutôt que tranches pleines + résidu) est délibéré : il évite les tranches ridicules en fin de segment et rend le contrat simple à comprendre pour le consommateur du JSON.

---

### 3.14 Interface — Layout 3 colonnes

| ID | Fonctionnalité | Priorité | Version |
|---|---|---|---|
| **F-170** | **Toolbar supérieure (56 px)** | **Must** | **v3.8** |
| **F-171** | **Colonne gauche — Montées (320 px)** | **Must** | **v3.8** |
| **F-172** | **Colonne centrale — Carte + Profil** | **Must** | **v3.8** |
| **F-173** | **Colonne droite — Paramètres (340 px, repliable)** | **Must** | **v3.8** |
| **F-174** | **Bouton ⚙️ pour replier/déplier le volet params** | **Must** | **v3.8** |
| **F-175** | **Drag overlay plein écran pendant le drop** | **Must** | **v3.8** |
| **F-176** | **Stats mini dans la colonne gauche** | **Must** | **v3.8** |
| **F-177** | **Drop-zone persistante dans la toolbar** | **Must** | **v3.8** |
| **F-178** | **Bouton 🐛 debug dans la toolbar** | **Should** | **v3.8** |
| **F-179** | **Responsive < 900 px (colonnes empilées)** | **Should** | **v3.8** |

**F-170 — Toolbar** :
- Marque + version à gauche
- Drop-zone persistante au centre
- Boutons 🐛 et ⚙️ à droite
- Hauteur fixe 56 px
- Ombre légère

**F-173 — Volet params** :
- Largeur 340 px
- Repliable (width → 0 avec transition)
- Header avec pastille diagnostic
- Sections : Mapbox, Préparation, Lissage, Détection, Export & Debug, Options

**F-179 — Responsive** :
- En dessous de 900 px, les colonnes s'empilent verticalement
- Utilise `flex-wrap` ou `grid`

---

## 4. Paramètres utilisateur

### 4.1 Paramètres numériques (15)

| Nom UI | Nom technique | Plage | Défaut | Unité | Impact |
|---|---|---|---|---|---|
| Pas de rééchantillonnage | `stepM` | 2–200 | **50** | m | Espacement cible après resampling |
| Anti-pic : fenêtre | `despikeWindowM` | 50–500 | **200** | m | Fenêtre Hampel |
| Anti-pic : seuil | `despikeThresholdM` | 0–100 | **15** | m | Seuil détection glitch |
| Lissage altitude | `fenetreAffichageM` | 20–∞ | **500** | m | Douceur du profil affiché + catégorisation |
| Lissage pente | `fenetrePenteM` | 10–∞ | **150** | m | Douceur de la courbe de pente |
| Fenêtre de calcul pente | `fenetreCatM` | 50–∞ | **100** | m | Largeur fenêtre glissante couleur |
| Seuil détection | `seuilMontee` | 0.1–∞ | **0.5** | % | Sensibilité de la détection |
| Longueur min. pente | `longueurMin` | 0–∞ | **200** | m | Fusion micro-segments |
| Écart min. entre montées | `gapMinM` | 0–5000 | **250** | m | Fusionne les montées plus proches |
| Recherche extrema | `refinePct` | 1–500 | **60** | % | Distance max de recherche extrema |
| Filtre médian | `medianWindowM` | 0–300 | **0** | m | Élimine les pics isolés |
| Lissage gaussien | `gaussianSigmaM` | 0–500 | **75** | m | Adoucissement post-ancrage |
| Correction plateaux | `plateauRun` | 0–20 | **6** | pts | Détection des marches MNT/GPS |
| Segments debug | `segmentLogM` | 10–2000 | **100** | m | Granularité log debug |
| Pas du profil exporté | `exportProfileStepM` | 10–500 | **50** | m | Granularité du `slopeProfile` exporté |

### 4.2 Toggles (2)

| Nom UI | Nom technique | Défaut | Impact |
|---|---|---|---|
| Lissage renforcé (double SG) | `smoothExtra` | **ON** | Second passage SG après ancrage |
| Lissage des pentes de segment | `segmentSmooth` | **ON** | Médiane mobile à 3 sur les pentes du log debug |

### 4.3 Constantes internes (non exposées)

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
| `DESPIKE_WINDOW_M_DEFAULT` | 200 | Fenêtre anti-pic par défaut (m) |
| `DESPIKE_WINDOW_M_MIN` | 50 | Plancher fenêtre anti-pic (m) |
| `DESPIKE_WINDOW_M_MAX` | 500 | Plafond fenêtre anti-pic (m) |
| `DESPIKE_THRESHOLD_M_DEFAULT` | 15 | Seuil anti-pic |
| `RESAMPLE_AVG_MIN_M` | 75 | Fenêtre minimale de moyennage |
| `INSERT_EDGE_MIN_KM` | 1.0 | Espace avant/après insertion |
| `INSERT_BETWEEN_MIN_KM` | 0.2 | Espace entre montées insertion |
| `INSERT_DEFAULT_LEN_KM` | 1.0 | Longueur cible insertion |
| `MANUAL_MIN_LEN_KM` | 0.05 | Longueur min. création manuelle |
| `EXPORT_PROFILE_STEP_M_DEFAULT` | 50 | Granularité par défaut |
| `EXPORT_PROFILE_STEP_M_MIN` | 10 | Plancher |
| `EXPORT_PROFILE_STEP_M_MAX` | 500 | Plafond |
| **`PROFILE_SCALE_X_FULL`** | **50** | **m/px profil complet** |
| **`PROFILE_SCALE_X_CLIMB`** | **10** | **m/px widget montée** |
| **`PROFILE_SCALE_Y`** | **10** | **m/px vertical** |
| `PROFILE_MIN_HEIGHT` | 110 | Hauteur mini canvas |
| `PROFILE_HANDLE_PAD` | 200 | Marge en m autour d'une montée |
| **`MAP_TERRAIN_EXAGGERATION`** | **1** | **Exagération terrain 3D** |
| **`MAP_CLIMB_MARGIN`** | **0.20** | **Marge zoom montée** |
| **`MAP_MIN_SPAN_DEG`** | **0.0014** | **Emprise minimale (~150 m)** |
| **`CONTOUR_BASE_COLOR`** | **`#7a5733`** | **Couleur contours normaux** |
| **`CONTOUR_INDEX_COLOR`** | **`#5e3f1d`** | **Couleur contours d'index** |
| `DRAG_MIN_LEN_KM` | 0.05 | Longueur min drag |
| `DRAG_MIN_GAP_KM` | 0.001 | Gap min drag |
| `_maxSearchSteps` | 800 | Sous-échantillonnage findBestSubClimb |
| `_minSegmentLength` | 300 | Longueur min absolue |

### 4.4 Guide de réglage anti-pic

| Type de trace | Fenêtre | Seuil | Justification |
|---|---|---|---|
| Trace GPS capteur | 200 m | 20–30 m | Bruit GPS peut atteindre 30 m |
| Trace éditeur SRTM | 200 m | 15 m | Glitch SRTM = 10–30 m |
| Trace très propre | 100 m | 10 m | Détection fine |
| Vraies bosses larges | 200 m | 25 m | Préserver les reliefs de +25 m |
| Séries de glitchs | 300–500 m | 15–20 m | Corriger des groupes de glitchs |
| Désactiver complètement | — | 0 | Comportement avant anti-pic |

### 4.5 Guide de cohérence multi-paramètres

| Règle | Formule | Conséquence si violée |
|---|---|---|
| Fenêtre SG ≥ 10 pts | `fenetreAffichageM ≥ 10 × stepM` | Lissage altitude inefficace |
| Ratio altitude/pente ≥ 2 | `fenetreAffichageM ≥ 2 × fenetrePenteM` | Pente plus lisse que l'altitude |
| Filtre médian ≥ 3 pts | `medianWindowM ≥ 3 × stepM` (ou 0) | Filtre médian inefficace |
| Gaussien σ ≥ 0.5 pt | `gaussianSigmaM ≥ 0.5 × stepM` (ou 0) | Filtre gaussien inefficace |
| Longueur min. pente ≥ Pas | `longueurMin ≥ stepM` (ou 0) | Paramètre inopérant |
| Écart min. ≥ Pas | `gapMinM ≥ stepM` (ou 0) | Paramètre inopérant |
| Anti-pic fenêtre ≥ 2 × Pas | `despikeWindowM ≥ 2 × stepM` (ou seuil = 0) | Arrondi force une fenêtre > demandée |
| Anti-pic seuil ≥ 5 m | `despikeThresholdM ≥ 5` (ou 0) | Risque d'écrêter des bosses |
| Pas du profil exporté ≤ Longueur min. pente | `exportProfileStepM ≤ longueurMin` | Tranches raccourcies |

---

## 5. Parcours utilisateur

### 5.1 Parcours nominal

1. L'utilisateur ouvre la page → toolbar + 3 colonnes vides
2. Il saisit son token Mapbox → **la carte se charge automatiquement** (v3.9.2)
3. Il glisse son fichier GPX n'importe où → analyse automatique lancée
4. Le pipeline effectue :
   - Parse GPX → distances cumulées + altitudes brutes + capture de `totalDistRawM`
   - Stats brutes (D+, D−, altitudes min/max, distribution des pas)
   - Correction plateaux (optionnelle)
   - Rééchantillonnage par moyenne à pas fixe (`stepM`)
   - Filtre anti-pic (fenêtre en mètres)
   - Pipeline de détection sur données nettoyées
   - **Fusion avec conservation des `mergedParts`**
   - **Validation stricte avec repli**
5. Affichage :
   - Stats compactes (colonne gauche)
   - Liste des montées avec badges (`⊕`, `⤺`, `🔗 N`)
   - Carte 3D avec trace colorée (centre haut)
   - Profil altimétrique complet à 50 m/px (centre bas)
   - Diagnostic + paramètres (colonne droite)
6. L'utilisateur ajuste les paramètres → recalcul en direct
7. Il clique sur une montée → **widget montée à 10 m/px, centré sur la montée**, carte zoomée, marqueurs posés
8. Il glisse les poignées → métriques temps réel, marqueurs carte temps réel
9. Il peut **séparer** une fusion (`✂ Séparer`)
10. Il peut **refusionner** un groupe (`🔗 Refusionner`)
11. Il peut **supprimer** une montée (`✕`)
12. Il peut **créer** une montée (double-clic + drag sur profil full)
13. Il peut **désélectionner** (clic ✕ ou re-clic sur la carte)
14. Il exporte le JSON

### 5.2 Parcours d'erreur

| Cas | Comportement attendu |
|---|---|
| Fichier non-XML | Alerte, pas de chargement |
| GPX sans balise `<ele>` | Alerte, pas de chargement |
| Fichier très lourd (> 50 000 points) | Resampling automatique, latence acceptable |
| Édition créant un chevauchement | Contrainte automatique + `⚠` |
| Édition créant un segment invalide | Ligne `invalid`, métriques `—` |
| Drag hors zone cliquable | Fallthrough vers le pan carte |
| Drag avec canvas détruit en arrière-plan | Abandon silencieux, pas de crash |
| Montée sous le seuil après fusion | Repli sur sous-segment valide (`⤺`) |
| Montée sans sous-segment valide | Rejetée (log debug) |
| Anti-pic fenêtre < 2 × Pas | Warning diagnostic (🟠) |
| Anti-pic seuil < 5 m | Warning diagnostic (🟠) |
| `exportProfileStepM > longueurMin` | Warning diagnostic (🟠) |
| **Création manuelle chevauchante** | **Refus + flash rouge** |
| **Vallée création dans montée existante** | **Refus + flash rouge** |
| **Token Mapbox invalide** | **Message d'erreur dans le volet** |
| **Token Mapbox absent** | **Placeholder, pas d'erreur bloquante** |
| **Clic ✂ sur montée non fusionnée** | **Bouton absent (impossible)** |
| **Clic 🔗 sur groupe à 1 membre** | **Bouton absent (normalizeGroups a dissous)** |

### 5.3 Parcours avancé — organiser un col enchaîné

1. Charger un GPX de col enchaîné (Mortirolo, etc.)
2. La détection produit **1 montée fusionnée** (`mergedCount > 0`)
3. L'utilisateur clique **✂ Séparer** → groupe `#N` créé avec `#N.1`, `#N.2`, `#N.3`
4. La carte zoome sur **tout le groupe** et affiche **tous les marqueurs** (v3.9.2)
5. L'utilisateur peut :
   - Éditer chaque sous-montée individuellement (drag)
   - Supprimer une sous-montée qui n'aurait pas dû être détectée (✕)
   - Refusionner le tout (🔗) — les bornes éditées sont conservées
6. Exporter le JSON avec les `groupId`/`subIndex`

### 5.4 Parcours avancé — ajouter une montée manuellement

1. Charger un GPX
2. La détection a raté une bosse (seuil trop haut)
3. L'utilisateur est en mode full (profil complet)
4. Il **double-clique** sur le profil à l'endroit de la vallée → ligne verte apparaît
5. Il **clique-glisse** vers le sommet → zone verte (ou rouge si chevauchement)
6. Il **relâche** → montée créée et sélectionnée
7. Le profil passe en mode montée, centré sur la nouvelle montée
8. La carte zoome sur la montée

---

## 6. Architecture technique

### 6.1 Stack

| Composant | Choix | Justification |
|---|---|---|
| Langage | HTML / CSS / JavaScript vanilla | Aucun build, portable |
| Visualisation profil | **Canvas 2D natif** | Contrôle total, perfs optimales |
| Cartographie | **Mapbox GL JS v3.7+** | 3D natif, terrain, contours |
| Carte fond | **Standard** / **Satellite Streets** | Toggle utilisateur |
| Terrain | `mapbox.mapbox-terrain-dem-v1` | DEM SRTM 30 m natif |
| Courbes de niveau | `mapbox.mapbox-terrain-v2` (layer `contour`) | Vector source Mapbox |
| Backend | Aucun | Confidentialité, simplicité |
| Export | Blob + `URL.createObjectURL` natif | Aucun ajout de dépendance |
| Stockage token | localStorage | Persistance |
| Stockage toggle contours | localStorage | Persistance |

### 6.2 Pipeline complet (v3.9.2)

```
┌─────────────────────────────────────────────────────────────┐
│ CHARGEMENT                                                  │
├─────────────────────────────────────────────────────────────┤
│ 1. Parse GPX → rawPoints[]                                  │
│ 2. distRaw[] = cumsum(haversine)                            │
│ 3. totalDistRawM = distRaw[last]                            │
│ 4. eleRawOrig[] = altitudes GPX d'origine                   │
│ 5. rawStats = computeRawStats(distRaw, eleRawOrig)          │
│ 6. [Optionnel] eleCorrected = plateauCorrect(eleRawOrig)    │
│ 7. resampleUniform(distRaw, eleCorrected, stepM)            │
│    → dist[], ele[] (pas fixe, moyennage fenêtre ≥ 75 m)     │
│    → totalDistSampledM = dist[last]                         │
│    → trailingGapM = totalDistRawM - totalDistSampledM       │
│ 8. Conversion fenêtre anti-pic m → pts :                    │
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
│    → conserve mergedParts à chaque fusion                   │
│ 9. Lissage ancré final (SG + gaussien + doubleSG optionnel) │
│ 10. VALIDATION STRICTE :                                    │
│     - pente moyenne sur bornes FINALES                      │
│     - si échec → repli sur findBestSubClimb (⤺)             │
│     - si aucun sous-segment → rejet                         │
│     - mergedParts conservés à travers la validation         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ REBUILD (à chaque édition)                                  │
├─────────────────────────────────────────────────────────────┤
│ 1. Contraintes anti-chevauchement                           │
│ 2. Ancrage sur les bornes utilisateur                       │
│ 3. Lissage ancré (eleDisplay)                               │
│ 4. Pentes (raw / display)                                   │
│ 5. SG non ancré (eleBaseCat) → slopeForCat                  │
│ 6. Segmentation par catégorie → catSegments                 │
│ 7. Marqueurs vallée/sommet                                  │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ RENDU                                                       │
├─────────────────────────────────────────────────────────────┤
│ 1. renderStats (mini-cards)                                 │
│ 2. renderClimbsList (liste gauche avec groupes)             │
│ 3. renderProfile (canvas 2D + scroll centré)                │
│ 4. Mapbox :                                                 │
│    - buildTraceFeatures (points bruts + catOf)              │
│    - updateMapMarkers (mono ou groupe)                      │
│    - [zoom auto sur montée/groupe]                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ EXPORT JSON (buildExportData)                               │
├─────────────────────────────────────────────────────────────┤
│ 1. Rebuild (fraîcheur des données)                          │
│ 2. Segments plats (startKm/endKm/progressions/…)            │
│ 3. Montées (bornes, métriques, slopeProfile, groupId)       │
│ 4. slopeProfile : découpage par segment global              │
│    (aucune tranche ne traverse une frontière)               │
│ 5. Legend + metadata                                        │
│ 6. Blob → download                                          │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Modèle de données en mémoire

```javascript
STATE = {
  // Données brutes (fichier GPX)
  rawPoints: [{lat, lon, ele}],
  distRaw: number[],
  eleRawOrig: number[],
  totalDistRawM: number,
  rawStats: {n, dPlus, dMoins, gapMin, gapMed, gapMax, gapMean, eleMin, eleMax, doublons},

  // Données resamplées + nettoyées
  dist: number[],
  eleRaw: number[],            // après resampling ET anti-pic
  stepM: number,
  fileName: string,

  // Montées
  autoClimbs: AutoClimb[],
  userClimbs: UserClimb[],

  // UI
  selectedClimbIdx: number | null,
  profileMode: 'full' | 'climb',
  pendingNewClimb: {valleyKm, summitKm} | null,
  newClimbDragging: boolean,

  // Groupes
  nextGroupId: number,

  // Cache de rendu
  eleDisplay: number[],
  __lastData: RebuildResult | null
}

MAP_STATE = {
  map: any,                    // mapboxgl.Map
  ready: boolean,
  loading: boolean,
  basemap: 'standard' | 'satellite',
  tokenErrorShown: boolean
}
```

### 6.4 Ordre du code (fichier source)

```
1.  Constantes (REFINE_*, STEP_*, DESPIKE_*, MAP_*, CONTOUR_*)
2.  Debug logger (dbg, dbgInfo, dbgWarn, dbgRaw, dbgResample,
                  dbgMerge, dbgFallback, dbgPlateau, dbgDespike,
                  dbgSplit, dbgGroup, dbgDelete)
3.  Utilitaires (haversine, movingAverage, savitzkyGolay,
                 gaussianFilter, medianFilter, computeSlope,
                 computeSlopeWindow, clipSlope, ...)
4.  resampleUniform (par moyenne)
5.  plateauCorrect
6.  despikeWindowMToPts + despikeAltitude
7.  computeRawStats
8.  anchoredMovingAverage
9.  Catégories + critères
10. Parseur GPX
11. mergeShortSegments, enrichSegment, segmentByCategory
12. detectEvents, buildClimbCandidates, findBestSubClimb
13. computeAuto (avec FUSION + mergedParts + VALIDATION STRICTE)
14. STATE + MAP_STATE + getParams
15. rebuild
16. renderAll, renderStats, renderClimbsList
17. Groupes (splitMergedClimb, refuseGroup, normalizeGroups)
18. Suppression (deleteClimb)
19. Création manuelle (addManualClimb, isFreeRange, isFreeKm)
20. renderProfile (canvas 2D + scroll centré)
21. Interactions (drag poignées, création manuelle)
22. Mapbox (loadMapboxGL, initMapboxMap, setupMapLayers,
            updateContourVisibility, buildTraceFeatures,
            updateMapTrace, updateMapMarkers, updateMapMarkersLive,
            refreshMapAll, zoomMapToClimb, zoomMapToGroup,
            fitMapToTrace, setMapBasemap)
23. loadFile
24. recomputeAuto
25. computeDiagnostics, updateDiagnostics
26. buildClimbSlopeProfile, buildExportData, exportJson
27. setupGlobalDragDrop
28. Interface (boutons, toggles)
29. initMapUI (avec auto-load token)
30. Resize handling
31. Annulation création par Échap
32. Enregistrement des plugins + init
```

---

## 7. Exigences non-fonctionnelles

| Exigence | Cible |
|---|---|
| Temps de calcul initial (10 000 pts, stepM=25) | < 3 secondes |
| Temps de recalcul après édition | < 500 ms |
| Latence pendant drag (par frame) | < 16 ms (60 fps) |
| Temps d'export JSON (trace 10 000 pts) | < 500 ms |
| Compatibilité navigateurs | Chrome, Firefox, Safari, Edge (2 dernières versions) |
| Support mobile | Responsive, lisible sur tablette (≥ 768 px) |
| Support tactile drag | Touch events gérés |
| Confidentialité | Aucune donnée envoyée (sauf tuiles Mapbox) |
| Accessibilité | Contraste WCAG AA, textes ≥ 12 px |
| Persistance | Aucune (état en mémoire), sauf token + toggle contours |
| Chargement Mapbox | < 2 secondes (CDN) |
| Initialisation carte | < 1 seconde après token |

### 7.1 Métriques de performance détaillées

| Opération | cible | mesure |
|---|---|---|
| Parse GPX (10 000 pts) | < 200 ms | `performance.now()` |
| computeCumulativeDistances | < 50 ms | — |
| resampleUniform | < 100 ms | — |
| despikeAltitude (win=9) | < 200 ms | — |
| computeAuto complet | < 1 s | — |
| rebuild | < 100 ms | — |
| renderProfile | < 50 ms | — |
| buildTraceFeatures | < 50 ms | — |
| updateMapTrace (setData) | < 100 ms | — |
| exportJson | < 500 ms | — |

### 7.2 Limites connues

- **Trace très longue (> 100 km)** : resampling automatique peut être lent (> 5 s)
- **Très grand nombre de montées (> 100)** : la liste de gauche devient dense
- **Mapbox sur mobile** : rendu 3D peut être lent sur appareils anciens
- **Export JSON sur trace de 50 000 pts** : peut dépasser 1 Mo

---

## 8. Hors périmètre (V3.9.2)

- Carte OSM alternative (seulement Mapbox)
- Export PDF / PNG du profil
- Sauvegarde de la configuration utilisateur (localStorage) — sauf token + contours
- Comparaison multi-tracks
- Détection automatique de « cols » nommés
- Export CSV / JSON alternatif
- Import FIT, TCX
- Édition des seuils de catégorie
- Génération de géométrie LineString complète dans le JSON
- Vue cartographique intégrée dans une autre app (l'utilisateur consomme le JSON)
- Undo/Redo
- Snap magnétique sur les extrema bruts
- Traduction anglaise
- Mode hors-ligne (Mapbox nécessite internet)
- Édition de la trace elle-même (déplacement de points GPS)

---

## 9. Évolutions envisagées (V4+)

| Priorité | Fonctionnalité | Complexité estimée |
|---|---|---|
| **P1** | Export PNG / PDF avec annotations | Moyenne |
| **P1** | Undo/Redo des éditions | Élevée |
| **P1** | Édition des critères de validation dans l'UI | Faible |
| **P1** | Sauvegarde/chargement de la config (localStorage) | Faible |
| **P2** | Carte OSM synchronisée avec le zoom | Moyenne |
| **P2** | Regroupement automatique en « cols » | Élevée |
| **P2** | Import FIT, TCX | Moyenne |
| **P2** | Snap magnétique sur les extrema bruts pendant le drag | Moyenne |
| **P2** | Prévisualisation Mapbox intégrée (consommation directe du JSON) | Faible |
| **P3** | Export CSV des montées détectées | Faible |
| **P3** | Traduction anglaise | Faible |
| **P3** | Anti-pic adaptatif (détection automatique de la fenêtre) | Élevée |
| **P3** | Choix de méthode de découpage du slopeProfile | Faible |
| **P3** | Personnalisation des couleurs de catégories | Faible |

### 9.1 Détail des P1

**Undo/Redo** :
- Historique des `userClimbs` (stack de snapshots)
- `Ctrl+Z` / `Ctrl+Shift+Z` (ou `Ctrl+Y`)
- Limité aux 50 dernières opérations
- Persistance optionnelle (localStorage)

**Édition des critères de validation** :
- Remplacer la constante `CLIMB_CRITERIA` par un state éditable
- Validation : ordre croissant des longueurs, pente décroissante
- Impact sur la détection (à recalculer)

**Sauvegarde config** :
- localStorage de `AnalyzerParameters`
- Restauration au chargement
- Bouton « Réinitialiser aux défauts »

### 9.2 Détail des P2

**Carte OSM** :
- Nécessite un serveur de tuiles OSM ou MapTiler
- Synchronisation du zoom/pitch avec Mapbox
- Alternative gratuite au token Mapbox

**Regroupement automatique en « cols »** :
- Détection des cols nommés (base de données externe ?)
- Ou cluster de montées séparées de < 5 km avec point de passage bas
- Numérotation automatique

**Import FIT/TCX** :
- Parsers dédiés
- Conversion vers le format interne GPX
- Attention : FIT a des messages différents (records, laps, etc.)

**Snap magnétique** :
- Détection des extrema locaux bruts
- Aimantation de la poignée au point le plus proche
- Améliore la précision du drag

---

## 10. Risques et mitigation

| Risque | Impact | Mitigation | Statut |
|---|---|---|---|
| Variabilité des traces GPS | Faux positifs/négatifs | Anti-pic + double lissage + édition | ✅ v3.x |
| Complexité des paramètres | Frustration utilisateur | Diagnostic visuel + valeurs par défaut | ✅ v3.2 |
| Trace très longue | Lenteur | Sous-échantillonnage + Float64Array | ✅ v2.9 |
| Effet de bord entre paramètres | Régressions | Diagnostic de cohérence + tests | ✅ v3.2 |
| Catégorisation trop sensible | Taches noires | Fenêtre glissante | ✅ v0.10 |
| Chevauchement éditions | Incohérence | Contrainte auto + indicateur | ✅ v0.8 |
| Conflit drag / zoom-pan | Impossible de draguer | Capture DOM + désactivation pan | ✅ v0.5 |
| Tooltip masquant pendant drag | Courbes cachées | Désactivation tooltip | ✅ v2.1 |
| Insertion gap trop étroit | Chevauchement | Bouton masqué si gap < 200 m | ✅ v2.2 |
| Anti-pic trop zélé | Écrêtage de vraies bosses | Diagnostic avertit si seuil < 5 m | ✅ v3.3 |
| Anti-pic inefficace | Glitch persiste | Diagnostic avertit si fenêtre < 2 × Pas | ✅ v3.3 |
| Bruit résiduel à petit Pas | Signal bruité malgré Pas fin | Rééchantillonnage par moyenne | ✅ v3.5 |
| Fenêtre anti-pic dépendante du Pas | Comportement imprévisible | Fenêtre en mètres | ✅ v3.6 |
| **Destruction du canvas pendant le drag** | **Crash `chart.canvas is null`** | **`renderAll` inerte pendant drag** | ✅ v3.7.1 |
| **Listeners `mousemove` orphelins** | **Erreurs à chaque survol** | **Retrait explicite (`_hoverListener`)** | ✅ v3.7.1 |
| **Divergence catégories carte ↔ profil (v3.8)** | **Incohérence utilisateur** | **`slopeProfile` aligné + coloration via `catSegments`** | ✅ v3.8.1 |
| **Couleurs changent au drag (v3.8)** | **Instabilité visuelle** | **`eleBaseCat` non ancrée** | ✅ v3.8.1 |
| **Étirement trace au zoom (v3.8)** | **Fausse échelle** | **Échelle X fixe** | ✅ v3.8.3 |
| **Contours invisibles (v3.8.4)** | **Fonctionnalité cassée** | **Ordre des couches** | ✅ v3.8.5 |
| **Zoom carte sur mauvais objet (v3.8.1)** | **Vue incorrecte** | **`zoomMapToClimb` accepte les 2 formats** | ✅ v3.8.2 |
| **Séparation sans refusion possible (v3.9.0)** | **Perte de lien** | **`groupId` + `subIndex`** | ✅ v3.9.1 |
| **Suppression impossible (v3.9.1)** | **UX** | **Bouton ✕** | ✅ v3.9.2 |
| **Marqueurs groupe mono (v3.9.1)** | **Visualisation incomplète** | **Multi-marqueurs** | ✅ v3.9.2 |
| **Trace carte ≠ GPX (v3.8→v3.9.1)** | **Fidélité** | **Points bruts** | ✅ v3.9.2 |
| **Distance exportée ≠ distance réelle** | **Décalage progressions Mapbox** | **`totalDistRawM` distinct** | ✅ v3.7 |
| **JSON surchargé** | **Difficulté d'intégration** | **Format plat, sans FeatureCollection** | ✅ v3.7.2 |
| **Token Mapbox en clair dans le code** | **Fuite** | **localStorage + input utilisateur** | ✅ v3.8 |
| **Perte de token au reload** | **UX** | **localStorage** | ✅ v3.8 |
| **Carte non chargée au démarrage** | **UX** | **Auto-load v3.9.2** | ✅ v3.9.2 |

---

## 11. Critères d'acceptation

| Réf | Critère | Méthode de test | Version |
|---|---|---|---|
| CA-01 | Fichier GPX (10 000 pts) chargé et analysé en < 3 s | Chronométrage | v0.1 |
| CA-02 | Sur trace Morzine, les 7 montées sont détectées | Comparaison visuelle | v0.1 |
| CA-03 | Édition du km de vallée → mise à jour < 500 ms | Chronométrage | v0.7 |
| CA-04 | Boutons `↺`, `✕`, « Recharger auto » fonctionnels | Test manuel | v0.7 |
| CA-05 | Aucun crash sur fichier sans `<ele>` | Fichier corrompu | v0.1 |
| CA-06 | Aucune zone blanche entre segments | Zoom visuel | v0.10 |
| CA-07 | Chevauchement empêché + `⚠` | Édition volontaire | v0.8 |
| CA-08 | 8 catégories affichées | Légende | v0.9 |
| CA-09 | Altitude ancrée passe exactement par les extrema | Zoom | v0.5 |
| CA-10 | Drag & drop d'une poignée fonctionne | Test manuel | v3.8 |
| CA-11 | Tooltip masqué pendant drag, restauré après | Test visuel | v3.8 |
| CA-12 | Boutons insertion visibles aux bons endroits | Vérification | v2.2 |
| CA-13 | Désélection par ✕ ou re-clic | Test manuel | v3.8 |
| CA-14 | Drop-zone reste active après chargement | Recharger | v2.3 |
| CA-15 | Aucune montée sous son seuil de pente | Post-chargement | v3.1 |
| CA-16 | Montées < gapMinM fusionnées avec `⊕` | Trace fragmentée | v3.0 |
| CA-17 | Repli sur sous-segment visible `⤺` | Trace avec fusion | v3.1 |
| CA-18 | Stats D+ brut cohérentes avec trace d'origine | Comparaison | v0.4 |
| CA-19 | Pas de doublons GPS comptés dans D+ | Trace avec duplicats | v0.4 |
| CA-20 | Anti-pic corrige un glitch isolé | Trace + win=200m, seuil=15m | v3.3 |
| CA-21 | Anti-pic ne modifie pas une vraie bosse | Trace + seuil=25 m | v3.3 |
| CA-22 | Diagnostic avertit si fenêtre < 2 × Pas ou seuil < 5 m | Régler fenêtre=50m @ Pas=50 | v3.3 |
| CA-23 | Rendu visuel à Pas=50 m comparable à Pas=25 m | Trace GPS dense | v3.5 |
| CA-24 | Diagnostic 🟢 avec les défauts v3.9.2 | Chargement sans modification | v3.9.2 |
| CA-25 | `avgWinM` affiché dans le log Debug | Vérification log | v3.5 |
| CA-26 | Renommages appliqués : `Fenêtre de calcul pente`, `Longueur min. pente` | Inspection UI | v3.5.2 |
| CA-27 | `Anti-pic : fenêtre` affichée en mètres dans le formulaire et le diagnostic | Inspection UI | v3.6 |
| CA-28 | Fenêtre effective en points affichée sous le champ | Inspection UI | v3.6 |
| CA-29 | Comportement anti-pic indépendant du Pas (test 25/50/100 m) | Changer Pas et vérifier | v3.6 |
| CA-30 | Drag fluide sans erreur console (survol graphiques) | Ouvrir console, draguer | v3.7.1 |
| CA-31 | `totalDistanceKm` reflète la longueur réelle, pas la grille | Comparer avec mesure GPX source | v3.7 |
| CA-32 | `trailingGapM` cohérent avec `totalDistRawM - totalDistSampledM` | Inspection JSON | v3.7 |
| CA-33 | Aucune tranche de `slopeProfile` ne traverse une frontière | Vérifier par inspection | v3.7 |
| CA-34 | `categoryId` d'une tranche = `categoryId` du segment qui la contient | Comparer | v3.7 |
| CA-35 | Export JSON valide et parseable | Ouvrir dans un éditeur JSON | v3.7 |
| CA-36 | `startProgress`/`endProgress` = fraction de `totalDistanceKm` | Vérifier dernier segment | v3.7 |
| CA-37 | Diagnostic `exportProfileStepM` 🟠 si > longueurMin | Régler | v3.7 |
| CA-38 | Aucune clé `profile`, `geometry`, `type`, `lengthM`, `categoryLabel` dans le JSON | Inspection | v3.7.2 |
| CA-39 | **Carte se charge auto si token présent** | localStorage rempli | v3.9.2 |
| CA-40 | **Trace carte = points bruts du GPX** | Comparer visuellement | v3.9.2 |
| CA-41 | **Couverture des couleurs carte = profil** | Compare carte et profil | v3.8.1 |
| CA-42 | **Couleurs stables pendant le drag** | Drag d'une poignée | v3.8.1 |
| CA-43 | **Échelle X profil strictement fixe** | Mesurer entre deux points | v3.8.3 |
| CA-44 | **Séparation crée `#N.1`, `#N.2`** | Cliquer ✂ | v3.9.1 |
| CA-45 | **Refusion restaure la montée fusionnée** | Cliquer 🔗 | v3.9.1 |
| CA-46 | **Refusion conserve les bornes éditées** | Éditer, séparer, refusionner | v3.9.1 |
| CA-47 | **Suppression renumérote les `subIndex`** | Supprimer un membre de groupe | v3.9.2 |
| CA-48 | **Création manuelle refuse les chevauchements** | Double-clic dans une montée | v3.9.0 |
| CA-49 | **Création manuelle insère au bon endroit** | Créer entre deux montées | v3.9.0 |
| CA-50 | **Multi-marqueurs sur groupe** | Sélectionner un membre de groupe | v3.9.2 |
| CA-51 | **Zoom groupe = toute la zone du groupe** | Cliquer sur un membre | v3.9.2 |
| CA-52 | **Profil montée = trace complète + scroll centré** | Sélectionner une montée | v3.9.2 |
| CA-53 | **Échelle montée = 10 m/px** | Mesurer deux points | v3.9.2 |
| CA-54 | **Courbes de niveau visibles** | Toggle dans params | v3.8.5 |
| CA-55 | **Ordre des couches Mapbox** | Carte avec contours activés | v3.8.5 |
| CA-56 | **Bouton ✕ sur chaque carte** | Inspection UI | v3.9.2 |
| CA-57 | **Séparation puis refusion avec édition** | Éditer, séparer, refusionner, vérifier | v3.9.1 |
| CA-58 | **Normalisation des groupes orphelins** | Supprimer membres jusqu'à 1 | v3.9.1 |

---

## 12. Traces de référence

| Nom | Distance | Caractéristique | Utilisation |
|---|---|---|---|
| Bormio–Mortirolo 2025 | ~110 km | Trace éditeur Openrunner, MNT SRTM | Trace principale |
| Trace GPS terrain dense | variable | Pas de 3–5 m, bruit capteur | Test haute densité |
| Trace éditeur fragmentée | 80 km | Multiples faux-plats | Test fusion + séparation |
| Glitch isolé | 40 km | 1–2 points aberrants | Test anti-pic |
| Glitch + fusion | 60 km | Glitch + fragment | Test anti-pic + fusion |
| Trace plate | 50 km | Aucune montée | Anti-faux-positifs |
| MontVernier | 4.28 km | Trace courte avec un seul col | Test export JSON et alignement |
| Trace très courte (< 1 km) | ~500 m | Une seule petite bosse | Test échelle X fixe (canvas étroit) |
| Trace ultra-longue (> 200 km) | 250 km | Ultra-distance | Test performance |
| Trace avec 10 cols enchaînés | 150 km | Cols séparés de 2 km | Test fusion/séparation/refusion |

---

## 13. Interface — wireframe textuel

```
┌──────────────────────────────────────────────────────────────────┐
│ ⛰️ GPX Analyzer v3.9.2                          🐛  ⚙️          │
│ ┌─────────────────────────────────────┐                          │
│ │ 📁 trace.gpx · 3312 → 2190 pts      │                          │
│ └─────────────────────────────────────┘                          │
├────────────┬────────────────────────────────────┬────────────────┤
│ MONTÉES    │ 🗺️ CARTE MAPBOX 3D                 │ PARAMÈTRES 🟢  │
│ DÉTECTÉES  │                                    │                │
│ (3)        │  [terrain + trace colorée]         │ ┌────────────┐ │
│            │  [🛰️] [🎯]                         │ │Mapbox      │ │
│ ┌────────┐ │                                    │ │Token:      │ │
│ │Stats   │ │                                    │ │[pk.xxx][OK]│ │
│ │mini    │ │                                    │ │☐ Contours  │ │
│ └────────┘ │                                    │ └────────────┘ │
│            ├────────────────────────────────────┤                │
│ ┌────────┐ │ 📊 PROFIL ALTIMÉTRIQUE            │ ┌────────────┐ │
│ │ #1 ⊕3  │ │ 50 m/px · 10 m/px  💡 hint   ✕    │ │Diagnostic  │ │
│ │ km 3.4 │ │ [courbe colorée + poignées]        │ │🟢 Cohérent │ │
│ │ 4.7km  │ │                                    │ └────────────┘ │
│ │ 8.2%   │ │                                    │                │
│ └────────┘ │                                    │ ┌────────────┐ │
│ ┌────────┐ │                                    │ │Préparation │ │
│ │ #2 ⤺   │ │                                    │ │Pas    [50] │ │
│ │ km 12  │ │                                    │ │Anti-pic    │ │
│ └────────┘ │                                    │ │win  [200]  │ │
│ ┌────────┐ │                                    │ │thr  [15]   │ │
│ │#3.1 🔗 │ │                                    │ └────────────┘ │
│ │#3.2 🔗 │ │                                    │                │
│ │#3.3 🔗 │ │                                    │ ┌────────────┐ │
│ └────────┘ │                                    │ │Lissage     │ │
│            │                                    │ │...         │ │
│            │                                    │ └────────────┘ │
│            │                                    │                │
│            │                                    │ ┌────────────┐ │
│            │                                    │ │Détection   │ │
│            │                                    │ │...         │ │
│            │                                    │ └────────────┘ │
│            │                                    │                │
│            │                                    │ [↺ Recharger]  │
│            │                                    │ [💾 Export]    │
└────────────┴────────────────────────────────────┴────────────────┘
```

### 13.1 États visuels des cartes de montée

```
Montée autonome (normale):
┌───────────────────────────────┐
│ #1  km 3.420                  │
│ Long. 4.7 km  D+ 385 m  8.2% │
└───────────────────────────────┘

Montée fusionnée:
┌───────────────────────────────┐
│ #1 ⊕ 3  km 3.420  ✂ Séparer  │
│ Long. 4.7 km  D+ 385 m  8.2% │
└───────────────────────────────┘

Montée avec repli:
┌───────────────────────────────┐
│ #2 ⤺  km 12.050               │
│ Long. 2.1 km  D+ 165 m  7.8% │
└───────────────────────────────┘

Membre de groupe:
┌───┐ (bordure colorée)
│#3.1 🔗 3  km 20.4  🔗 Refusionner│
│ Long. 1.5 km  D+ 90 m  6.0%   │
└───────────────────────────────┘

Sélectionnée:
┌───────────────────────────────┐
│ #1 ⊕ 3  km 3.420  ✂ Séparer  │ ← fond bleu clair
│ Long. 4.7 km  D+ 385 m  8.2% │
└───────────────────────────────┘
```

### 13.2 États du widget profil

```
Mode full:
┌──────────────────────────────────────────────┐
│ Profil altimétrique complet                  │
│ 50 m/px · 10 m/px                            │
│ 💡 Double-cliquez pour créer une montée      │
│ [courbe complète + scroll]                   │
└──────────────────────────────────────────────┘

Mode montée:
┌──────────────────────────────────────────────┐
│ Montée #3 — ajuster les bornes               │
│ 10 m/px · 10 m/px                        ✕   │
│ 💡 Glissez les poignées                      │
│ [courbe complète + scroll centré]            │
└──────────────────────────────────────────────┘

Mode création (vallée posée):
┌──────────────────────────────────────────────┐
│ Profil altimétrique complet                  │
│ 50 m/px · 10 m/px                            │
│ 🎯 Cliquez-glissez pour définir le sommet    │ ← vert pulsant
│ [zone verte/rouge en cours de création]      │
└──────────────────────────────────────────────┘
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
| **Désélection** | Sortir du mode édition d'une montée (clic ✕ ou re-clic carte) |
| **Drop-zone persistante** | Zone de dépôt toujours visible, mode compact après chargement |
| **Rééchantillonnage** | Conversion d'une série irrégulière en pas fixe |
| **Rééchantillonnage par moyenne** | Chaque point de sortie = moyenne des points bruts dans une fenêtre centrée |
| **Fusion (`⊕`)** | Montée étendue pour englober une suivante trop proche |
| **Repli (`⤺`)** | Repli sur le meilleur sous-segment valide |
| **SG** | Savitzky-Golay, lisseur polynomial |
| **Plateau GPS/MNT** | Série d'altitudes identiques (capteur figé ou même cellule MNT) |
| **Anti-pic** | Filtre médian conditionnel — ne remplace que les points aberrants |
| **Fenêtre anti-pic** | Largeur (en mètres) du voisinage inspecté |
| **Diagnostic** | Panneau de cohérence en temps réel des paramètres |
| **Valeur effective** | Valeur calculée en points (= paramètre en mètres ÷ stepM) |
| **`totalDistRawM`** | Longueur réelle de la trace GPX d'origine, avant resampling |
| **`totalDistSampledM`** | Longueur couverte par la grille resamplée (multiple de `stepM`) |
| **`trailingGapM`** | Queue de trace au-delà du dernier multiple de `stepM`, ignorée par le resampling |
| **`startProgress` / `endProgress`** | Fractions 0–1 de la longueur totale, compatibles `line-gradient` Mapbox |
| **`slopeProfile`** | Sous-profil de pente d'une montée, découpé en tranches alignées sur les segments globaux |
| **`catSegments`** | Liste des segments de catégorie fusionnés (utilisée par la carte ET le profil) |
| **`eleDisplay`** | Altitude lissée **ancrée** sur les bornes des montées (courbe affichée) |
| **`eleBaseCat`** | Altitude lissée **non ancrée** (utilisée pour la catégorisation des couleurs) |
| **`groupId`** | Identifiant entier du groupe de montées (null si autonome) |
| **`subIndex`** | Position d'un membre dans son groupe (1, 2, 3…) |
| **`mergedParts`** | Tableau des bornes des sous-montées d'origine (permet la séparation) |
| **`mergedCount`** | Nombre de fusions ayant contribué à une montée |
| **Mapbox GL JS** | Bibliothèque de cartographie vectorielle 3D (Mapbox) |
| **`mapbox-terrain-v2`** | Source vectorielle Mapbox pour les courbes de niveau |
| **`mapbox-dem`** | Source raster-dem Mapbox pour le terrain 3D |
| **`fitBounds`** | Fonction Mapbox pour zoomer sur une emprise |
| **`localStorage`** | Stockage persistant du navigateur |
| **Séparation (`✂`)** | Découpage d'une montée fusionnée en ses sous-montées d'origine |
| **Refusion (`🔗`)** | Reconstitution d'une montée fusionnée à partir de ses sous-montées |
| **Normalisation** | Dissolution automatique des groupes à 1 membre |
| **Suppression (`✕`)** | Retrait d'une montée de la liste |

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
| 3.6 | 2026-09-24 | Anti-pic : fenêtre en mètres (50–500, défaut 200) · Pas par défaut 50 m · Lissage altitude 500 m |
| 3.7 | 2026-09-24 | Export JSON : segments + montées + slopeProfile · Seuil descente raide −7 % · `totalDistRawM` / `totalDistSampledM` / `trailingGapM` · `startProgress`/`endProgress` · `slopeProfile` aligné · Diagnostic `exportProfileStepM` |
| 3.7.1 | 2026-09-24 | Correctif drag : `renderAll` inerte · Nettoyage listeners · Garde-fous canvas null |
| 3.7.2 | 2026-09-24 | JSON allégé : suppression `categoryDistances`, `segmentsTimeline`, `FeatureCollection`, `profile`, `lengthM`, `dPlusM` (slopeProfile), `categoryLabel`, `refineM` |
| **3.8** | **2026-10-15** | **Refonte UI 3 colonnes · Carte Mapbox 3D · Widget profil canvas 2D · Suppression Chart.js · Chargement paresseux Mapbox** |
| **3.8.1** | **2026-10-15** | **Catégorisation sur `eleBaseCat` non ancrée · Coloration profil via `catSegments` · Correctif divergence carte/profil** |
| **3.8.2** | **2026-10-15** | **Correctif `zoomMapToClimb` acceptant `{startKm, endKm}` et `{startDist, endDist}` · Emprise minimale 150 m** |
| **3.8.3** | **2026-10-16** | **Correctif échelle X profil strictement fixe (`canvasWidth = viewWidthM / scaleX`)** |
| **3.8.4** | **2026-10-16** | **Courbes de niveau optionnelles (`mapbox-terrain-v2`)** |
| **3.8.5** | **2026-10-16** | **Correctif ordre couches Mapbox : trace avant contours (`beforeId`)** |
| **3.9.0** | **2026-10-16** | **Séparation des montées fusionnées en sous-montées · Création manuelle par double-clic + drag · Conservation `mergedParts`** |
| **3.9.1** | **2026-10-16** | **Groupes persistants (`groupId` + `subIndex`) · Refusion · Numérotation `#N.M` · Couleur stable par groupe** |
| **3.9.2** | **2026-10-17** | **Échelle widget montée 10 m/px · Auto-load carte si token · Suppression ✕ · Multi-marqueurs groupe · Widget montée = trace complète + scroll centré · Trace carte sur points bruts du GPX** |

---

## 16. Annexes

### 16.1 Formule de Haversine

```
a = sin²(Δφ/2) + cos(φ1) × cos(φ2) × sin²(Δλ/2)
c = 2 × atan2(√a, √(1−a))
d = R × c    avec R = 6 371 000 m
```

### 16.2 Rééchantillonnage par moyenne

```
Pour chaque position cible k × stepM :
  - Calculer la fenêtre de moyennage :
      avgWin = clamp(stepM, 75 m, 3 × stepM)
  - Moyenner tous les points bruts dont la distance cumulée tombe
    dans [k × stepM − avgWin/2, k × stepM + avgWin/2]
  - Si aucun point dans la fenêtre : fallback par interpolation linéaire

Longueur couverte par la grille :
  totalDistSampledM = floor(totalDistRawM / stepM) × stepM
Queue ignorée :
  trailingGapM = totalDistRawM − totalDistSampledM
```

### 16.3 Anti-pic Hampel avec fenêtre en mètres

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

### 16.4 Fusion par distance avec `mergedParts`

```
Si (nouvelle montée startM − précédent endM) < gapMinM :
  - Si !prev._mergedParts :
      prev._mergedParts = [{
        startKm: dist[prev.startIdx] / 1000,
        endKm: dist[prev.endIdx] / 1000
      }]
  - Nouveau sommet = argmax entre précédent.endIdx et nouveau.endIdx
  - Nouveau creux = argmin entre précédent.endIdx et nouveau.startIdx
                   si plus bas que précédent.startIdx
  - prev._mergedParts.push({
      startKm: dist[newStart] / 1000,
      endKm: dist[newEnd] / 1000
    })
  - Étendre la montée précédente
  - mergedCount++
```

### 16.5 Validation finale stricte

```
Pour chaque montée raffinée+fusionnée :
  length = dist[c.endIdx] − dist[c.startIdx]
  avgSlope = D+ / length

  Si avgSlope < seuil(length) :
    sub = findBestSubClimb(dist, eleDisplay, c.startIdx, c.endIdx)
    Si sub ≠ null :
      → repli (⤺) : bornes = [sub.startIdx, sub.endIdx]
      → mergedParts conservés
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

### 16.7 Découpage du `slopeProfile`

```
Pour chaque montée [startIdx, endIdx] :

  1. Identifier les segments globaux intersectant la montée :
       relevantSegs = catSegments.filter(s =>
         s.endDist > climbStartDist && s.startDist < climbEndDist)

  2. Pour chaque segment intersectant :
       segStart = max(seg.startDist, climbStartDist)
       segEnd   = min(seg.endDist,   climbEndDist)
       segLen   = segEnd − segStart

  3. Découper en tranches uniformes :
       nChunks  = max(1, ceil(segLen / profileStepM))
       chunkLen = segLen / nChunks

  4. Pour chaque tranche :
       - kmStart / kmEnd   = bornes réelles (arrondies 4 décimales)
       - eleStartM / eleEndM = altitudes lissées ancrées
       - avgSlopePct       = pente physique D+/longueur
       - categoryId        = seg.key   ← hérité du segment parent

Propriété garantie :
  Aucune tranche ne traverse une frontière de segment.
  Le categoryId de chaque tranche est strictement identique
  à celui du segment global qui la contient.
```

### 16.8 Groupes — séparation et refusion

**Séparation (`✂`)** :
```
gid = nextGroupId++
Retirer la montée fusionnée
Créer une montée par sous-montée :
  - startKm, endKm : depuis mergedParts
  - autoStartKm, autoEndKm : null
  - refineM, candidateLen : null
  - mergedCount : 0
  - mergedParts : null
  - fallback : false
  - groupId : gid
  - subIndex : k + 1
Trier par startKm
Sélectionner la première
Zoomer la carte sur tout le groupe
```

**Refusion (`🔗`)** :
```
members = userClimbs.filter(c => c.groupId === gid)
  .sort((a, b) => a.startKm - b.startKm)

startKm = members[0].startKm
endKm = members[members.length - 1].endKm
mergedParts = members.map(m => ({ startKm: m.startKm, endKm: m.endKm }))

Retirer tous les membres
Créer la montée fusionnée :
  - mergedCount = members.length - 1
  - mergedParts = mergedParts
  - groupId = null, subIndex = null
Trier par startKm
Sélectionner la montée fusionnée
```

### 16.9 Création manuelle

```
État idle → dblclick sur profil full → pending
État pending → mousedown + drag → dragging
État dragging → mouseup → validation

Validation :
  - summitKm > valleyKm + 50 m ?
  - isFreeRange(valleyKm, summitKm) ?
  - Si OUI : addManualClimb()
  - Si NON : annulation

Annulation : Échap
```

### 16.10 Normalisation des groupes

```
Appelée :
  - À la fin de chaque drag de poignée
  - Après recomputeAuto
  - Après deleteClimb

Logique :
  counts = compter les occurrences de chaque groupId
  Pour chaque c dans userClimbs :
    Si c.groupId != null ET counts[c.groupId] < 2 :
      c.groupId = null
      c.subIndex = null
```

### 16.11 Valeurs par défaut v3.9.2

| Paramètre | Défaut | Unité |
|---|---|---|
| Pas de rééchantillonnage | 50 | m |
| Anti-pic : fenêtre | 200 | m |
| Anti-pic : seuil | 15 | m |
| Lissage altitude | 500 | m |
| Lissage pente | 150 | m |
| Fenêtre de calcul pente | 100 | m |
| Seuil détection | 0.5 | % |
| Longueur min. pente | 200 | m |
| Écart min. entre montées | 250 | m |
| Recherche extrema | 60 | % |
| Filtre médian | 0 (désactivé) | m |
| Lissage gaussien | 75 | m |
| Correction plateaux | 6 | pts |
| Segments debug | 100 | m |
| Pas du profil exporté | 50 | m |
| Double SG | ON | — |
| Lissage segments | ON | — |

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
Pas du profil exporté ........... 50 m
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
Pas du profil exporté ........... 50 m
```

### 16.12 Matrice des corrections de bugs connus

| Version | Bug | Correctif |
|---|---|---|
| v3.1 | Pente moyenne sous son seuil | Validation stricte sur bornes finales + repli |
| v3.7.1 | Drag cassé par destruction du canvas | `renderAll` inerte pendant drag + garde-fous |
| v3.7.1 | Listeners mousemove orphelins | Retrait explicite `_hoverListener` |
| v3.8.1 | Couleurs carte ≠ profil | Coloration profil via `catSegments` |
| v3.8.1 | Couleurs changent au drag | `eleBaseCat` non ancrée |
| v3.8.2 | Clic montée ne zoome pas | `zoomMapToClimb` accepte 2 formats |
| v3.8.3 | Échelle X étirée sur trace courte | `canvasWidth = viewWidthM / scaleX` |
| v3.8.5 | Contours invisibles | Ordre couches (trace avant contours) |
| v3.9.0 | Séparation sans refusion | `groupId` + `subIndex` (v3.9.1) |
| v3.9.1 | Suppression impossible | Bouton ✕ (v3.9.2) |
| v3.9.1 | Marqueurs groupe mono | Multi-marqueurs (v3.9.2) |
| v3.9.1 | Trace carte = grille resamplée | Points bruts (v3.9.2) |

### 16.13 Matrice d'accessibilité

| Élément | Contraste | Taille minimale | Navigation clavier |
|---|---|---|---|
| Texte principal | WCAG AA (4.5:1) | 14 px | — |
| Texte secondaire | WCAG AA (4.5:1) | 12 px | — |
| Boutons | WCAG AA | 38 px (icône) | Tab + Enter |
| Champs input | WCAG AA | 14 px | Tab |
| Cartes de montée | WCAG AA | 12 px | Tab (focus visible) |
| Poignées de drag | — | 10 px (visuel) / 14 px (hit) | Flèches + Shift + Alt |
| Bouton ✕ | WCAG AA | 20 px | Tab + Enter |
| Bouton ✂ | WCAG AA | 20 px | Tab + Enter |
| Bouton 🔗 | WCAG AA | 20 px | Tab + Enter |
| Carte Mapbox | — | — | Navigation Mapbox standard |
| Canvas profil | — | — | Drag + flèches |

### 16.14 Compatibilité navigateurs

| Fonctionnalité | Chrome | Firefox | Safari | Edge |
|---|---|---|---|---|
| Canvas 2D | ✅ | ✅ | ✅ | ✅ |
| Mapbox GL JS | ✅ | ✅ | ✅ | ✅ |
| localStorage | ✅ | ✅ | ✅ | ✅ |
| FileReader | ✅ | ✅ | ✅ | ✅ |
| Pointer Events | ✅ | ✅ | ✅ | ✅ |
| ResizeObserver | ✅ | ✅ | ✅ | ✅ |
| CSS Grid / Flexbox | ✅ | ✅ | ✅ | ✅ |
| `<dialog>` (V4) | ✅ | ✅ | ✅ | ✅ |

### 16.15 Checklist de release v3.9.2

- [ ] Tous les tests unitaires passent
- [ ] Tous les tests d'intégration passent
- [ ] Tous les critères d'acceptation validés
- [ ] Diagnostic 🟢 avec la config par défaut
- [ ] Chargement d'une trace de référence sans erreur
- [ ] Carte se charge auto avec token en localStorage
- [ ] Trace sur la carte = points bruts du GPX
- [ ] Séparation/refusion fonctionnelle avec préservation des éditions
- [ ] Suppression renumérote les `subIndex`
- [ ] Création manuelle refuse les chevauchements
- [ ] Multi-marqueurs visibles sur montée groupée
- [ ] Widget montée à 10 m/px avec scroll centré
- [ ] Courbes de niveau visibles si toggle activé
- [ ] Export JSON contient `groupId` et `subIndex`
- [ ] Pas d'erreur console dans les scénarios nominaux
- [ ] Documentation à jour (MUT, Algorithmes, Note technique, PRD)

---

**Fin du PRD v3.9.2**