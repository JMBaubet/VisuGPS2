# PRD — Analyseur de profil altimétrique GPX

**Nom de code** : GPX Climb Analyzer
**Version** : 2.3
**Date** : 2026-09-21
**Statut** : Spécification produit — application web monopage (SPA)
**Auteur** : [à compléter]
**Destinataire** : développeur front-end, product owner

---

## 1. Contexte et objectif

### 1.1 Contexte

Les cyclistes, randonneurs et traileurs utilisent des traces GPX pour préparer ou analyser leurs sorties. Les outils existants (Strava, Garmin Connect, Openrunner) détectent automatiquement les montées mais imposent leurs propres critères (souvent opaques) et ne permettent pas d'ajuster manuellement les bornes détectées de façon intuitive.

### 1.2 Objectif

Fournir un outil web autonome permettant :

- de charger une trace GPX (drop-zone persistante),
- d'afficher un profil altimétrique détaillé,
- de détecter automatiquement les montées significatives selon des critères configurables,
- **d'ajuster manuellement les bornes de chaque montée par drag & drop direct sur le profil** (v2),
- **d'insérer de nouvelles montées à n'importe quel endroit de la trace** (v2.2),
- de segmenter le parcours par catégorie de pente avec un code couleur explicite,
- de garantir la cohérence visuelle et numérique de toutes les statistiques affichées.

### 1.3 Proposition de valeur

| Axe | Bénéfice utilisateur |
|---|---|
| Transparence | Tous les critères de détection sont explicites et ajustables en direct |
| **Édition naturelle** | **Drag & drop des marqueurs sur le graphique** |
| **Insertion contextuelle** | **Nouvelle montée où on veut, quand on veut** |
| Souplesse | Chaque montée reste éditable manuellement (drag ou km) |
| Confidentialité | 100 % côté navigateur, aucun envoi sur un serveur |
| Richesse | Analyse multi-niveaux : segmentation par pente + montées significatives |
| Cohérence | Anti-chevauchement, anti-zones-blanches, catégorisation par fenêtre glissante |

---

## 2. Utilisateurs cibles

| Profil | Usage principal | Attentes |
|---|---|---|
| Cycliste route / gravel | Analyser une étape avant de la rouler | Identifier les cols, D+, pente moyenne, ajuster les bornes au vrai sommet |
| Randonneur / traileur | Évaluer la difficulté d'un parcours | Voir les portions raides, D+ cumulé |
| Organisateur d'événement | Vérifier un tracé, catégoriser les difficultés | Ajouter des montées non détectées automatiquement |
| Data analyst / développeur | Explorer et valider les données GPX | Paramétrer finement la détection (dont garde-fou hybride) |

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
| **F-07** | **Drop-zone persistante après chargement (mode compact v2.3)** | **Must** |

### 3.2 Prétraitement du signal d'altitude

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-10 | Calcul de la distance cumulée (formule de Haversine) | Must |
| F-11 | Lissage de l'altitude par moyenne mobile (paramétrable) | Must |
| F-12 | Calcul de la pente par différence centrée | Must |
| F-13 | Double lissage : altitude → pente → lissage pente | Must |
| F-14 | Lissage « ancré » qui force le passage par les extrema détectés | Must |
| F-15 | Pente de catégorisation calculée par fenêtre glissante (500 m par défaut) | Must |

### 3.3 Détection automatique des montées

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-20 | Détection par hystérésis sur la pente lissée (seuil paramétrable) | Must |
| F-21 | Identification des candidats « montée / non-montée » | Must |
| F-22 | Sous-segment optimal (findBestSubClimb) pour validation intermédiaire | Must |
| F-23 | Extension des bornes aux extrema locaux par argmin/argmax | Must |
| F-24 | Borne basse naturelle : ne pas empiéter sur la montée précédente | Must |
| **F-25** | **Garde-fou hybride de refinage `clamp(pct × longueur, 1500 m, 8000 m)` (v2)** | **Must** |
| F-26 | Double validation : avant et après ancrage | Must |
| **F-27** | **Transparence : affichage de la valeur effective du garde-fou par montée (v2)** | **Must** |

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

