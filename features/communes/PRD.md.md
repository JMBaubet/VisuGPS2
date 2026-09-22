# PRD — Communes GPX

**Détection des communes traversées par une trace GPX**

| | |
|---|---|
| **Version** | 1.0 |
| **Statut** | Draft |
| **Date** | 2026-09-18 |
| **Auteur** | — |
| **Type** | Application web locale, mono-fichier |
| **Public cible** | Randonneurs, cyclistes, analystes SIG, cyclotouristes |

---

## Table des matières

1. [Contexte](#1-contexte)
2. [Objectifs](#2-objectifs)
3. [Utilisateurs cibles](#3-utilisateurs-cibles)
4. [Périmètre](#4-périmètre)
5. [Fonctionnalités détaillées](#5-fonctionnalités-détaillées)
6. [Paramètres](#6-paramètres)
7. [Critères d'acceptation](#7-critères-dacceptation)
8. [Contraintes](#8-contraintes)
9. [Risques et mitigation](#9-risques-et-mitigation)
10. [Axes d'amélioration](#10-axes-damélioration)
11. [Annexes](#11-annexes)

---

## 1. Contexte

### 1.1 Problème

Après une sortie, l'utilisateur qui analyse sa trace GPX veut souvent connaître **la liste précise des communes traversées**, avec les kilomètres d'entrée et de sortie de chaque commune. Cette information est :

- **fastidieuse à obtenir manuellement** (lecture d'une carte, croisement avec la trace) ;
- **difficile à obtenir de façon fiable** sur les traces traversant plusieurs régions ou pays (France, Espagne, Italie, Suisse) ;
- **indispensable** pour préparer un dossier administratif (déclaration de parcours, autorisations), calculer un budget de péage ou d'indemnité kilométrique, ou préparer une publication.

Les outils existants (Strava, Garmin Connect, Komoot) n'exposent pas cette information.

### 1.2 État de l'art

| Outil | Fonctionnalité | Limite |
|---|---|---|
| Strava | Analyse de trace | Aucune info communale |
| Garmin Connect | Visualisation | Aucune info communale |
| OpenRunner | Édition | Pas d'extraction communale |
| Géoportail | Limites administratives | Croisement manuel |
| QGIS + Overpass | Analyse ad hoc | Compétences SIG requises |
| **Communes GPX** | **Détection automatique multi-pays** | **À construire** |

### 1.3 Contrainte forte

**Aucune donnée ne doit quitter le poste de l'utilisateur.** Pas d'upload de trace vers un serveur tiers, pas de télémétrie, pas de compte à créer. Les seules requêtes réseau sont :

- le chargement initial des bibliothèques (Turf.js, osmtogeojson) depuis un CDN ;
- les requêtes Overpass (API OSM) nécessaires à l'analyse, qui n'envoient **que la bounding box de la trace**, jamais la trace elle-même.

---

## 2. Objectifs

### 2.1 Objectif principal

Produire, à partir d'un fichier GPX, la **liste ordonnée des communes traversées**, avec pour chaque passage les **kilomètres d'entrée et de sortie** le long de la trace.

### 2.2 Objectifs secondaires

- Fonctionner pour la **France, l'Espagne, l'Italie et la Suisse**.
- Gérer les traces jusqu'à **200 km** avec une précision métrique.
- Offrir un **export CSV et JSON tabulaire** directement exploitable dans Excel, QGIS, pandas.
- Fonctionner **entièrement dans le navigateur** (aucun backend).
- Fournir un **mini-graphique de répartition** des communes le long de la trace.
- Résister aux **polygones OSM défectueux** (multipolygones cassés, chevauchements anormaux).

### 2.3 Indicateurs de succès (v1.0)

| Indicateur | Cible |
|---|---|
| Temps d'analyse pour 30 km | < 15 s (dont ~10 s d'attente Overpass) |
| Temps d'analyse pour 200 km | < 60 s |
| Précision des points d'entrée/sortie | < 10 m d'erreur |
| Taille du fichier HTML | < 30 KB hors CDN |
| Fonctionne hors ligne après chargement initial | Non (Overpass requis) |

---

## 3. Utilisateurs cibles

### 3.1 Personas

**Cyclotouriste / randonneur longue distance**
- Traces de 50 à 200 km traversant plusieurs communes, parfois plusieurs départements.
- Veut un récapitulatif chronologique avec les kilomètres, pour préparer un carnet de route ou un dossier de demande de subvention.

**Organisateur de randonnée**
- Prépare un parcours officiel et doit déclarer les communes traversées pour obtenir les autorisations.
- A besoin d'un export CSV exploitable pour sa communication préfecture.

**Analyste SIG occasionnel**
- Veut croiser la trace avec d'autres données (zonage, PLU, cadastre).
- A besoin d'un export tabulaire simple, sans géométrie.

**Voyageur multi-pays**
- Trace traversant France, Italie, Suisse.
- A besoin que l'analyse fonctionne sans configuration manuelle des niveaux administratifs.

### 3.2 Cas d'usage détaillés

**UC-1 : Préparation d'un brevet cyclotouriste**
- Un cyclotouriste prépare un 200 km. Il veut la liste des communes pour préparer ses points de ravitaillement.
- Il charge le GPX, obtient la liste avec les km d'entrée et sortie, exporte le CSV.

**UC-2 : Déclaration d'une randonnée officielle**
- L'organisateur charge la trace de 80 km.
- Il exporte le JSON, l'envoie à la préfecture avec la liste.

**UC-3 : Analyse d'une trace transfrontalière**
- Une trace Nice-Menton passe en Italie. L'analyse doit retourner les communes françaises (`admin_level=8`) et italiennes (`admin_level=8`).

**UC-4 : Diagnostic des erreurs OSM**
- Une trace affiche un résultat incohérent (chevauchement).
- La console affiche les polygones suspects, l'utilisateur peut vérifier dans OSM.

---

## 4. Périmètre

### 4.1 Inclus (v1.0)

- Import d'un **fichier GPX local** par drag-drop ou sélection.
- Requête Overpass sur l'**API publique** avec fallback sur 4 miroirs.
- Détection des communes (`admin_level=8`) intersectant la trace.
- Filtrage des **polygones aberrants** (surface > 10× la médiane).
- Analyse fine par échantillonnage (100 m) et raffinement par intersection géométrique.
- Affichage :
    - **mini-graphique horizontal** : une barre par passage, positionnée sur l'axe kilométrique ;
    - **tableau chronologique** trié par km d'entrée.
- **Export CSV** et **JSON tabulaire**.
- **Badges** de passage multiples (`passage 1/2`, etc.).
- **Diagnostic console** (timeline, segments bruts, candidats, chevauchements).

### 4.2 Exclu (v1.0)

- Carte interactive (visualisation Leaflet).
- Détection des communes par d'autres niveaux (`admin_level=6` pour les départements, etc.).
- Gestion du multi-fichier.
- Prise en compte de l'altitude ou du temps.
- Caching local des polygones OSM.
- Installation hors ligne.

### 4.3 Reporté (v1.x)

Voir [section 10](#10-axes-damélioration).

---

## 5. Fonctionnalités détaillées

### 5.1 Import

**F-01 — Dépôt de fichier GPX**
- Zone de dépôt dans la page, support drag-drop **et** clic pour ouverture du sélecteur natif.
- Accepte uniquement les fichiers `.gpx` (test sur l'extension).
- Message d'erreur explicite si fichier invalide.

**F-02 — Lecture et parsing**
- Parsing XML via `DOMParser`.
- Extraction des `<trkpt>` (sinon `<rtept>` en fallback, puis `<wpt>`).
- Filtrage des coordonnées aberrantes (`|lat|>90`, `|lon|>180`, point `(0,0)`).
- Minimum 2 points requis.

**F-03 — Simplification de la trace**
- Simplification Turf avec tolérance 5 m pour accélérer l'analyse.
- Conservation de la trace simplifiée uniquement pour les calculs internes.

### 5.2 Analyse

**F-04 — Calcul de la bounding box**
- Calcul via `turf.bbox` sur la trace simplifiée.
- Élargissement de 0.01° (~1 km) pour capturer les communes en bordure.

**F-05 — Requête Overpass**
- Requête POST sur `overpass-api.de/api/interpreter`.
- Fallback automatique sur 3 miroirs si erreur HTTP.
- Récupération de toutes les relations `boundary=administrative`, `admin_level=8` dans la bbox.
- Timeout : 180 s (mais le frontend Overpass coupe plus tôt, d'où l'importance du fallback).

**F-06 — Conversion OSM → GeoJSON**
- Utilisation de `osmtogeojson` pour convertir la réponse JSON Overpass en `FeatureCollection`.

**F-07 — Filtrage préliminaire**
- Filtrage par `turf.booleanIntersects` : seules les communes dont le polygone intersecte la trace sont conservées.

**F-08 — Filtre des polygones aberrants**
- Calcul de la surface médiane des communes candidates.
- Exclusion de toute commune dont la surface dépasse 10× cette médiane.

**F-09 — Analyse fine par échantillonnage**
- Rééchantillonnage de la trace à pas fixe (100 m minimum).
- Pour chaque échantillon : identification de la commune contenante.
- Arbitrage par **distance au centroïde** en cas de chevauchement.

**F-10 — Détection des segments**
- Construction d'une **timeline** : tableau où chaque case contient la commune gagnante (ou `null`).
- Extraction des segments contigus (une ligne par passage).

**F-11 — Raffinement des points d'entrée/sortie**
- Calcul de l'intersection exacte entre le petit segment de trace (≤ 100 m) et la frontière communale.
- Conversion en kilomètre par **interpolation locale** entre les deux échantillons encadrants.

**F-12 — Filtrage des micro-segments**
- Exclusion des passages < 15 m (bruit lié à la simplification ou aux polygones OSM).

**F-13 — Détection des passages multiples**
- Toute commune traversée plusieurs fois apparaît comme plusieurs segments distincts.
- Chaque passage est numéroté (`passage 1/N`, `2/N`, etc.).

### 5.3 Restitution

**F-14 — Mini-graphique**
- Une barre horizontale par passage, positionnée en pourcentage de la longueur totale.
- Couleur déterministe par index (`hsl((i × 47) % 360, ...)`).
- Libellé avec numéro de passage si multiple.

**F-15 — Tableau chronologique**
- Tri strict par km d'entrée.
- Colonnes : `Km entrée`, `Commune`, `Km sortie`, `Longueur`.
- Badge `passage N/M` si traversée multiple.

**F-16 — Résumé**
- Nombre de passages et de communes distinctes.
- Kilométrage cumulé dans les communes (peut être légèrement inférieur à la longueur totale si une portion passe hors commune).
- Nombre de relations OSM reçues, de communes retenues, de communes exclues.

### 5.4 Export

**F-17 — Export CSV**
- Une ligne par passage.
- Colonnes : `Km_entree`, `Commune`, `Passage`, `Nb_passages_total`, `Km_sortie`, `Longueur_km`.
- Encodage UTF-8, séparateur virgule, guillemets sur tous les champs.

**F-18 — Export JSON**
- Tableau JSON simple (pas de GeoJSON, pas de géométrie).
- Champs : `commune`, `passage`, `nb_passages_total`, `km_entree`, `km_sortie`, `longueur_km`.
- Indentation de 2 espaces pour lisibilité.

### 5.5 Diagnostic

**F-19 — Logs console**
- Timeline compacte (`CCCCAAAABBBBCCCC`).
- Surfaces et centroïdes des candidates.
- Liste des polygones exclus par surface.
- Nombre de chevauchements détectés.
- Position du point de départ / d'arrivée.

---

## 6. Paramètres

| Paramètre | Symbole | Défaut | Rôle |
|---|---|---|---|
| `adminLevel` | `AL` | `"8"` | Niveau administratif OSM ciblé (communes) |
| `bboxBufferDeg` | `BB` | `0.01` | Élargissement de la bbox de recherche (°) |
| `simplifyTolerance` | `ST` | `0.00005` | Tolérance de simplification Turf (~5 m) |
| `targetSamples` | `TS` | `2000` | Nombre cible d'échantillons |
| `minSampleStepKm` | `MS` | `0.1` | Pas minimal entre échantillons (100 m) |
| `maxAreaRatio` | `MA` | `10` | Facteur d'exclusion par surface |
| `minSegmentKm` | `MG` | `0.015` | Longueur minimale d'un passage retenu (15 m) |

Ces paramètres sont définis dans l'objet `CONFIG` en haut du script et peuvent être ajustés par l'utilisateur s'il modifie le fichier HTML.

---

## 7. Critères d'acceptation

### 7.1 Fonctionnels

| ID | Critère |
|---|---|
| **CA-01** | L'import d'un GPX valide affiche le tableau des communes en < 30 s (Overpass inclus). |
| **CA-02** | Un fichier non-GPX déclenche un message d'erreur explicite. |
| **CA-03** | Un GPX avec moins de 2 points déclenche une erreur. |
| **CA-04** | Une trace de 13.57 km près de Magny-le-Hongre retourne 3 communes distinctes. |
| **CA-05** | La somme des longueurs de passages ≈ la longueur totale de la trace (± 5 %). |
| **CA-06** | Un passage < 15 m est ignoré. |
| **CA-07** | Une commune-enveloppe ne couvre pas toute la trace. |
| **CA-08** | Le tableau est trié strictement par km d'entrée croissant. |
| **CA-09** | Un passage multiple porte un badge `passage N/M`. |
| **CA-10** | L'export CSV contient une ligne par passage, avec séparateur virgule. |
| **CA-11** | L'export JSON est un tableau d'objets avec 6 clés. |
| **CA-12** | Le mini-graphique affiche une barre par passage. |
| **CA-13** | La console affiche la timeline complète. |

### 7.2 Non-fonctionnels

| ID | Critère |
|---|---|
| **CA-NF-01** | Le fichier `.html` complet fait < 30 KB hors libs CDN. |
| **CA-NF-02** | Fonctionne dans Chrome, Firefox, Safari (ES2020+). |
| **CA-NF-03** | Aucune trace n'est envoyée à un serveur tiers. |
| **CA-NF-04** | Le navigateur ne se fige pas pendant l'analyse (feedback de progression). |
| **CA-NF-05** | L'analyse est robuste aux miroirs Overpass en panne (fallback). |

---

## 8. Contraintes

### 8.1 Techniques

- **Mono-fichier HTML** : CSS et JS inline.
- **Libs via CDN** : Turf.js 6, osmtogeojson 3.
- **Aucun backend** : tout se passe dans le navigateur.
- **API Overpass publique** : soumise à quotas, tolérée pour usage occasionnel.

### 8.2 Réglementaires

- **RGPD** : aucune donnée personnelle.
- **Licences** : Turf MIT, osmtogeojson MIT, données OSM ODbL.
- **Attribution OSM obligatoire** dans toute publication utilisant les résultats.

### 8.3 Budgétaires

- Projet personnel, aucun budget externe.

---

## 9. Risques et mitigation

| Risque | Impact | Probabilité | Mitigation |
|---|---|---|---|
| API Overpass saturée (HTTP 504) | Moyen | Moyenne | Fallback automatique sur 4 miroirs |
| Polygone OSM défectueux | Moyen | Faible | Filtre de surface + arbitrage par centroïde |
| Quota Overpass dépassé | Faible | Faible | Usage occasionnel uniquement |
| Trace très longue (200 km) | Faible | Moyenne | Pas plafonné, mais temps d'attente Overpass long |
| Frontière OSM imprécise | Faible | Élevée | Précision documentée, non corrigeable côté client |
| Commune traversée mais non taguée dans OSM | Faible | Faible | Signalée dans le résumé (relations reçues vs retenues) |

---

## 10. Axes d'amélioration

### 10.1 Priorité haute

- **A-01** — Carte interactive Leaflet pour visualiser les points d'entrée/sortie.
- **A-02** — Sélection du niveau administratif (départements, régions, cantons).
- **A-03** — Cache local des polygones dans `localStorage` (IndexedDB) pour éviter les requêtes Overpass répétées.

### 10.2 Priorité moyenne

- **A-04** — Export GeoJSON avec géométrie de chaque tronçon.
- **A-05** — Filtrage par type de commune (rural/urbain) via densité de population.
- **A-06** — Prise en compte de l'altitude (dénivelé par commune).
- **A-07** — Détection multi-niveaux (commune + département + région en une seule requête).

### 10.3 Priorité basse

- **A-08** — Mode clair / sombre.
- **A-09** — Impression PDF du tableau.
- **A-10** — Comparaison de plusieurs traces.

---

## 11. Annexes

### 11.1 Glossaire

| Terme | Définition |
|---|---|
| **Trace** | Suite ordonnée de points GPS. |
| **Commune** | Entité administrative de niveau `admin_level=8` en France, Espagne, Italie, Suisse. |
| **Passage** | Portion de trace située dans une commune, entre un point d'entrée et un point de sortie. |
| **Point d'entrée** | Coordonnée du franchissement de la frontière communale (entrée). |
| **Point de sortie** | Coordonnée du franchissement de la frontière communale (sortie). |
| **Communes-enveloppe** | Polygone OSM défectueux dont la surface englobe ses voisines. |
| **Timeline** | Représentation interne : tableau où chaque case indique la commune gagnante à un point donné. |

### 11.2 Références

- **GPX 1.1** : https://www.topografix.com/GPX/1/1/
- **Overpass API** : https://wiki.openstreetmap.org/wiki/Overpass_API
- **Turf.js** : https://turfjs.org/
- **osmtogeojson** : https://github.com/tyrasd/osmtogeojson

### 11.3 Historique des décisions

| Date    | Décision                            | Motif                                          |
| ------- | ----------------------------------- | ---------------------------------------------- |
| 2026-09 | Pas de géométrie dans l'export JSON | Réponse à la demande utilisateur               |
| 2026-09 | Arbitrage par distance au centroïde | Résout le bug Magny-le-Hongre                  |
| 2026-09 | Interpolation locale du km          | Résout l'ambiguïté sur les traces aller-retour |
| 2026-09 | Timeline compacte                   | Diagnostic visuel immédiat                     |
| 2026-09 | Bbox buffer réduite à 0.01°         | Évite le 504 Overpass                          |