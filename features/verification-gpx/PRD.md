# PRD — Multi-Sens

**Détection de portions répétées dans une trace GPX**

| | |
|---|---|
| **Version** | 1.0 |
| **Statut** | Draft |
| **Date** | 2026-09-16 |
| **Auteur** | — |
| **Type** | Application web locale, mono-fichier |
| **Public cible** | Randonneurs, cyclistes, traileurs, analystes SIG |

---

## Table des matières

1. [Contexte](#1-contexte)
2. [Objectifs](#2-objectifs)
3. [Utilisateurs cibles](#3-utilisateurs-cibles)
4. [Périmètre](#4-périmètre)
5. [Fonctionnalités détaillées](#5-fonctionnalités-détaillées)
6. [Paramètres utilisateur](#6-paramètres-utilisateur)
7. [Critères d'acceptation](#7-critères-dacceptation)
8. [Contraintes](#8-contraintes)
9. [Risques et mitigation](#9-risques-et-mitigation)
10. [Métriques de succès](#10-métriques-de-succès)
11. [Points à trancher](#11-points-à-trancher)
12. [Axes d'amélioration](#12-axes-damélioration)
13. [Annexes](#13-annexes)

---

## 1. Contexte

### 1.1 Problème

Un utilisateur qui analyse sa trace GPX doit souvent identifier **manuellement** les **portions empruntées plusieurs fois** :

- **Aller-retour** sur un même tronçon (demi-tour, col, impasse) ;
- **Reconnaissance** d'un parcours avec passages multiples sur une section ;

Cette identification manuelle est **longue, fastidieuse et source d'erreurs**. Un aller-retour sur 5 km sur une trace de 60 km représente 10 minutes d'analyse visuelle sur un lecteur GPX classique.

### 1.2 État de l'art

| Outil          | Fonctionnalité               | Limite                                                            |
| -------------- | ---------------------------- | ----------------------------------------------------------------- |
| Strava         | Détection segments           | Nécessite un compte, pas d'export, dépend de segments pré-définis |
| Garmin Connect | Visualisation                | Pas de détection de répétition                                    |
| OpenRunner     | Édition                      | Pas de détection automatique                                      |
| **Multi-Sens** | **Détection locale, export** | **À construire**                                                  |

Aucun outil grand public ne propose la détection automatique **et** l'export des portions répétées d'une trace GPX.

### 1.3 Contexte d'usage

- L'utilisateur est **seul devant son navigateur**, après sa sortie.
- Il dispose d'un fichier `.gpx` exporté depuis son application habituelle (Garmin, OpenRunner, Komoot…).
- Il n'a **pas de compte à créer**, **pas de serveur à installer**.
- Il veut un **résultat visuel immédiat** et un **export réutilisable**.

### 1.4 Contrainte forte

**Aucune donnée ne doit quitter le poste de l'utilisateur.** Pas d'upload, pas de télémétrie, pas d'API tierce sollicitée en dehors du chargement initial des bibliothèques (MapBox, Turf, OpenRouteService.).

---

## 2. Objectifs

### 2.1 Objectif principal

Détecter et restituer visuellement les portions répétées d'une trace GPX, avec un contrôle manuel de l'utilisateur sur les cas ambigus.

### 2.2 Objectifs secondaires

- Permettre l'**ajustement des paramètres** de détection sans recharger la page.
- Offrir un **export standard (GeoJSON)** exploitable dans de futures fonctionnalités.
- Produire un export **compact** qui ne duplique pas les géométries de la trace d'origine.
- Fonctionner **hors ligne** après premier chargement (libs mises en cache par le navigateur).
- Ne **jamais bloquer** l'application : l'analyse se fait avec feedback de progression.

### 2.3 Indicateurs de succès (v1.0)

| Indicateur                                      | Cible                         |
| ----------------------------------------------- | ----------------------------- |
| Temps d'analyse pour 10 000 points              | < 1 s                         |
| Temps d'analyse pour 30 000 points              | < 5 s                         |
| Précision de détection (trace de référence)     | ≥ 90 % des portions attendues |
| Faux positifs sur trace de référence            | ≤ 10 %                        |
| Fonctionne sans réseau après chargement initial | Oui                           |
| Taille du fichier HTML                          | < 100 KB (hors libs CDN)      |
| Taille de l'export GeoJSON                      | < 10 KB pour 20 passages      |

---

## 3. Utilisateurs cibles

### 3.1 Personas

**Cycliste / VTT-Route**
- Traces plus longues (50–200 km), .
- Cherche à identifier les portions reprises (montée redescendue, circuits, passages multiples).
- Sensible au **temps d'analyse** et à la **qualité de la détection**.

**Traileur / orienteur**
- Traces en boucle, parfois avec répétitions intentionnelles (entraînement fractionné).
- Veut distinguer les **répétitions volontaires** des **erreurs**.

**Analyste SIG occasionnel**
- Souhaite exporter les portions répétées vers un SIG pour croiser avec d'autres données (foncier, topographie, occupation du sol).
- Attaché à la **précision** de l'export (géométrie, propriétés, géoréférencement).
- Souhaite un export **léger** et **joignable** à la trace d'origine.

### 3.2 Cas d'usage détaillés

**UC-1 : Reconnaissance VTT avec boucles multiples**
- On a repéré un circuit VTT de 60 km avec 3 incursions dans un même bois.
- Elle charge la trace, ajuste la tolérance à 15 m (bruit GPS en forêt).
- Elle obtient 3 segments répétés, dont deux se chevauchent légèrement.
- Elle marque le troisième comme faux positif (portion différente mais proche visuellement).
- Elle exporte les deux segments retenus.

**UC-3 : Traversée de village avec sens uniques**
- On a une trace où le centre-ville est traversé 2 fois par des rues différentes (sens uniques).
- L'analyse détecte 3 segments courts quasi adjacents.
- Il les **fusionne manuellement** en un seul segment logique.
- Il exporte.

**UC-4 : Erreur de trace**
- Une trace a été éditée avec des micros aller-Retour 
- L'analyse détecte un micro-segment répété. Normalement ces erreurs ont déjà fait l'objet d'une analyse préalable.
- L'utilisateur ajuste la **longueur minimale** à 200 m → le micro-segment disparaît.

**UC-5 : Intégration 
- On veut visualiser sur une carte la pente de la trace. Dans le cas d'un aller-retour sur un col par exemple, dans un sens la pente est pasitive (couleur Jaune, orange, rouge, noir) dans l'autre sens elle est négative (verte bleu clair bleu foncé etc). Il faut savoir dans quel sens nous sommes pour afficher le bon layout.

---

## 4. Périmètre

### 4.1 Inclus (v1.0)

- Import **fichier GPX local** (drag-drop ou sélecteur de fichier).
- Projection locale des coordonnées pour comparaisons métriques.
- Rééchantillonnage à pas fixe (paramétrable).
- Détection automatique des portions répétées (sens aller et retour).
- Restitution **carte Mapbox** + **panneau latéral** synchronisés.
- Fusion **manuelle** de segments adjacents (uniquement avec le précédent).
- Marquage **faux positif** sur un segment (exclu de l'export).
- Bouton **réinitialiser** les modifications utilisateur.
- **Export GeoJSON compact** : un Feature par passage, géométrie bornée par les 2 extrémités, métadonnées d'indices et de distances cumulées.
- Aide contextuelle sur chaque paramètre.

### 4.2 Exclu (v1.0)

- Multi-trace / comparaison entre fichiers.
- Prise en compte de l'**altitude** (`<ele>`) ou du **temps** (`<time>`).
- Comptes utilisateur, sauvegarde serveur, historique.
- Chargement depuis une URL distante.
- Édition de la trace (découpage, suppression).
- Export GPX ou CSV.
- Application mobile native.
- Recherche, filtres, tri des segments.

### 4.3 Reporté (v1.x ou v2)

Voir section [12. Axes d'amélioration](#12-axes-damélioration).

---

## 5. Fonctionnalités détaillées

### 5.1 Import de trace

**F-01 — Dépôt de fichier GPX** (déjà réalisé)

- Zone de dépôt visible dans le panneau latéral.
- Accepte uniquement les fichiers `.gpx` (extension et MIME `application/gpx+xml`).
- Support drag-drop sur la zone **et** sur toute la page (overlay global).
- Support clic → ouverture du sélecteur natif.
- Accessible au clavier (`Entrée` / `Espace`).
- Message d'erreur explicite en cas de fichier invalide ou de parsing impossible.

**F-02 — Lecture et validation** (déjà réalisé)

- Parsing XML via `DOMParser`.
- Extraction des `<trkpt>` (sinon `<rtept>` en fallback).
- Ignore les points sans latitude ou longitude valides.
- Minimum 2 points requis.
- Message d'information après chargement : nombre de points bruts.

**F-03 — Nettoyage géométrique** (déjà réalisé)

- Projection équirectangulaire locale centrée sur le premier point.
- Dédoublonnage des points à moins de 5 cm.
- Calcul de la distance curviligne cumulée.
- Affichage : nombre de points bruts, après nettoyage, distance totale.

### 5.2 Paramétrage

**F-04 — Quatre paramètres ajustables**

| Slider | Description | Aide contextuelle |
|---|---|---|
| Tolérance de superposition | Distance max pour deux points « au même endroit » | Bulle d'aide |
| Longueur minimale | Longueur min pour qu'un segment soit retenu | Bulle d'aide |
| Pas d'échantillonnage | Granularité du rééchantillonnage | Bulle d'aide |
| Fusion références proches | Distance de fusion automatique | Bulle d'aide |

**F-05 — Signature de paramètres**

- Les paramètres actifs sont mémorisés au moment de l'analyse.
- Si l'utilisateur modifie un paramètre après analyse → pastille visuelle sur le bouton « Analyser » (rappel que le résultat affiché n'est plus à jour).

### 5.3 Analyse

**F-06 — Rééchantillonnage**

- Calcul du nombre de points `n = round(total / step)`.
- Pas effectif calé sur la fin de trace.
- Garde-fou : si `total / step > 120 000`, on augmente le pas automatiquement et on prévient l'utilisateur (toast).

**F-07 — Détection des runs**

- Index spatial grille (`cell = max(tol, step × 1.5)`).
- Appariement point-à-point :
  - séparation temporelle min `max(minLen/2, step×4)` ;
  - distance ≤ `tol` ;
  - tangentes non perpendiculaires (`|cos| > 0.12`).
- Chaînage glouton en runs maximaux avec tolérance de trou (`MAXGAP = 3` points).
- Filtrage par `minLen`.

**F-08 — Assemblage en segments**

- Union-find itératif (max 5 passes) :
  - Phase A : fusion des intervalles « même passage » (chevauchement ou gap ≤ `fuse`) ;
  - test de frontière entre deux passes (inversion de cap, ré-emprunt de route) ;
  - Phase B : composantes connexes via les liens de correspondance ;
  - Phase C : fusion des références chevauchantes.
- **Passe D** — fusion intra-segment : passages contigus de même sens à ≤ 2 × `fuse` sont fusionnés (comble les trous courts : virages serrés, tunnels, pertes GPS).

**F-09 — Sens relatif**

- Échantillonnage du 1er passage (8 à 60 points).
- Recherche du point le plus proche dans le second passage (pointeur glissant).
- Comparaison d'azimuts via `turf.bearing`.
- Résultat : `+1` (aller) ou `-1` (retour).

**F-10 — Progression**

- Overlay modal avec libellé, barre de progression, sous-étape.
- Yields UI entre chaque étape pour ne pas figer le navigateur.
- Étape finale : rendu cartographique.

### 5.4 Restitution

**F-11 — Carte**

- MapBox
- Marqueurs départ (rond blanc) et arrivée (rond noir).
- Zones réutilisées avec `fitBounds` au chargement.
- Polylignes superposées :
  - **Référence** : bleu `#3564e0`, épaisse 8, opacité 0.85 (pane z-index 410) ;
  - **Aller** : orange `#ec9c11`, épaisse 6, opacité 0.95 (pane z-index 420) ;
  - **Retour** : rouge `#dd3327`, épaisse 4., opacité 0.95 (pane z-index 425).
- Popup au clic : passage n°, sens, points début/fin, distance.

**F-12 — Panneau de résultats**

- **Résumé** : nombre de segments visibles, faux positifs, fusions, km répétés, temps d'analyse, total trace, paramètres actifs.
- **Par segment** :
  - nom (`Segment N`) + badges (`fusionné`, `FP`) ;
  - nombre de passages, longueur de référence ;
  - mini-ribbon : position de chaque passage sur la trace totale ;
  - liste des passages : point début-fin, km début-fin, tag de sens.
- **Actions** : exporter GeoJSON, réinitialiser modifs (si applicable).

**F-13 — Synchronisation liste ↔ carte**

- Survol d'un passage → épaississement de la polyligne correspondante.
- Clic sur un segment → `fitBounds` sur son étendue + flash visuel des polylignes.
- Après fusion ou FP → **re-rendu complet de la carte** depuis l'état `working`.

### 5.5 Manipulations utilisateur

**F-14 — Fusion manuelle de segments**

- Bouton **« ↩ Fusionner S{n} »** sur chaque segment sauf le premier.
- Fusionne le segment courant avec le précédent.
- Règles :
  - les passages sont triés par km début ;
  - deux passages consécutifs fusionnent seulement s'ils vont **dans le même sens** (frontière aller/retour respectée) ;
  - tolérance généreuse : **1 km** (`MERGE_MANUAL_TOL_KM`), indépendante du paramètre `fuse` ;
  - le premier passage résultant devient référence, les anciennes références non-tête basculent en aller.
- Badge **fusionné** affiché.
- Compteur `mergedCount` cumulatif.

**F-15 — Marquage faux positif**

- Bouton **« Faux positif »** / **« Retirer FP »**.
- Effets :
  - carte de segment grisée (opacité 0.5), titre barré ;
  - badge **FP** ;
  - polylignes sur la carte passées en opacité 0.15 ;
  - segment **exclu de l'export GeoJSON** ;
  - segment **non compté** dans les km répétés.

**F-16 — Réinitialisation**

- Bouton **« Réinitialiser modifs »** visible dès qu'une modification existe.
- Restaure les groupes depuis `state.analysis.segments`.
- Re-rend la carte en cohérence.

### 5.6 Export

**F-17 — Export GeoJSON compact**

**Principe** : l'export ne duplique **pas** les géométries de la trace d'origine. Chaque passage est représenté par ses **deux extrémités** (point d'entrée, point de sortie) et un ensemble de métadonnées permettant de le relier à la trace GPX.

**Structure** : `FeatureCollection` avec :

- **`properties` globaux** :
  - `source` : nom du fichier GPX ;
  - `date` : ISO 8601 ;
  - `trace.point_count` : nombre de points dans la trace d'origine ;
  - `trace.length_km` : longueur totale de la trace ;
  - `parametres` : tolérance, longueur min, pas, fusion, pas plafonné ;
  - `ajustements` : nombre de fusions manuelles, nombre de faux positifs exclus ;
  - `note` : explication du format, rappel que la géométrie est bornée aux 2 extrémités.

- **Une `Feature` par passage** :
  - **`properties`** :
    - `segment` : numéro du segment (1-based) ;
    - `passage` : numéro du passage dans le segment (1-based) ;
    - `sens` : `reference` | `aller` | `retour` ;
    - `point_entree` : index du point d'entrée dans la trace GPX d'origine (1-based) ;
    - `point_sortie` : index du point de sortie ;
    - `km_entree` : distance cumulée depuis le km 0 de la trace (le long de la trace, pas distance euclidienne) ;
    - `km_sortie` : idem pour la sortie ;
    - `longueur_km` : différence entre km_sortie et km_entree ;
    - `fusionne` : booléen, marque un passage ayant subi une fusion manuelle.
  - **`geometry`** : `LineString` à **exactement 2 coordonnées** `[[lon_entry, lat_entry], [lon_exit, lat_exit]]`.

**Nom du fichier** : `<nom_gpx>_segments.geojson`.

**Usage côté SIG** : la portion complète d'un passage se reconstitue en joignant `point_entree` / `point_sortie` avec la trace d'origine (voir annexe 13.6).

---

## 6. Paramètres utilisateur

### 6.1 Tolérance de superposition

| Propriété | Valeur |
|---|---|
| **Unité** | mètres |
| **Min** | 3 |
| **Défaut** | 10 |
| **Max** | 30 |
| **Pas** | 1 |
| **Description** | Distance maximale entre deux points pour qu'ils soient considérés comme « le même lieu géographique ». |
| **Impact bas** | < 5 m → rate les portions répétées à cause du bruit GPS (5–15 m en zone ouverte, plus en forêt). |
| **Impact haut** | > 25 m → mélange des voies parallèles distinctes (route et sentier adjacents, deux rues d'un même carrefour). |
| **Conseil** | 8–15 m selon la qualité de la trace. |
| **Utilisé dans** | `findRuns` (rayon d'appariement), `boundaryBetweenPasses` (gate de proximité), `relativeDirection` (marge de recherche). |

### 6.2 Longueur minimale

| Propriété | Valeur |
|---|---|
| **Unité** | mètres |
| **Min** | 20 |
| **Défaut** | 100 |
| **Max** | 500 |
| **Pas** | 10 |
| **Description** | Longueur minimale d'un segment pour être signalé dans les résultats. |
| **Impact bas** | < 30 m → beaucoup de bruit (demi-tour parking, passage sur pont, arrêt technique). |
| **Impact haut** | > 300 m → peut rater des portions réellement répétées mais courtes (impasse, reconnaissance brève). |
| **Conseil** | 50–200 m selon le type de sortie. |
| **Utilisé dans** | `findRuns` (filtrage final), séparation temporelle minimale `max(minLen/2, step×4)`. |

### 6.3 Pas d'échantillonnage

| Propriété | Valeur |
|---|---|
| **Unité** | mètres |
| **Min** | 2 |
| **Défaut** | 4 |
| **Max** | 10 |
| **Pas** | 1 |
| **Description** | Granularité à laquelle la trace est rééchantillonnée avant détection. |
| **Impact bas** | < 3 m → détection plus précise, mais coût CPU/mémoire croissant (grille spatiale plus dense). |
| **Impact haut** | > 8 m → plus rapide, mais peut manquer un chevauchement court ou imprécis (virages serrés). |
| **Conseil** | 4 m — bon compromis. |
| **Garde-fou** | Plafond automatique à 120 000 points ; si dépassé, le pas est augmenté et l'utilisateur est averti. |
| **Utilisé dans** | `resampleGeo`, `findRuns` (calcul de `minSep`, `cell`), `buildSegments` (conversion `fuseDist → fusePts`). |

### 6.4 Fusion références proches

| Propriété | Valeur |
|---|---|
| **Unité** | mètres |
| **Min** | 0 (off) |
| **Défaut** | 100 |
| **Max** | 500 |
| **Pas** | 5 |
| **Description** | Distance en deçà de laquelle deux passages dont les intervalles temporels sont adjacents sont considérés comme un seul passage logique. |
| **Impact bas** | < 20 m → chaque micro-coupure (pause, arrêt photo, redémarrage GPS) fragmente les segments. |
| **Impact haut** | > 300 m → risque de coller des passages réellement différents (deux portions séparées sur le terrain). |
| **Conseil** | 50–150 m selon la qualité de la trace et la densité d'arrêts. |
| **Effet double** | Pilote aussi la **fusion intra-segment** (Passe D) avec un facteur ×2 : les passages contigus de même sens séparés de moins de 2 × `fuse` sont fusionnés automatiquement. |
| **Valeur 0** | Désactive toute fusion automatique et intra-segment. |
| **Utilisé dans** | `buildSegments` (`samePassage`, `fusePts`, `maxGapPts`). |

### 6.5 Paramètres internes (non exposés)

| Nom | Valeur | Rôle |
|---|---|---|
| `MAXGAP` | 3 | Trous tolérés dans le chaînage des runs |
| `PERPENDICULAR_COS` | 0.12 | Seuil de rejet des croisements perpendiculaires |
| `MAX_ITERATIONS` | 5 | Nombre max d'itérations de l'assemblage |
| `MERGE_WITHIN_SEGMENT_FACTOR` | 2 | Facteur de la fusion intra-segment |
| `MERGE_MANUAL_TOL_KM` | 1.0 | Tolérance de la fusion manuelle (km) |
| `MAX_SAMPLES` | 120 000 | Plafond de points rééchantillonnés |
| `FP_OPACITY` | 0.15 | Opacité des polylignes d'un segment FP |
| `DUP_THRESHOLD2` | 0.0025 | Seuil de dédoublonnage (0.05 m)² |

---

## 7. Critères d'acceptation

### 7.1 Fonctionnels

| ID | Critère |
|---|---|
| **CA-01** | L'import d'un GPX valide affiche la trace sur la carte en < 1 s (trace de 5 000 points). |
| **CA-02** | Un fichier non-GPX déclenche un toast d'erreur, aucun état modifié. |
| **CA-03** | Un GPX avec moins de 2 points déclenche une erreur explicite. |
| **CA-04** | Les 4 sliders sont réactifs et affichent la valeur courante. |
| **CA-05** | Modifier un slider après analyse fait apparaître la pastille sur le bouton « Analyser ». |
| **CA-06** | Une trace d'aller-retour simple (10 km, demi-tour) produit 1 segment avec 1 référence + 1 retour. |
| **CA-07** | Les couleurs correspondent au sens : bleu référence, orange aller, rouge retour. |
| **CA-08** | Le survol d'un passage épaissit la polyligne correspondante. |
| **CA-09** | Le clic sur un segment zoome la carte sur son étendue. |
| **CA-10** | Fusionner S2 avec S1 met à jour la carte en < 200 ms. |
| **CA-11** | Un segment marqué FP est grisé dans la liste et exclu du GeoJSON. |
| **CA-12** | Le bouton « Réinitialiser modifs » restaure l'état initial. |
| **CA-13** | L'export GeoJSON contient une Feature par passage non-FP. |
| **CA-14** | Le nom du fichier exporté suit le pattern `<nom_gpx>_segments.geojson`. |
| **CA-15** | Chaque Feature contient un `LineString` à **exactement 2 coordonnées**. |
| **CA-16** | Les propriétés `point_entree` et `point_sortie` correspondent aux index réels dans la trace d'origine (1-based). |
| **CA-17** | `km_entree` et `km_sortie` sont calculés par distance cumulée le long de la trace, pas en distance euclidienne. |
| **CA-18** | Le bloc `properties.trace` contient `point_count` et `length_km`. |
| **CA-19** | La Feature porte un champ `fusionne` à `true` pour un segment ayant subi une fusion manuelle. |

### 7.2 Non-fonctionnels

| ID           | Critère                                                                                 |
| ------------ | --------------------------------------------------------------------------------------- |
| **CA-NF-01** | Le fichier `.html` complet fait < 100 KB hors libs CDN.                                 |
| **CA-NF-02** | L'application fonctionne après le premier chargement, même sans réseau (libs en cache). |
| **CA-NF-03** | Aucune requête réseau n'est émise après l'import d'un GPX.                              |
| **CA-NF-04** | Analyse de 10 000 points en < 1 s sur un laptop récent.                                 |
| **CA-NF-05** | Analyse de 30 000 points en < 5 s.                                                      |
| **CA-NF-06** | Le navigateur ne se fige pas pendant l'analyse (feedback de progression actif).         |
| **CA-NF-07** | Compatible Chrome, Firefox, Safari (versions ES2020+).                                  |
| **CA-NF-08** | Responsive : bascule en colonne sous 860 px de large.                                   |
| **CA-NF-09** | Export GeoJSON < 10 KB pour une trace comportant 20 passages.                           |

---

## 8. Contraintes

### 8.1 Techniques


- MapBox
- Vuetify V2
- Node js
- Tauri

### 8.2 Réglementaires

- **RGPD** : aucune donnée personnelle traitée, aucune donnée transmise.
- **Licences** : MapBox

### 8.3 Budgétaires / délais

- Projet personnel, pas de budget externe.
- Pas de deadline imposée.

---

## 9. Risques et mitigation

| Risque | Impact | Probabilité | Mitigation |
|---|---|---|---|
| Bruit GPS important → faux positifs | Moyen | Élevée | Paramètre `tol` ajustable, marquage FP manuel |
| Trace très longue → lenteur | Moyen | Moyenne | Plafond 120k points, step ajusté automatiquement |
| Fusion excessive → perte de granularité | Moyen | Moyenne | Slider `fuse` réglable, fusion manuelle uniquement sur segments adjacents |
| Sens mal détecté (virages en épingle) | Faible | Moyenne | Échantillonnage multiple de `relativeDirection`, score cumulé sur cos(Δ) |
| Traces avec altitude → non exploité | Faible | Élevée | Documenté hors scope v1.0 |
| Fichiers GPX exotiques (namespaces, extensions) | Faible | Faible | Parsing `getElementsByTagNameNS('*', …)` |
| Utilisateur perdu face aux paramètres | Moyen | Moyenne | Bulles d'aide contextuelles, valeurs par défaut conservatrices |
| Régression lors d'une fusion | Moyen | Faible | Bouton « Réinitialiser modifs » disponible |
| Export GeoJSON trop volumineux | Faible | Faible | Géométrie bornée à 2 points (v1.0) |
| Confusion sur l'interprétation des 2 points d'un Feature | Moyen | Moyenne | Champ `note` explicatif dans les properties |

---

## 10. Métriques de succès

### 10.1 Métriques quantitatives

- **Temps d'analyse** : médiane sur 10 traces de référence.
- **Précision** : % de portions répétées correctement identifiées sur trace annotée manuellement.
- **Rappel** : % de portions attendues présentes dans le résultat.
- **Faux positifs** : nombre moyen par trace de 30 km.
- **Taille export** : moyenne en KB pour une trace avec 5 segments × 2 passages.

### 10.2 Métriques qualitatives

- L'utilisateur peut-il comprendre les paramètres sans documentation externe ? (test utilisateur)
- L'export GeoJSON est-il exploitable directement dans QGIS ? (test sur 3 SIG)
- Le workflow fusion / FP est-il intuitif ? (test utilisateur)
- L'export compact est-il compris comme tel ? (test utilisateur sur la note)

### 10.3 Cibles

| Métrique | Cible v1.0 | Cible v1.1 |
|---|---|---|
| Précision | ≥ 90 % | ≥ 95 % |
| Rappel | ≥ 85 % | ≥ 90 % |
| Faux positifs / 30 km | ≤ 2 | ≤ 1 |
| Temps analyse 10k pts | < 5 s | < 3 s |
| Taille export 20 passages | < 10 KB | < 8 KB |

---

## 11. Points à trancher

Questions ouvertes nécessitant une décision produit ou technique avant ou après v1.0.

### 11.1 Détection

**T1 — Prise en compte de l'altitude et du temps**
- La v1.0 ignore `<ele>` et `<time>`.
- **Question** : en v1.1, distingue-t-on un aller-retour sur route plate d'un aller-retour sur un col par le profil altimétrique ? Le temps (durée du passage) peut-il servir de score de confiance ?
- **Impact** : complexité +, précision +.

**T2 — Détection d'auto-intersections (croisements en X)**
- Actuellement explicitement rejetés (`|cos| ≤ 0.12`).
- **Question** : utile pour les itinéraires urbains en damier (deux rues croisées, même carrefour traversé 4 fois) ?
- **Impact** : risque de faux positifs si mal réglé.

**T3 — Score de confiance par segment**
- Aucun indicateur de qualité actuellement.
- **Question** : afficher un score (nombre de points appariés / longueur, cohérence de cap) pour hiérarchiser les résultats ?
- **Impact** : UI plus riche, tri possible.

**T4 — Mode boucle stricte**
- Cas marginal : trace fermée avec portions répétées entre début et fin.
- **Question** : autoriser un mode « enroulé » où le point 0 et le point N sont considérés adjacents ?
- **Impact** : cas très rare, complexité moyenne.

### 11.2 Manipulations utilisateur

**T5 — Portée du UNDO**
- Voir axe d'amélioration A-01 ci-dessous.
- **Question** : simple undo (dernière action) ou historique complet ?
- **Impact** : complexité de l'état à maintenir.

**T6 — Fusion avec un segment non adjacent**
- Actuellement : seulement le précédent.
- **Question** : autoriser la fusion avec n'importe quel segment (drag-and-drop dans la liste) ?
- **Impact** : UX à repenser, risque d'erreurs.

**T7 — Annotations utilisateur persistées**
- Marquer un segment comme « volontaire », « à corriger », « à explorer ».
- **Question** : stockage localStorage ou export dans le GeoJSON ?
- **Impact** : structure de données enrichie.

### 11.3 UX

**T8 — Filtre par segment**
- Masquer/afficher un segment individuellement (yeux dans la liste).
- **Question** : distinct du FP ? Le masquage est-il temporaire (session) ou persistant (export) ?

**T9 — Comparaison multi-traces**
- Charger 2 GPX (référence + nouvelle sortie) et détecter les recoupements.
- **Question** : v1.1 ou v2 ? Change fondamentalement l'architecture (state multi-fichiers).

**T10 — Export supplémentaire (GPX, CSV)**
- GPX : segments isolés pour Strava/Garmin.
- CSV : synthèse tabulaire (segments × passages × km).
- **Question** : priorité v1.1 ou v2 ?

### 11.4 Export

**T14 — Choix du mode d'export GeoJSON**
- v1.0 : export compact borné (2 points par Feature).
- **Question** : faut-il ajouter une **option** d'export « complet » (tous les points de chaque passage) pour les utilisateurs qui veulent un fichier autonome, au prix d'une taille plus élevée ?
- **Impact** : double code d'export, case à cocher dans le résumé.
- **Recommandation** : garder le mode compact par défaut, ajouter un second bouton « Exporter complet » si demande utilisateur.

**T15 — Structure alternative : 2 Features par passage**
- v1.0 : 1 Feature par passage avec LineString à 2 points.
- **Question** : préférer 2 Features Point (entrée + sortie) avec un champ `role: "entree"|"sortie"` ?
- **Impact** : plus pratique pour des traitements par point, moins pour la visualisation de la portion.
- **Recommandation** : conserver le LineString (1 Feature = 1 passage), plus lisible.

**T16 — Export des bornes seules sans géométrie**
- Variante possible : un Feature de type `Point` par borne, sans LineString.
- **Question** : utile pour les utilisateurs qui veulent juste les coordonnées d'entrée/sortie ?
- **Recommandation** : hors scope, les coordonnées sont déjà dans la géométrie LineString.

### 11.5 Technique

**T11 — Web Worker pour l'analyse**
- Actuellement yields UI.
- **Question** : passer à un Web Worker pour supprimer tout risque de figement sur très longues traces ?
- **Impact** : restructuration, mais robustesse accrue.

**T12 — Tests automatisés**
- Aucun test actuellement.
- **Question** : introduire Vitest sur les fonctions pures (`findRuns`, `buildSegments`, `relativeDirection`, `exportGeoJSON`) ?
- **Impact** : nécessite sortir le code du `<script>` inline ou utiliser un runner dans le navigateur.

**T13 — Types JSDoc**
- Aucune annotation de type.
- **Question** : documenter `Run`, `Passage`, `Segment`, `ExportedFeature` via JSDoc pour autocomplétion IDE ?
- **Impact** : faible coût, gain de lisibilité important.

---

## 12. Axes d'amélioration

### 12.1 Priorité haute

**A-01 — UNDO des manipulations utilisateur**
- **Description** : pouvoir annuler la dernière action (fusion, FP) voire plusieurs.
- **Implémentation proposée** :
  - pile d'états `undoStack` contenant des snapshots de `state.working` (peu volumineux : quelques centaines d'objets) ;
  - limite de profondeur : 20 entrées max ;
  - bouton **« Annuler »** dans le résumé, désactivé quand la pile est vide ;
  - raccourci clavier `Ctrl+Z` / `Cmd+Z` ;
  - réinitialisation de la pile après `runAnalysis()`.
- **Points à trancher** : T5 (undo simple ou historique complet).
- **Effort** : faible (~50 lignes).

**A-02 — Notification explicite quand `step` est plafonné**
- Déjà en place (toast + mention dans le résumé).
- **Amélioration** : proposer un lien « augmenter le pas manuellement » vers le slider.

**A-03 — Web Worker pour l'analyse**
- Éviter tout figement sur très longues traces.
- **Effort** : moyen (restructuration).

**A-04 — Option d'export GeoJSON complet**
- Second bouton « Exporter complet (avec tous les points) » à côté du bouton compact.
- **Effort** : faible (~30 lignes).
- **Décision liée** : T14.

### 12.2 Priorité moyenne

**A-05 — Score de confiance par segment**
- Basé sur : nombre de points appariés / longueur attendue, cohérence de cap, régularité de l'écart.
- Affichage : badge ou jauge visuelle.
- **Effort** : moyen.

**A-06 — Export GPX et CSV**
- GPX : portions répétées isolées, prêtes à charger dans Strava.
- CSV : synthèse tabulaire.
- **Effort** : faible.

**A-07 — Filtre visuel par segment**
- Bouton œil sur chaque segment (masqué côté carte uniquement).
- **Effort** : faible.

**A-08 — Légende cliquable**
- Isoler référence / aller / retour via la légende.
- **Effort** : faible.

**A-09 — Annotations utilisateur**
- Marquer : « volontaire », « à corriger », « à explorer ».
- Stockage localStorage (persistance entre sessions).
- **Effort** : moyen.

**A-10 — Prise en compte de l'altitude**
- Profil altimétrique par passage.
- Détection de répétitions en montée vs descente.
- **Effort** : moyen.

**A-11 — Indicateur visuel du sens sur la carte**
- Flèches le long des polylignes (via SVG markers custom).
- **Effort** : moyen.

**A-12 — Barre de progression plus détaillée**
- Afficher des estimations temporelles (« ~3 s restantes »).
- **Effort** : faible.

**A-13 — URL partageable**
- Encoder les paramètres + hash de trace en query string.
- Nécessite un backend ou localStorage chiffré.
- **Effort** : élevé.

### 12.3 Priorité basse

**A-14 — Mode clair**
- Media query `prefers-color-scheme` + bascule manuelle.
- **Effort** : faible.

**A-15 — Tri des segments**
- Par longueur, nombre de passages, position.
- **Effort** : faible.

**A-16 — Recherche / filtre textuel**
- Sur nom de segment, plage km.
- **Effort** : faible.

**A-17 — Comparaison multi-traces**
- Charger un GPX « référentiel » + plusieurs sorties.
- Détecter les portions communes.
- **Effort** : élevé (architecture à revoir).

**A-18 — Détection d'auto-intersections (croisements en X)**
- Relâcher le filtre `PERPENDICULAR_COS` sous condition.
- **Effort** : moyen.

**A-19 — Tests automatisés**
- Vitest sur fonctions pures.
- Fixtures : boucle, aller-retour, damier urbain, trace bruitée.
- **Effort** : moyen.

**A-20 — Découpage modulaire**
- Sortir les fonctions pures dans des modules ES.
- Permet tests + réutilisation.
- **Effort** : moyen.

**A-21 — Types JSDoc**
- Annoter `Run`, `Passage`, `Segment`, `ExportedFeature`.
- **Effort** : faible, gain de lisibilité élevé.

**A-22 — Rendu WebGL**
- Si > 1M points dessinés (traces multi-jours).
- `Leaflet.glify` ou `deck.gl`.
- **Effort** : élevé.

---

## 13. Annexes

### 13.1 Glossaire

| Terme | Définition |
|---|---|
| **Trace** | Suite ordonnée de points GPS formant un parcours. |
| **Trace de référence** | Trace GPX de test servant à valider l'application. |
| **Run** | Paire d'intervalles appariés point-à-point sur la trace (segment de trace similaire ré-emprunté). |
| **Passage** | Portion de trace correspondant à un emprunt du segment, avec un sens. |
| **Segment** | Ensemble logique de plusieurs passages (référence + allers/retours). |
| **Référence** | Premier passage d'un segment (affiché en bleu). |
| **Aller** | Passage dans le même sens que la référence (affiché en orange). |
| **Retour** | Passage en sens inverse (affiché en rouge). |
| **Faux positif** | Segment détecté à tort, exclu des résultats exportés. |
| **Fusion** | Action manuelle d'union de deux segments adjacents. |
| **Fusion intra-segment** | Fusion automatique de passages contigus de même sens (Passe D). |
| **Export compact** | Format GeoJSON où chaque passage est borné par 2 points (entrée, sortie). |
| **km cumulé** | Distance le long de la trace depuis le point de départ (pas distance euclidienne). |

### 13.2 Références

- **GPX 1.1** : https://www.topografix.com/GPX/1/1/
- **Mapbox** : https://leafletjs.com/
- **Turf.js** : https://turfjs.org/
- **GeoJSON (RFC 7946)** : https://datatracker.ietf.org/doc/html/rfc7946
- **ODbL OSM** : https://www.openstreetmap.org/copyright

### 13.3 Fichiers de référence pour tests

| Fichier                    | Description                          | Segments attendus        |
| -------------------------- | ------------------------------------ | ------------------------ |
| `poggio.gpx`               | plusieurs segments                   | 0                        |
| `CalpePhoto.gpx`           | 10 km dont 5 km en aller-retour      | 1 (réf + retour)         |
| `reconnaissance_vtt.gpx`   | 60 km, 3 incursions dans même bois   | 3 (réf + allers/retours) |
| `village_sens_uniques.gpx` | Traversée multiple d'un centre-ville | 1 à 3 (selon tolérance)  |
| `trace_bruitee.gpx`        | Trace GPS de mauvaise qualité        | 1 (segments erratiques)  |

### 13.4 Format de l'export GeoJSON compact

```json
{
  "type": "FeatureCollection",
  "properties": {
    "source": "menton_poggio.gpx",
    "date": "2026-09-16T10:30:00.000Z",
    "trace": {
      "point_count": 2712,
      "length_km": 78.421
    },
    "parametres": {
      "tolerance_m": 10,
      "longueur_min_m": 100,
      "pas_echantillonnage_m": 4,
      "fusion_references_m": 100,
      "pas_plafonne": false
    },
    "ajustements": {
      "fusions_manuelles": 1,
      "faux_positifs_exclus": 0
    },
    "note": "Chaque Feature représente un passage. La géométrie LineString ne contient que les 2 points bornes (entrée, sortie). Pour reconstituer la portion de trace, joindre point_entree / point_sortie avec la trace d'origine."
  },
  "features": [
    {
      "type": "Feature",
      "properties": {
        "segment": 1,
        "passage": 1,
        "sens": "reference",
        "point_entree": 240,
        "point_sortie": 842,
        "km_entree": 7.62,
        "km_sortie": 23.06,
        "longueur_km": 15.44,
        "fusionne": true
      },
      "geometry": {
        "type": "LineString",
        "coordinates": [
          [7.49912, 43.77584],
          [7.49278, 43.79012]
        ]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "segment": 1,
        "passage": 2,
        "sens": "retour",
        "point_entree": 1824,
        "point_sortie": 2464,
        "km_entree": 51.37,
        "km_sortie": 67.05,
        "longueur_km": 15.68,
        "fusionne": true
      },
      "geometry": {
        "type": "LineString",
        "coordinates": [
          [7.53124, 43.80112],
          [7.49912, 43.77584]
        ]
      }
    }
  ]
}
```

### 13.5 Palette de couleurs

| Rôle | Variable CSS | Valeur |
|---|---|---|
| Fond principal | `--bg` | `#0f1114` |
| Panneau | `--panel` | `#14171c` |
| Texte principal | `--tx` | `#e8ebf0` |
| Référence | `--c-ref` | `#3564e0` |
| Aller | `--c-all` | `#ec9c11` |
| Retour | `--c-ret` | `#dd3327` |
| Succès | `--ok` | `#3ecf8e` |

### 13.6 Reconstitution de la portion complète côté SIG

L'export compact fournit les bornes de chaque passage. Pour tracer la portion complète d'un passage dans QGIS, uMap ou tout autre outil :

**Jointure attributaire** :

```sql
-- Table trace_gpx : les points du GPX d'origine avec leur ordre (id 1..N)
-- Table passages  : les Features de l'export
SELECT t.geometry, p.segment, p.passage, p.sens
FROM trace_gpx t
JOIN passages p
  ON t.id BETWEEN p.point_entree AND p.point_sortie
ORDER BY p.segment, p.passage, t.id;
```

**Ou, plus directement**, charger le GPX d'origine dans QGIS et créer un filtre par plage d'index :

```python
# QGIS Field Calculator ou expression filter
"id" >= point_entree AND "id" <= point_sortie
```

**Sens inverse** : pour un `retour`, l'ordre des points doit être **décroissant** (`t.id DESC`).

### 13.7 Historique des décisions

| Date | Décision | Motif |
|---|---|---|
| 2026-09 | Correction signature `buildSegments(runs, R, tol, fuse, minLen)` | Bug détecté en revue |
| 2026-09 | Passage de `tol` en paramètre à `relativeDirection` | Bug : lecture de `state.analysis.tol` non défini |
| 2026-09 | Ajout de la Passe D (fusion intra-segment) | Trous de détection courts non comblés |
| 2026-09 | Fusion manuelle respectant le sens des passages | Évite de mélanger allers et retours |
| 2026-09 | Tolérance fusion manuelle à 1 km (indépendante de `fuse`) | Décision explicite de l'utilisateur |
| 2026-09 | Re-rendu carte à chaque modification utilisateur | Synchronisation liste ↔ carte |
| 2026-09 | Export GeoJSON **compact** (2 points par Feature) | Éviter la duplication des géométries de la trace |
| 2026-09 | Ajout de `point_entree` / `point_sortie` dans les propriétés | Permettre la jointure avec la trace d'origine |
| 2026-09 | Bloc `trace` (point_count, length_km) dans les properties globaux | Contexte pour l'utilisateur SIG |

---

**Fin du document.**