### 3.5 Édition manuelle des montées

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-30 | Tableau des montées avec champs éditables (km vallée, km sommet) | Must |
| F-31 | Recalcul en direct des statistiques (longueur, D+, pente) | Must |
| F-32 | Bouton « Restaurer la valeur auto » par ligne (`↺`) | Must |
| F-33 | Bouton « Supprimer » par ligne (`✕`) | Must |
| F-34 | **Insertion contextuelle entre deux montées / avant #1 / après #N (v2.2)** | **Must** |
| F-35 | Bouton « Recharger depuis la détection auto » | Must |
| F-36 | Indicateur visuel « modifié » sur les lignes éditées | Should |
| F-37 | Synchronisation intelligente : les paramètres modifiés n'écrasent pas les éditions manuelles | Must |
| F-38 | **Anti-chevauchement** : contrainte automatique sur les bornes | Must |
| F-39 | Signal visuel ⚠ + fond orange sur les lignes contraintes | Should |
| **F-40** | **Édition par drag & drop direct sur le graphique (v0.5)** | **Must** |
| **F-41** | **Sélection par radio dans le tableau (mono-sélection)** | **Must** |
| **F-42** | **Désélection (reclic radio + touche Échap) (v2.3)** | **Must** |
| **F-43** | **Contraintes anti-chevauchement temps réel pendant le drag** | **Must** |
| **F-44** | **Retour visuel temps réel (métriques + canvas) pendant le drag** | **Must** |
| **F-45** | **Édition clavier (flèches + Shift + Alt)** | **Should** |

### 3.6 Segmentation par catégorie de pente

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-50 | Classification en 8 catégories (voir tableau 3.6.1) | Must |
| **F-51** | **Fusion des segments < 250 m (paramétrable) dans un voisin (v2.1)** | **Must** |
| F-52 | **Segments contigus** : partage du point frontière, aucune zone blanche | Must |
| F-53 | Bandes colorées en fond des graphiques | Must |
| F-54 | Bande de segmentation cliquable (zoom sur segment) | Should |
| F-55 | Tableau statistiques par catégorie | Should |
| F-56 | Catégorisation par pente moyenne sur fenêtre glissante | Must |

#### 3.6.1 Grille des catégories

| Couleur | Libellé | Plage de pente |
|---|---|---|
| `#1e40af` Bleu foncé | Descente raide | p < −8 % |
| `#3b82f6` Bleu | Descente | −8 % ≤ p < −3 % |
| `#93c5fd` Bleu clair | Faux plat descendant | −3 % ≤ p < −1 % |
| `#22c55e` Vert | Plat | −1 % ≤ p ≤ +1 % |
| `#fde68a` Jaune | Faux plat montant | +1 % < p ≤ +3 % |
| `#f97316` Orange | Montée | +3 % < p ≤ +7 % |
| `#dc2626` Rouge | Montée raide | +7 % < p ≤ +10 % |
| `#000000` Noir | Montée très raide | p > +10 % |

### 3.7 Visualisation

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-60 | Profil altimétrique : altitude brute + altitude lissée ancrée | Must |
| F-61 | Graphique de pente : pente brute + pente lissée | Must |
| F-62 | Zoom molette synchronisé entre les deux graphiques | Must |
| F-63 | Déplacement clic-glisser synchronisé | Must |
| F-64 | Double-clic pour réinitialiser le zoom | Should |
| F-65 | Lignes verticales : vert = vallée, rouge pointillé = sommet | Must |
| F-66 | Tooltip avec km et valeur au survol | Must |
| F-67 | Sous-échantillonnage automatique (> 4000 points) | Must |
| F-68 | Légende explicite sous la bande de segmentation | Must |
| **F-69** | **Toggle « Mettre en avant l'altitude brute » avec états distincts (v2.3)** | **Must** |
| **F-70** | **Tooltip masqué pendant le drag des poignées (v2.1)** | **Must** |
| **F-71** | **Graphe Pente repliable (replié par défaut) (v2.1)** | **Should** |
| **F-72** | **Bande de surbrillance jaune sur la montée sélectionnée** | **Must** |
| **F-73** | **Poignées triangulaires draggables (vert/rouge) sur le graphique** | **Must** |

### 3.8 Statistiques globales

| ID | Fonctionnalité | Priorité |
|---|---|---|
| F-80 | Distance totale | Must |
| F-81 | D+ brut (calculé sur altitude brute) | Must |
| F-82 | D− brut | Must |
| F-83 | Altitude min / max | Must |
| F-84 | Nombre de montées détectées | Must |
| F-85 | D+ cumulé des montées validées | Must |
| F-86 | Distance cumulée des montées validées | Must |
| F-87 | Nombre de segments de catégorie | Should |

---

## 4. Paramètres utilisateur

Tous les paramètres sont modifiables en temps réel. Chaque modification déclenche un recalcul automatique.

| Paramètre | Plage | Défaut | Impact |
|---|---|---|---|
| Lissage altitude | 3–∞ pts | 50 | Douceur du profil affiché |
| Lissage pente | 1–∞ pts | 30 | Douceur de la courbe de pente instantanée |
| Fenêtre catégorisation | 50–∞ m | 500 | Largeur de la fenêtre glissante pour la couleur |
| Seuil détection | 0.1–∞ % | 0.5 | Sensibilité de la détection |
| **Garde-fou refinage (v2)** | **1–500 %** | **60** | **% appliqué à la longueur détectée, clamps 1500/8000 m** |
| Longueur min. catégorie | 0–∞ m | **250** *(v2.1)* | Fusion des micro-segments |

### 4.1 Paramètre avancé : garde-fou de refinage (v2)

Le paramètre `refinePct` est exposé en **mode avancé** (badge violet). Il détermine la distance maximale de recherche du vrai creux/sommet pour chaque montée :

```
effectiveRefine = clamp(refinePct% × longueurDétectée, 1500 m, 8000 m)
```

- **Valeur basse** (ex. 30 %) : recherche serrée autour du candidat. Rapide mais peut rater le vrai sommet.
- **Valeur par défaut** (60 %) : bon équilibre.
- **Valeur haute** (ex. 150 %) : recherche large. Plus de chance de trouver le vrai extremum, mais risque de capturer un extremum d'une autre montée.

**Transparence** : la valeur effective est affichée par montée dans la **colonne Refine** du tableau, avec un tooltip détaillant le calcul.

### 4.2 Guide de réglage

| Type de trace | Longueur min. catégorie | Fenêtre catégorisation | Garde-fou (%) |
|---|---|---|---|
| Courte (< 30 km) | 150–250 m | 200–300 m | 40–60 % |
| Moyenne (30–60 km) | 250–300 m | 300–500 m | 60 % |
| Longue (60–100 km) | 250–400 m | 500 m | 60–80 % |
| Très long (> 100 km) | 300–500 m | 500–1000 m | 60–100 % |

---

## 5. Parcours utilisateur

### 5.1 Parcours nominal

1. L'utilisateur ouvre la page → zone de dépôt visible
2. Il glisse son fichier GPX → analyse automatique lancée
3. Affichage immédiat :
   - Statistiques globales (distance, D+, D−, altitudes min/max)
   - Bande de segmentation colorée sans zones blanches
   - Profil altimétrique (graphique actif)
   - Graphe Pente **replié** (v2.1) — cliquer sur le titre pour le déplier
   - Tableau éditable des montées détectées avec boutons d'insertion contextuels
   - La drop-zone passe en **mode compact** (v2.3) — reste active pour recharger
4. Il ajuste les paramètres si nécessaire → recalcul en direct
5. **Il sélectionne une montée** (radio du tableau ou clic sur la ligne) → surbrillance jaune sur le graphique + poignées draggables
6. **Il glisse la poignée verte (vallée) ou rouge (sommet)** → les métriques se mettent à jour en temps réel, le tooltip est masqué pendant le drag
7. **Il peut aussi insérer une nouvelle montée** via les boutons contextuels entre les lignes
8. **Il désélectionne** en recliquant sur la radio ou en appuyant sur Échap
9. Il peut zoomer/dézoomer pour vérifier visuellement

### 5.2 Parcours d'erreur

| Cas | Comportement attendu |
|---|---|
| Fichier non-XML | Message d'alerte, pas de chargement |
| GPX sans balise `<ele>` | Message d'alerte, pas de chargement |
| Fichier très lourd (> 50 000 points) | Sous-échantillonnage automatique, latence acceptable |
| Édition créant un chevauchement | Contrainte automatique + indicateur ⚠ |
| Édition créant un segment invalide | Ligne marquée `invalid`, métriques affichées `—` |
| **Drag hors zone cliquable** | Fallthrough vers le pan (comportement normal) |
| **Insertion entre deux montées trop proches** | Bouton masqué (gap < 200 m) |

---

## 6. Architecture technique

### 6.1 Stack

| Composant | Choix | Justification |
|---|---|---|
| Langage | HTML / CSS / JavaScript vanilla | Aucun build, portable, lisible |
| Visualisation | Chart.js 4.4.1 | Léger, plugin-zoom disponible |
| Interaction | chartjs-plugin-zoom 2.0.1 + Hammer.js 2.0.8 | Molette, pinch, pan |
| Backend | Aucun | Confidentialité, simplicité |

### 6.2 Structure du code (ordre dans le fichier)

```
1.  Constantes internes (REFINE_MIN/MAX, INSERT_*, LONGUEUR_MIN_DEFAULT)
2.  Utilitaires (haversine, movingAverage, computeSlope, computeSlopeWindow,
                  findIdxAtDist, argminRange, argmaxRange, debounce, fmtKm)
3.  Lissage ancré (anchoredMovingAverage)
4.  Catégories + critères (CATEGORIES, CLIMB_CRITERIA, categorize,
                            minSlopeForLength, colorForSlope)
5.  Parseur GPX (parseGPX)
6.  Fusion segments courts + enrichissement (mergeShortSegments,
                                              enrichSegment, segmentByCategory)
7.  Détection (detectEvents, buildClimbCandidates, findBestSubClimb)
8.  Garde-fou hybride (computeEffectiveRefine)         ← v2
9.  Détection complète (computeAuto)
10. Helpers d'insertion contextuelle                    ← v2.2
11. État global (STATE, getParams)
12. Reconstruction (rebuild) — recalcule tout depuis STATE.userClimbs
13. Plugin Chart.js (segmentationPlugin)
14. Plugin Chart.js (markerDragPlugin)                  ← v0.5
15. Helpers drag (applyDragConstraint, updatePreviewMetrics, commitDrag)
16. Rendu (renderAll, climbRowHtml, insertionRowHtml)
17. Sélection / désélection                              ← v2.3
18. Zoom / sync
19. Chargement / détection auto
20. Interface (boutons, édition, événements, drop-zone)
21. Édition clavier (+ Échap)                            ← v2.3
22. Interception DOM capture (drag poignées)             ← v0.5
23. Enregistrement des plugins
```

### 6.3 Modèle de données en mémoire

```javascript
STATE = {
  dist: Float64Array,              // distances cumulées (m)
  eleRaw: Float64Array,            // altitudes brutes (m)
  eleDisplay: Float64Array,        // altitudes lissées ancrées
  rawPoints: Array<{lat, lon, ele}>, // points d'origine (pour recalcul auto)
  fileName: string | null,         // v2.3 — nom du fichier chargé
  autoClimbs: [                    // détection automatique courante
    { startKm, endKm, refineM, candidateLen }
  ],
  userClimbs: [                    // état utilisateur (éditable)
    {
      startKm, endKm,              // bornes actuelles (éditées ou auto)
      autoStartKm, autoEndKm,      // bornes auto pour le bouton reset
      refineM,                     // v2 — garde-fou effectif (null si manuel)
      candidateLen,                // v2 — longueur du candidat (null si manuel)
      _constrainedStart,           // flag interne : contrainte appliquée
      _constrainedEnd
    }
  ],
  elevChart: Chart,                // instance Chart.js
  slopeChart: Chart,               // instance Chart.js
  syncing: boolean,                // verrou anti-boucle pour le zoom
  selectedClimbIdx: number | null, // v0.3 — montée sélectionnée
  showRawDetail: boolean,          // v2.3 — toggle altitude brute
  slopeChartExpanded: boolean,     // v2.1 — repli graphe pente
  draggingHandle: null,            // v0.5 — 'valley' | 'summit' | null
  dragPreview: null,               // v0.5 — { handle, km } pendant drag
  panOriginalState: null,          // v0.5 — sauvegarde pan.enabled
}
```

### 6.4 Pipeline de calcul

```
ÉTAPE 1 — Détection initiale
  ├─ eleBase = MA(eleRaw, fenetreAffichage)
  ├─ slopeBase = MA(computeSlope(eleBase, dist), fenetrePente)
  └─ candidates = buildClimbCandidates(detectEvents(slopeBase, seuil))

ÉTAPE 2 — Sous-segment optimal
  └─ pour chaque candidat : findBestSubClimb(sur eleBase)

ÉTAPE 3 — Raffinement des bornes (v2 : garde-fou hybride)
  ├─ effectiveRefine = clamp(refinePct% × len, 1500, 8000)
  ├─ sommet : argmax dans [endIdx, endIdx + effectiveRefine]
  ├─ vallée : argmin dans [max(lastEnd+1, safetyLow), startIdx]
  └─ anchors = [vallée, sommet, vallée, sommet, ...]

ÉTAPE 4 — Lissage ancré
  └─ eleDisplay = anchoredMovingAverage(eleRaw, anchors)

ÉTAPE 5 — Pentes finales
  ├─ slopeRaw = computeSlope(MA(eleRaw, 5))
  ├─ slopeDisplay = MA(computeSlope(eleDisplay), fenetrePente)
  └─ slopeForCat = computeSlopeWindow(eleDisplay, fenetreCatM)

ÉTAPE 6 — Re-validation
  └─ pour chaque segment raffiné : findBestSubClimb(sur eleDisplay)

ÉTAPE 7 — Segmentation par catégorie
  └─ segmentByCategory(slopeForCat, ...) avec segments contigus

ÉTAPE 8 — Marqueurs
  └─ paires (vallée, sommet) pour annotation graphique
```

### 6.5 Cycle d'édition par drag & drop (v0.5)

```
1. Utilisateur sélectionne une montée (radio ou clic sur ligne)
   → Bande jaune + poignées dessinées

2. Utilisateur clique sur une poignée (mousedown/pointerdown)
   → Intercepté au window en phase capture
   → stopImmediatePropagation() : Chart.js ne voit rien
   → pan.enabled = false + tooltip.enabled = false
   → draggingHandle = 'valley' | 'summit'

3. Utilisateur déplace la souris (mousemove/pointermove)
   → km = scales.x.getValueForPixel(px)
   → constrainedKm = applyDragConstraint(handle, km, climb)
   → Mise à jour : dragPreview, métriques du tableau, chart.draw()
   → AUCUN rebuild complet

4. Utilisateur relâche (mouseup/pointerup)
   → commitDrag(handle, preview.km, climb) → écrit la valeur
   → renderAll() → rebuild complet
   → pan.enabled = true + tooltip.enabled = true
   → draggingHandle = null
```

---

## 7. Exigences non-fonctionnelles

| Exigence | Cible |
|---|---|
| Temps de calcul initial (10 000 pts) | < 2 secondes |
| Temps de recalcul après édition | < 500 ms |
| **Latence pendant drag (par frame)** | **< 16 ms (60 fps)** |
| Compatibilité navigateurs | Chrome, Firefox, Safari, Edge (2 dernières versions) |
| Support mobile | Responsive, lisible sur tablette (≥ 768 px) |
| **Support tactile drag** | **Touch events gérés (v0.5)** |
| Confidentialité | Aucune donnée envoyée sur un serveur |
| Accessibilité | Contraste WCAG AA, textes ≥ 12 px, **édition clavier (v0.5)** |
| Persistance | Aucune (état en mémoire uniquement) |

---

## 8. Hors périmètre (V2.3)

- Carte géographique (OpenStreetMap, satellite)
- Export PDF / PNG du profil
- Sauvegarde de la configuration utilisateur (localStorage)
- Comparaison multi-tracks
- Détection automatique de « cols » (regroupement de montées)
- Export CSV / JSON des montées
- Import FIT, TCX
- Édition des seuils de catégorie

---

## 9. Évolutions envisagées (V3+)

| Priorité | Fonctionnalité |
|---|---|
| P1 | Export PNG / PDF du profil avec annotations |
| P1 | Sauvegarde/chargement de la config (localStorage) |
| P1 | Édition des critères de validation dans l'UI |
| P1 | Édition des seuils de catégorisation |
| P2 | Carte OSM synchronisée avec le zoom |
| P2 | Regroupement automatique en « cols » (montées enchaînées) |
| P2 | Import FIT, TCX |
| P2 | Undo/Redo des éditions |
| P3 | Mode comparaison (2 traces côte à côte) |
| P3 | Export CSV des montées détectées |
| P3 | Traduction anglaise |
| P3 | Snap magnétique sur les extrema bruts pendant le drag |

---

## 10. Risques et mitigation

| Risque | Impact | Mitigation |
|---|---|---|
| Variabilité des traces GPS (bruit) | Faux positifs/négatifs | Double lissage + édition manuelle |
| Complexité des paramètres | Frustration utilisateur | Valeurs par défaut éprouvées + hints visuels |
| Trace très longue (> 50 km) | Lenteur | Sous-échantillonnage + Float64Array |
| Effet de bord entre paramètres | Régressions | Tests manuels sur traces de référence |
| Catégorisation trop sensible | Taches noires sur raidillons | Fenêtre glissante (fenetreCatM) |
| Chevauchement de montées éditées | Incohérence | Contrainte automatique + indicateur ⚠ |
| **Conflit drag / zoom-pan** | **Impossible de draguer** | **Capture DOM + désactivation pan (v0.5)** |
| **Tooltip masquant pendant drag** | **Courbes cachées** | **Désactivation tooltip (v2.1)** |
| **Insertion dans un gap trop étroit** | **Chevauchement** | **Bouton masqué si gap < 200 m (v2.2)** |

---

## 11. Critères d'acceptation

| Réf | Critère | Méthode de test |
|---|---|---|
| CA-01 | Fichier GPX (10 000 pts) chargé et analysé en < 2 s | Chronométrage manuel |
| CA-02 | Sur trace Morzine, les 7 montées sont détectées avec bornes attendues | Comparaison visuelle |
| CA-03 | Édition du km de vallée → mise à jour < 500 ms | Chronométrage manuel |
| CA-04 | Boutons `↺`, `✕`, « Recharger auto » fonctionnels | Test manuel |
| CA-05 | Zoom synchronisé entre les deux graphiques | Molette sur graphique 1 |
| CA-06 | Aucun crash sur fichier sans `<ele>` ou XML malformé | Test avec fichier corrompu |
| CA-07 | Aucune zone blanche entre segments de catégorie | Zoom visuel sur bande |
| CA-08 | Chevauchement empêché + indicateur ⚠ | Édition volontaire d'un chevauchement |
| CA-09 | 8 catégories affichées avec couleurs correctes | Vérification légende |
| CA-10 | Altitude lissée ancrée passe exactement par les extrema | Zoom sur un sommet |
| **CA-11** | **Drag & drop d'une poignée fonctionne sans déplacer le graphique (v0.5)** | Test manuel |
| **CA-12** | **Tooltip masqué pendant le drag, restauré après (v2.1)** | Test visuel |
| **CA-13** | **Boutons d'insertion contextuelle visibles aux bons endroits (v2.2)** | Vérification tableau |
| **CA-14** | **Désélection par reclic radio ou Échap (v2.3)** | Test manuel |
| **CA-15** | **Drop-zone reste active et visible après chargement (v2.3)** | Recharger un 2ᵉ fichier |
| **CA-16** | **Toggle brut : 3 états visuels distincts (off, user, auto) (v2.3)** | Cocher/décocher + sélectionner/désélectionner |
| **CA-17** | **Graphe Pente replié par défaut (v2.1)** | Observation au chargement |
| **CA-18** | **Colonne Refine affiche la valeur effective par montée (v2)** | Vérification tableau |

---

## 12. Traces de référence

| Nom | Distance | Caractéristique | Utilisation |
|---|---|---|---|
| Morzine TdF 2024 | ~103 km | 7 montées, D+ 2000+ m, longs vallons | Trace principale de validation |
| Sortie courte 30 km | ~30 km | Peu de montées | Test performances |
| Trace plat | ~50 km | Aucune montée | Test faux positifs |
| Trace montagne courte | ~15 km | 2–3 montées raides | Test sensibilité |

---

## 13. Interface utilisateur — wireframe textuel

```
┌─────────────────────────────────────────────────────────────────┐
│   Analyse GPX — Montées éditables                               │
│  Détection automatique + édition directe par drag & drop.       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │   trace.gpx — glissez un nouveau fichier .gpx             │  │  ← v2.3 compact
│  │     ou cliquez pour changer                               │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌── Paramètres ────----────────────────────────────────────┐   │
│  │  Lissage alt. [ 50 ]  Lissage pente [ 30 ]               │   │
│  │  Fenêtre cat. [ 500 ]  Seuil détection [ 0.5 ]           │   │
│  │  Long. min    [ 250 ]  Garde-fou [ 60 % ] [Avancé]       │   │  ← v2
│  │                                                          │   │
│  │  refine = clamp(60% × longueur, 1500 m – 8000 m)         │   │
│  │                                                          │   │
│  │  Critères : ≥1000m·3% | 750-1000m·4% | ...               │   │
│  │                                                          │   │
│  │  [ ↺ Recharger auto ]                                    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Statistiques ──────────────────────────────────────────┐   │
│  │  Distance  D+  D−  Alt.min  Alt.max  Montées  D+m  Km    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Segmentation par catégorie ────────────────────────────┐   │
│  │  [█████████████████████████████████████████████]         │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Profil altimétrique  ☐ Mettre en avant brut ───────────┐   │
│  │  ▲drag                                             ▼drag │   │  ← v0.5
│  │  [Graphique avec poignées draggables + bande jaune] -    │   │
│  │                                                          │   │
│  │  Glissez les poignées ▼ vallée / ▼ sommet                │   │
│  │  Flèches ← → · Shift 1m · Alt sommet · Échap dese.       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──  Pente (%)                                    ▶   ─────┐   │  ← v2.1 replié
│  │  Cliquez pour afficher le graphique de pente             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌── Stats catégories ───◊┐  ┌── Montées éditables ───────┐     │
│  │ Cat. | Dist | D+ | %   │  │        [ + Avant #1 ]      │     │  ← v2.2
│  │ Orange| 12.5 | 340|4.2 │  │ ☑ #1 | 3.42 | 8.15 | ...   │     │
│  │ Rouge |  2.1 | 120|8.5 │  │        [ + Entre #1 et #2] │     │
│  │ Noir  |  0.4 |  45|12.3│  │ ☐ #2 | 10.02 | 15.34|...   │     │
│  │                        │  │        [ + Après #2 ]      │     │
│  └────────────────────────┘  └────────────────────────────┘     │
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
| **Hystérésis** | Technique de détection avec seuil aller et seuil retour distincts |
| **Argmin / argmax** | Indice du minimum / maximum d'un tableau dans un intervalle |
| **Lissage ancré** | Moyenne mobile forcée à passer exactement par certains points |
| **Fenêtre glissante** | Intervalle mobile de N mètres autour d'un point de référence |
| **Pente instantanée** | Pente calculée sur 3 points consécutifs (différence centrée) |
| **Pente fenêtrée** | Pente calculée sur une portion de N mètres (moyenne) |
| **Sous-échantillonnage** | Réduction du nombre de points pour performance |
| **Garde-fou hybride (v2)** | `clamp(pct × longueur, min, max)` pour la recherche des extrema |
| **Drag & drop (v0.5)** | Édition directe des poignées vallée/sommet sur le graphique |
| **Insertion contextuelle (v2.2)** | Bouton `+` entre les lignes du tableau pour créer une montée |
| **Désélection (v2.3)** | Sortir du mode édition d'une montée (reclic radio ou Échap) |
| **Drop-zone persistante (v2.3)** | Zone de dépôt toujours visible, mode compact après chargement |

---

## 15. Historique des versions

| Version | Date | Changements majeurs |
|---|---|---|
| 0.1 | 2026-09-20 | Prototype : profil altimétrique + pente + détection basique |
| 0.2 | 2026-09-20 | Ajout segmentation 7 catégories |
| 0.3 | 2026-09-20 | Détection stricte des montées par critères longueur/pente |
| 0.4 | 2026-09-20 | Double lissage + affichage brut vs lissé |
| 0.5 | 2026-09-20 | Ancrage sur extrema bruts (sommets + vallées) |
| 0.6 | 2026-09-20 | Validation sur valeurs lissées (anti-faux-positifs) |
| 0.7 | 2026-09-21 | Édition manuelle des montées |
| 0.8 | 2026-09-21 | Anti-chevauchement + garde-fou fenêtre max |
| 0.9 | 2026-09-21 | 8ᵉ catégorie « Montée très raide » (> 10 %, noir) |
| 0.10 | 2026-09-21 | Catégorisation par fenêtre glissante (anti-taches noires) |
| 1.0 | 2026-09-21 | Segments contigus (anti-zones-blanches) — version stable |
| **0.3** (UX) | 2026-09-21 | **Sélection radio, surbrillance, drag & drop naïf** |
| **0.4–0.5** | 2026-09-21 | **Drag & drop fonctionnel via capture DOM + debug panel** |
| **2.0** | 2026-09-21 | **Garde-fou hybride `clamp(pct × len, 1500, 8000)` + colonne Refine** |
| **2.1** | 2026-09-21 | **Tooltip masqué pendant drag + `longueurMin=250` + graphe Pente repliable** |
| **2.2** | 2026-09-21 | **Insertion contextuelle de montées dans le tableau** |
| **2.3** | 2026-09-21 | **Drop-zone persistante + désélection radio/Échap + toggle brut 3 états** |

---

## 16. Annexes

### 16.1 Formule de Haversine

```
a = sin²(Δφ/2) + cos(φ1) × cos(φ2) × sin²(Δλ/2)
c = 2 × atan2(√a, √(1−a))
d = R × c
```

avec R = 6 371 000 m, φ la latitude, λ la longitude.

### 16.2 Calcul de la pente instantanée

```
pente[i] = (ele[i+1] − ele[i−1]) / (dist[i+1] − dist[i−1]) × 100
```

Différence centrée sur 3 points.

### 16.3 Calcul de la pente par fenêtre glissante

```
lo = dernier indice tel que dist[i] − dist[lo] ≤ winM
hi = premier indice tel que dist[hi] − dist[i] ≤ winM
pente[i] = (ele[hi] − ele[lo]) / (dist[hi] − dist[lo]) × 100
```

### 16.4 Lissage ancré

```
base = movingAverage(raw, win)
corr[a] = raw[a] − base[a]   pour chaque ancre a
result[i] = base[i] + interpolation linéaire(corr) sur [a, b]
```

Garantit `result[a] = raw[a]` pour chaque ancre.

### 16.5 Anti-chevauchement (édition numérique)

```javascript
let previousEndKm = -Infinity;
for (const c of sorted) {
  if (c.startKm < previousEndKm) {
    c._constrainedStart = true;
    c.startKm = previousEndKm + 0.001;  // +1 m
  }
  if (c.endKm <= c.startKm) {
    c._constrainedEnd = true;
    c.endKm = c.startKm + 0.1;  // au moins 100 m
  }
  previousEndKm = c.endKm;
}
```

### 16.6 Contrainte pendant drag (v0.5)

```javascript
function applyDragConstraint(handle, km, climb) {
  // Bornes globales
  let lo = prevClimb ? prevClimb.endKm + 0.001 : 0;
  let hi = nextClimb ? nextClimb.startKm - 0.001 : totalKm;

  if (handle === 'valley') {
    return Math.max(lo, Math.min(climb.endKm - 0.05, km));
  } else {
    return Math.max(climb.startKm + 0.05, Math.min(hi, km));
  }
}
```

### 16.7 Garde-fou hybride (v2)

```javascript
const REFINE_MIN_M = 1500;
const REFINE_MAX_M = 8000;

function computeEffectiveRefine(candidateLen, refinePct) {
  const pctValue = (refinePct * candidateLen) / 100;
  return Math.max(REFINE_MIN_M, Math.min(REFINE_MAX_M, pctValue));
}
```

### 16.8 Insertion contextuelle (v2.2)

```javascript
const INSERT_EDGE_MIN_KM = 1.0;
const INSERT_BETWEEN_MIN_KM = 0.2;
const INSERT_DEFAULT_LEN_KM = 1.0;

function insertInGap(gapStartKm, gapEndKm) {
  const gapLen = gapEndKm - gapStartKm;
  if (gapLen < INSERT_BETWEEN_MIN_KM) return null;

  const targetLen = Math.max(0.1, Math.min(INSERT_DEFAULT_LEN_KM, gapLen * 0.8));
  const center = (gapStartKm + gapEndKm) / 2;
  const start = Math.max(gapStartKm + 0.01, center - targetLen / 2);
  const end   = Math.min(gapEndKm - 0.01,   center + targetLen / 2);

  return end > start ? { start, end } : null;
}
```

### 16.9 Segments contigus

```javascript
// Point frontière partagé entre segments adjacents
const endIdx = (i === cat.length) ? i - 1 : i;
raw.push({ startIdx: start, endIdx, key: cat[start] });
start = i;
```

---

**Fin du PRD**