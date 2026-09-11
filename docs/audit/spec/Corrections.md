# Spécification du moteur de correction — transformation de la trace de travail

> **CORRECTIONS — V1.0**
> Complément de **ANALYSE V1.0** (détecteurs AR et RP — contrat de
> sortie des findings, §4) et **IHM V1.0** (présentation, interactions,
> prévisualisations, échanges). Ce document spécifie le **moteur de
> correction au niveau des données** : modèle d'état des anomalies,
> réalignement des index par identifiants stables, les trois
> transformations de la trace de travail — **suppression de points**,
> **routage**, **faux positif** — l'annulation par anomalie, la règle
> d'imbrication et les invariants.
>
> Découpage des responsabilités entre les trois documents (strict) :
> ANALYSE définit *ce qui est détecté* ; IHM définit *ce que
> l'utilisateur voit et manipule* (vues, sliders, prévisualisations,
> rendus) ; CORRECTIONS définit *ce que devient la donnée* — les
> structures d'état, les pseudocodes de transformation et de
> restauration, les garanties. Une réimplémentation du moteur de
> correction ne nécessite que ce document et le contrat de sortie
> d'ANALYSE (§4) ; les prévisualisations et le rendu graphique ne sont
> décrits ici que dans la mesure où ils **consomment** ou **produisent**
> des paramètres du moteur.

---

## Table des matières

1. [Objet, périmètre, garanties](#1)
2. [Modèle de données de correction](#2)
3. [Réalignement des index (`syncIndexes`)](#3)
4. [Chaîne post-édition (`afterTraceEdit`)](#4)
5. [Correction par suppression de points](#5)
6. [Correction par routage](#6)
7. [Faux positif](#7)
8. [Annulation d'une correction (`undoCorrection`)](#8)
9. [Règle d'imbrication](#9)
10. [Invariants de correction](#10)
11. [Cas limites](#11)
12. [Régression au niveau données](#12)

---

<a name="1"></a>
## 1. Objet, périmètre, garanties

### 1.1 Objet

Le moteur transforme la **trace de travail** (`working`) en consommant
les findings produits par les détecteurs (contrat ANALYSE §4) et les
paramètres produits par l'interface (plages sélectionnées, tracés
routiers reçus). Il couvre :

| Responsabilité | Sections |
|---|---|
| Réalignement des index des anomalies restantes après toute édition | §3 |
| Transformation « suppression de points » (AR et RP) | §5 |
| Transformation « routage » (insertion de points ORS) | §6 |
| Marquage « faux positif » | §7 |
| Annulation d'une correction, par anomalie | §8 |
| Protection des anomalies non corrigées (imbrication) | §9 |

### 1.2 Périmètre

**Inclus** : tout ce qui modifie `working`, l'état des findings, et
l'état d'annulation.

**Exclus** (spécifiés ailleurs) : la détection (ANALYSE V1.0), la
présentation et les interactions (IHM V1.0 — vues, prévisualisations,
rendus, boutons), les échanges réseau et fichiers (IHM V1.0 §17–§19).

### 1.3 Principe du flux batch

Les corrections s'accumulent sur la trace de travail **sans
re-détection**. Un verrou d'analyse (`isProcessed()`) est vrai dès
qu'au moins un finding a un statut ≠ `'pending'` : tant qu'il est
vrai, aucune nouvelle analyse n'est autorisée (IHM §1.2). La sortie du
cycle est l'export du GPX (fonction de `working`) suivi d'un
rechargement qui réinitialise l'intégralité de l'état. Les détecteurs
ne sont jamais exécutés sur une trace corrigée — les findings vivent
donc **une seule session de détection** et sont maintenus valides par
traduction (§3), jamais recalculés.

### 1.4 Garanties générales

| Garantie | Description |
|---|---|
| Intégrité par identifiants | Toute référence de finding à un point passe par son `id` stable ; la validité après édition est obtenue par **traduction**, jamais par arithmétique d'index |
| Restaurabilité | Toute correction pose un instantané d'annulation **complet avant** toute modification ; l'annulation restitue exactement l'état des points d'avant correction |
| Non-destruction | Aucun point d'une anomalie non corrigée n'est détruit ni déplacé par une correction (bornage des plages + règle d'imbrication, §9) |
| Non-dégénérescence | La trace de travail garde toujours ≥ 2 points |
| Pas de partage d'objets | Les instantanés d'annulation contiennent des **copies défensives** des points |
| Traçabilité | Les décomptes de corrections sont dérivables à tout instant de l'état des findings (aucun compteur parallèle) |

---

<a name="2"></a>
## 2. Modèle de données de correction

### 2.1 La trace de travail

`working` est une liste ordonnée de points :

```js
{ id: int, lat: number, lon: number, ele?: number }
```

| Règle | Détail |
|---|---|
| `id` stable | Alloué par un compteur global `nextPointId` (au chargement du fichier puis aux insertions de routage) ; **jamais recyclé** ; deux points ne partagent jamais un id, y compris après annulations |
| Partage initial | Les objets de `working` sont (initialement) ceux de la trace brute — mais le moteur ne modifie **jamais** un objet de point en place : il retire, insère ou restaure des points |
| Copie défensive | Toute donnée stockée dans un instantané d'annulation est une **copie** (`{ ...p }`) — cf. invariant C7 |
| Plancher | `working.length ≥ 2` en permanence (garde à la validation des suppressions) |

### 2.2 État d'un finding

```js
status     : 'pending' | 'corrected' | 'fp'
correction : null | 'delete' | 'route-car' | 'route-bike'   // si 'corrected'
undo       : null | { type:'delete', ... } | { type:'route', ... }
```

Transitions :

```
'pending' → 'corrected'   (suppression ou routage ; pose undo)
'pending' → 'fp'          (faux positif ; trace inchangée)
'fp'      → 'pending'     (retrait du marqueur)
'corrected' → 'pending'   (annulation ; undo consommé puis null)
```

Un finding `'corrected'` est **figé** : son emprise d'origine
(`zoneIds`) est intraduisible après édition (les points ont disparu) et
inutile — le réalignement (§3) le laisse intouché, son état visuel
persistant étant reconstruit depuis `undo` (points d'origine gris,
tracé appliqué, bornes).

### 2.3 Instantané d'annulation — `undo`

Pose **avant** toute modification de `working`. Deux formes :

**`type: 'delete'`** — suppression de la plage `[ds..de]` :

```js
{
  type          : 'delete',
  origPts       : [ { id, lat, lon, ele? }, ... ],  // COPIES des points supprimés,
                                                    // dans l'ordre de trace
  anchorLeftId  : id | null,   // id du point conservé avant la plage (null si ds = 0)
  anchorRightId : id | null,   // id du point conservé après la plage (null si de = m−1)
  firstNo       : int,         // premier n° affiché d'origine (ds + 1, base 1)
  absorbedFp    : [ finding, ... ]                  // faux positifs absorbés (§9.3), [] si aucun
}
```

**`type: 'route'`** — remplacement de l'intérieur `[start+1..end−1]`
par les points routés (les ancres `start` et `end` sont conservées) :

```js
{
  type         : 'route',
  origPts      : [ { id, lat, lon, ele? }, ... ],   // COPIES des points remplacés
  insertedIds  : [ id, ... ],                        // ids des points routés insérés, DANS L'ORDRE
  routePts     : [ { lat, lon }, ... ],             // TRACÉ DE RENDU : borné aux ancres EXACTES
                                                    //   (première = working[start], dernière =
                                                    //   working[end] AVANT modification) —
                                                    //   vit dans undo, détruit à l'annulation ;
                                                    //   ne vit PAS dans working
  startPt      : { lat, lon },                      // borne amont conservée (rendu)
  endPt        : { lat, lon },                      // borne aval conservée (rendu)
  firstNo      : int,                               // start + 2 (numérotation d'origine)
  absorbedFp   : [ finding, ... ]
}
```

**Distinction fondamentale — points insérés vs tracé de rendu** :

| | `insertedIds` / `mids` | `routePts` |
|---|---|---|
| Durée de vie | **vit dans `working`** (points réels de la trace, exportés) | **vit dans `undo`** uniquement (support du rendu persistant) |
| Contenu | coordonnées ORS **amputées de leurs 2 extrémités** (les ancres ne sont pas dupliquées) | coordonnées ORS **préfixées/suffixées par les ancres exactes** de la trace de travail — sans cette extension, le tracé rendu s'arrêtait à court du premier/dernier vertex ORS de quelques mètres |
| Nombre | `|coords| − 2` insérés | `|coords|` (ancres comprises) |
| À l'annulation | retirés de `working` | détruits avec l'instantané |

**`absorbedFp`** : liste des findings en statut `'fp'` dont l'emprise
intersectait la plage affectée au moment de la correction (§9.3). Ils
quittent la liste des findings (l'utilisateur ne les traite plus) et
sont **réinjectés** avec leur statut à l'annulation de la correction
qui les a absorbés (§8.3).

### 2.4 Mesures invariantes et régénération des textes

Les textes affichables (`summary`, `parts[].text`) sont **dérivables**
des mesures invariantes portées par le finding (`pairs` avec leurs
distances `d`, `ecart` pour AR, `totalAngle`/`turnText` pour RP) et des
index courants. C'est ce qui permet au réalignement (§3) de mettre à
jour les numéros affichés sans jamais altérer les mesures géométriques :

| Finding | Champ | Gabarit (n° affiché = index + 1 ; distances : 1 décimale si < 10 m, arrondi sinon) |
|---|---|---|
| AR | `summary` | `sommet pt {peak+1} · {k} paire(s) · écart {arrondi(ecart)}°` |
| AR | `parts[0].text` | `paires miroirs : pts {a+1}↔{b+1} ({d} m) · …` ou `retournement isolé, aucune paire miroir` (k = 0) |
| AR | `parts[1].text` | `cœur : demi-tour · branches {d1}/{d2} m` |
| RP | `summary` | `jonction pt {peak+1} · {totalAngle}° ({turnText}) · {k} paire(s)` |
| RP | `parts[0].text` | `superpositions : pts {a+1}↔{b+1} ({d} m) · …` ou `refermeture par croisement de trace` |
| RP | `parts[1].text` | `cœur : angle cumulé {totalAngle}° ({turnText})` |

---

<a name="3"></a>
## 3. Réalignement des index (`syncIndexes`)

Après **toute** modification de `working`, les index portés par les
findings (positions, paires, contextes, bornes de cœur) doivent être
retraduits depuis les identifiants stables.

### 3.1 Pseudocode complet

```text
syncIndexes() :
    si working ou analysis absent : retour
    idx ← Map( id → index courant )          // construite sur working
    M = working.length
    pour chaque finding f de analysis.findings :
        si f.status == 'corrected' : passer   // emprise d'origine intraduisible (C6)
        s  = idx[ f.zoneIds[0] ]
        e  = idx[ f.zoneIds[ dernier ] ]
        pk = idx[ f.peakId ]
        si s == null ou e == null ou pk == null :
            erreur console « emprise introuvable » ; f INCHANGÉ ; continuer
        f.parts[0].s ← s ;  f.parts[0].e ← e ;  f.peak ← pk
        si f.kind == 'ar' :
            f.parts[1].s ← max(0, pk − 1)
            f.parts[1].e ← min(M − 1, pk + 1)
        sinon :   // 'rp'
            cs = idx[ f.coreIds[0] ] ;  ce = idx[ f.coreIds[ dernier ] ]
            si cs ≠ null et ce ≠ null : f.parts[1].s ← cs ; f.parts[1].e ← ce
            sinon : erreur console « cœur introuvable » (f.parts[1] inchangé)
        pour chaque paire p de f.pairs :
            p.a ← idx[ p.aid ] ;  p.b ← idx[ p.bid ]
        f.pairIdx ← aplatissement des paires
        f.ctx.up ← ctxIds.up ≠ null ? (idx[ctxIds.up] ?? null) : null
        f.ctx.dn ← ctxIds.dn ≠ null ? (idx[ctxIds.dn] ?? null) : null
        rebuildTexts(f)                       // gabarits §2.4 — numéros seuls traduits
    trier findings par parts[0].s croissant
```

### 3.2 Garanties

| Garantie | Détail |
|---|---|
| Correction quel que soit l'ordre | La traduction par ids est correcte pour tout enchaînement et mélange de corrections et d'annulations — contrairement à toute translation arithmétique d'index, fausse dès la deuxième édition ou dès qu'une annulation réinsère des points au milieu |
| Non-régression des mesures | Les mesures géométriques (`d` des paires, `ecart`, `totalAngle`, `d1/d2`) sont **invariantes** : seule la numérotation affichée bouge |
| Stabilité géographique | Après réalignement, les positions géographiques des anomalies restantes sont inchangées ; seuls les numéros affichés se décalent (l'hôte le signale par un toast) |
| Non-touché des corrigées | Les findings `'corrected'` ne sont jamais retraduits (C6) |
| Auto-ordre | La re-tri final maintient les findings dans l'ordre de trace, condition du bornage mutuel des emprises et des sliders |

### 3.3 Condition de validité

La traduction est **correcte à coup sûr** tant que sa précondition est
respectée : **aucune correction ne détruit un point appartenant à
l'emprise d'un finding non corrigé**. Cette précondition est garantie
par : (a) le bornage des sliders de l'IHM sur l'emprise de l'anomalie
en cours (IHM §7.1, §8.1, §9), (b) la règle d'imbrication (§9) qui
refuse toute plage mordant un finding `'pending'`, (c) l'absorption des
`'fp'` (§9.3) qui retire de la liste ceux qu'une correction volontaire
recouvre. La perte du réalignement est par ailleurs **non silencieuse** :
émission en console et finding inchangé (dégradation visible, jamais
d'index faux silencieux).

---

<a name="4"></a>
## 4. Chaîne post-édition (`afterTraceEdit`)

Toute transformation réussie enchaîne, dans cet ordre :

```text
afterTraceEdit(f, detail) :
    state.geo ← buildGeometry(working)      // reconstruction — rétablit l'invariant
                                            // geo.ids[i] = working[i].id (C4)
    analysis.geo ← geo
    syncIndexes()                           // §3
    re-rendus : trace (sans recadrage), fileinfo, carte, liste
    nettoyage : étiquettes de prévisualisation, marqueurs de sliders
    updateLocks()                           // verrous + recoloration du bouton Générer
    toasts : « Anomalie traitée (detail) — trace : N points · P à traiter. »
             + « Toutes les anomalies sont traitées… » si P = 0
    panneau → vue « annuler la correction » pour f
```

Le point de données décisif est le premier : **la géométrie métrique
est toujours reconstruite intégralement après édition** — elle n'est
jamais mise à jour incrémentalement, ce qui garantit l'invariant C4 sans
cas particulier (suppressions, insertions, restaurations).

---

<a name="5"></a>
## 5. Correction par suppression de points

### 5.1 Paramètres produits par l'IHM (entrée du moteur)

| Famille | Paramètre | Signification |
|---|---|---|
| AR | plage `[ds..de]`, `ds ≤ de` | points à supprimer — les curseurs la bornent **directement** ; un point isolé est possible (`ds = de`) |
| RP | couple `(début, fin)`, `fin ≥ début + 1` | les curseurs portent les **points conservés** ; la plage réellement supprimée est `[début+1..fin−1]` ; **vide** si `fin = début + 1` → refus (§5.2) |

(Les prévisualisations qui produisent ces paramètres — gris/bleu,
chemin bleu, trait rouge dynamique — sont spécifiées en IHM §8.2/§9.2 ;
le moteur ne les connaît pas.)

### 5.2 Gardes (dans l'ordre, tout échec = refus toasté, état intact)

```text
1. RP : si début + 1 > fin − 1  → refus « Aucun point à supprimer entre
   les curseurs — écartez-les d'abord. »                     (plage vide)
2. nestingGuard(f, ds, de) ≠ null → refus nommant l'anomalie bloquante
   (§9.1)                                                   (imbrication)
3. working.length − (de − ds + 1) < 2 → refus « la trace deviendrait
   dégénérée. »                                             (C3)
```

### 5.3 Transformation

```text
undo ← { type:'delete',
         origPts       : copies(working[ds..de]),           // §2.3 — copies défensives
         anchorLeftId  : ds > 0        ? working[ds−1].id : null,
         anchorRightId : de < m − 1    ? working[de+1].id : null,
         firstNo       : ds + 1,
         absorbedFp    : absorbFpFindings(f, ds, de) }     // §9.3 — AVANT le retrait
working ← working[0..ds) ∪ working(de+1..]              // retrait de la plage
f.status ← 'corrected' ;  f.correction ← 'delete'
afterTraceEdit(f, « suppression de N point(s) »)
```

Le choix `absorbFpFindings` **avant** le retrait est un point
d'ordre : l'absorption lit les index courants (`parts[0]` des FP) qui
ne sont plus interprétables après l'édition.

### 5.4 État résultant

- Les points supprimés **sortent** de `working` mais leurs objets
  copiés survivent dans `undo.origPts` — c'est le support du rendu
  persistant « points d'origine gris » (IHM §8.4/§9.4) et de la
  restauration (§8) ;
- Les ancres (`anchorLeftId`/`anchorRightId`) désignent les points
  conservés adjacents : ce sont les points **d'ancrage de la
  réinsertion** (§8.2) ;
- Les findings non corrigés voient leurs index retraduits par §3 : une
  suppression **amont** décale tous les index vers le bas ; une
  suppression **à l'intérieur de l'emprise d'un finding RP** serait
  sinon un cas de perte de cohérence — elle est impossible si ce
  finding est `'pending'` (gardes) et détruirait un `'fp'` absorbé
  (dont la restauration repose sur les ids d'origine, toujours
  corrects).

---

<a name="6"></a>
## 6. Correction par routage

### 6.1 Paramètres produits par l'IHM (entrée du moteur)

| Paramètre | Signification |
|---|---|
| `start`, `end` | indices des **ancres** — conservées dans la trace ; contrainte `end ≥ start + 1` |
| profil | `'driving-car'` ou `'cycling-road'` — déterminé par l'IHM (radio, ou tracé unique si les deux profils sont géographiquement identiques, cf. IHM §7.3) |
| `coords` | tracé ORS reçu : liste `[lon, lat, ele?]` convertie par l'IHM en points géographiques — **sans ids** |

### 6.2 Gardes

```text
1. nestingGuard(f, start + 1, end − 1) ≠ null → refus (§9.1)
   — la zone REMPLACÉE est l'intérieur ; les ancres sont conservées,
     la garde ne porte donc que sur l'intérieur
2. absorbed = absorbFpFindings(f, start + 1, end − 1)
```

### 6.3 Transformation

```text
inner = working[start+1 .. end−1]                    // points remplacés
mids  = pour chaque coordonnée c de coords[1 .. |coords|−2] :
            { id : nextPointId++,                    // NOUVEAUX ids, jamais recyclés
              lat : c.lat, lon : c.lon,
              ele : c.ele si finie }                 // élévation ORS conservée
undo ← { type:'route',
         origPts     : copies(inner),
         insertedIds : [ p.id de mids, dans l'ordre ],
         routePts    : [ working[start], mids..., working[end] ],
                       // tracé de RENDU borné aux ancres exactes — §2.3
         startPt     : { lat, lon de working[start] },
         endPt       : { lat, lon de working[end] },
         firstNo     : start + 2,
         absorbedFp  : absorbed }
working ← working[0..start] ∪ mids ∪ working[end..]
f.status ← 'corrected' ;  f.correction ← profil == 'driving-car' ? 'route-car' : 'route-bike'
afterTraceEdit(f, « routage voiture/vélo de route »)
```

### 6.4 Points de contrat

| Point | Détail |
|---|---|
| Ancres conservées | `working[start]` et `working[end]` ne sont ni dupliqués ni modifiés — leur `ele` d'origine est préservée ; les `mids` prennent l'élévation ORS |
| Ordre des `insertedIds` | Celui de la trace (amont → aval) — condition du garde-fou de contiguïté à l'annulation (§8.1) |
| Zone intérieure vide | `start + 1 > end − 1` (ancres adjacentes) → `inner` vide, `mids` couvre le passage : remplacement trivial, `origPts` vide, annulation toujours possible (garde-fous trivialement satisfaits) |
| Le routage **n'informe pas** la suppression | Les deux corrections sont indépendantes ; un routage peut être appliqué sur une zone dont une suppression a déjà réduit le voisinage — les bornes sont relues sur `working` courant au moment de l'application |

---

<a name="7"></a>
## 7. Faux positif

```text
markFp(f)   : f.status ← 'fp'          // trace INTACTE — aucune donnée modifiée
unmarkFp(f) : f.status ← 'pending' ; f.correction ← null
```

Un finding `'fp'` est laissé **tel quel** dans la trace exportée. Deux
interactions avec le moteur :

1. un `'fp'` **ne bloque pas** les corrections des autres anomalies
   (contrairement à un `'pending'`) — il est **absorbé** si une
   correction recouvre son emprise (§9.3) ;
2. un `'fp'` absorbé puis restauré (par annulation) retrouve son
   statut `'fp'` et sa place dans la liste — il reste alors traitable
   normalement (retrait du marqueur, etc.).

---

<a name="8"></a>
## 8. Annulation d'une correction (`undoCorrection`)

Par anomalie : chaque finding `'corrected'` expose l'annulation. Le
moteur restitue la trace et l'état ; l'IHM re-rend et retourne à la
vue d'action principale.

### 8.1 `type = 'route'` — trois garde-fous successifs

```text
1. usedByOther = ∃ g ≠ f, g.undo ≠ null, tel que
       un id ∈ f.undo.insertedIds figure dans g.undo.origPts
   si usedByOther : refus « la zone routée a été réutilisée par une
       correction ultérieure. »
       — protège contre la destruction d'un point devenu l'origine
         (point gris) d'une autre correction

2. pos = position dans working du premier id inséré :
       pos = working.findIndex(p => p.id == insertedIds[0])
   si pos < 0 : refus « points de routage introuvables. »

3. contiguïté ordonnée :
       pour k = 0 .. |insertedIds| − 1 :
           working[pos + k] doit exister ET
           working[pos + k].id == insertedIds[k]
       sinon : refus « la zone a été modifiée par une correction
           ultérieure. »
       — la séquence insérée doit être intacte, contiguë et dans l'ordre

PUIS :
    working ← working[0..pos) ∪ copies(origPts) ∪ working[pos + |insertedIds| ..]
```

### 8.2 `type = 'delete'` — réinsertion par ancre

```text
idx = Map( id → index courant )
pos = anchorRightId ≠ null ? idx[anchorRightId]                    // insérer AVANT l'ancre droite
      : anchorLeftId ≠ null ? idx[anchorLeftId] + 1                // insérer APRÈS l'ancre gauche
      : null
si pos == null : refus « points d'ancrage disparus (corrections
    ultérieures). »                    // les deux ancres ont disparu
working ← working[0..pos) ∪ copies(origPts) ∪ working[pos..]
```

L'ancre droite est préférée : elle couvre tous les cas (le point après
la plage existe sauf si la plage touchait la fin de trace, auquel cas
l'ancre gauche prend le relais et la réinsertion se fait en queue).
Échec seulement si **les deux** ancres ont disparu — ce qui exige deux
corrections ultérieures mordant de part et d'autre.

### 8.3 Finalisation commune

```text
f.status ← 'pending' ;  f.correction ← null ;  f.undo ← null
si absorbedFp non vide :
    pour chaque g de absorbedFp : g.status ← 'fp' ;  findings += g
    (le nombre de FP restaurés est signalé par un toast)
state.geo ← buildGeometry(working) ;  analysis.geo ← geo
syncIndexes()                            // re-trie aussi les findings (§3.1)
re-rendus complets ;  updateLocks() ;  toasts ;  panneau → apMain
```

**Restauration des FP absorbés** : leurs `ids` repartent avec les
points restaurés (les `origPts` portent les ids d'origine) —
`syncIndexes` les retrouve donc sans opération supplémentaire. Le
statut `'fp'` est **maintenu** (l'utilisateur n'a pas démarqué
l'anomalie).

### 8.4 Garantie de restitution exacte

L'annulation restitue **exactement** le multi-ensemble de points
d'avant correction : mêmes ids (les `origPts` sont copiés avec leurs
ids, les `insertedIds` ne sont jamais recyclés), mêmes coordonnées
(copies), même ordre (insertion par ancre calculée / position
contiguë vérifiée). C'est l'invariant C8 — vérifiable mécaniquement
par le test de régression D3/D4 (§12).

---

<a name="9"></a>
## 9. Règle d'imbrication

Les emprises AR (quelques points) et RP (jusqu'à ~1 km) peuvent
**s'imbriquer** : un zigzag AR dans l'approche d'une boucle RP est un
cas réel documenté. Le bornage des sliders empêche le chevauchement
latéral des plages de sélection, mais pas l'inclusion d'une emprise
dans une autre.

### 9.1 Garde formelle (`nestingGuard`)

```text
nestingGuard(f, zs, ze) :
    retourne le PREMIER finding g tel que
        g ≠ f  ET  g.status == 'pending'  ET
        g.parts[0].e ≥ zs  ET  g.parts[0].s ≤ ze     // intersection
    sinon null
```

Plages vérifiées par correction (la plage **réellement affectée**, pas
l'emprise totale de l'anomalie) :

| Correction | Plage vérifiée |
|---|---|
| Suppression AR | `[start..end]` |
| Suppression RP | `[début+1..fin−1]` (l'intérieur des conservés) |
| Routage | `[start+1..end−1]` (l'intérieur des ancres) |

**Refus toasté** si le garde retourne un finding : « Correction
refusée : traitez d'abord "« label »", imbriquée dans cette zone (ou
marquez-la faux positif). »

### 9.2 Ordre de traitement

| Ordre | Résultat |
|---|---|
| **Imbriquée (AR) d'abord** | Sa correction retire ses points ; `syncIndexes` **contracte automatiquement** l'emprise de la boucle RP (ses ids à elle n'ont pas été détruits) ; la correction RP se déroule ensuite normalement — aucune opération spéciale |
| **Englobante (RP) d'abord** | **Refus** tant que l'AR est `'pending'` — la protection garantit qu'aucune anomalie non corrigée n'est détruite par une décision de l'utilisateur |

L'utilisateur a toujours une issue : traiter l'imbriquée d'abord, ou la
marquer faux positif (elle cesse alors de bloquer, cf. §9.3).

### 9.3 Faux positif englobé (absorption / restauration)

Un `'fp'` **ne bloque jamais** :

```text
absorbFpFindings(f, zs, ze) :
    absorbed = [], keep = []
    pour chaque g de findings :
        si g ≠ f ET g.status == 'fp' ET g.parts[0] intersecte [zs, ze] :
            absorbed += g ;  toast « "« label »" (faux positif) absorbée
                              par la correction. »
        sinon : keep += g
    findings ← keep
    retour absorbed        // stocké dans f.undo.absorbedFp (§2.3)
```

L'absorption est **réversible** : l'annulation de la correction
réinjecte les FP avec leur statut (§8.3). Sémantique assumée : un FP
signifie « ne me corrige pas en tant que telle », mais l'engloutir
dans une correction volontaire plus large est une décision
d'utilisateur — et elle reste annulable intégralement.

### 9.4 Cas connexe assumé

Une même zone peut recevoir un finding AR **et** un finding RP quasi
coextensifs (ex. une aiguille rejetée par l'anti-aiguille RP mais
captée par l'AR). La liste montre les deux ; la garde empêche toute
correction contradictoire simultanée ; une fois l'un traité (ou passé
FP), l'autre suit normalement.

---

<a name="10"></a>
## 10. Invariants de correction

Vérifiables mécaniquement à tout instant :

| # | Invariant |
|---|-----------|
| **C1** | Toute correction pose un `undo` **complet avant** toute modification de `working` |
| **C2** | Les plages affectées ne détruisent jamais un point d'un finding non corrigé : suppression AR `[start..end]` ⊆ emprise de `f` ; suppression RP `[début+1..fin−1]` ⊆ emprise de `f` ; routage `[start+1..end−1]` ⊆ emprise de `f` ; et `nestingGuard` refuse toute morsure d'un `pending` tiers |
| **C3** | `working.length ≥ 2` après toute correction acceptée |
| **C4** | Après toute édition, `geo.ids[i] = working[i].id` pour tout i (géométrie systématiquement reconstruite, §4) |
| **C5** | Les ids ne sont jamais recyclés : `nextPointId` est strictement croissant ; deux points vivants ou instantanés ne partagent jamais un id |
| **C6** | `syncIndexes` laisse les findings `'corrected'` intacts (emprise d'origine intraduisible et inutile) |
| **C7** | `undo.origPts` et tout point d'un instantané sont des **copies défensives** — aucun objet partagé entre les instantanés et `working` |
| **C8** | Toute annulation réussie restitue **exactement** le multi-ensemble de points d'avant correction : mêmes ids, mêmes coordonnées, même ordre (insertion par ancre ou position contiguë vérifiée) |
| **C9** | Les FP absorbés sont restaurés avec leur statut `'fp'` à l'annulation de la correction qui les a absorbés — jamais perdus ni requalifiés |
| **C10** | La trace exportée est exactement `working` — le moteur n'a aucune autre source de vérité |
| **C11** | Les plages des paramètres sont toujours bornées par l'emprise de l'anomalie en cours et par les emprises des anomalies `pending`/`fp` voisines (bornage des sliders, IHM §7.1/§8.1/§9) — le moteur ne re-borne pas, il fait confiance et vérifie par C2 |
| **C12** | Un finding `'corrected'` ne peut être re-corrigé ni annulé deux fois : l'annulation consomme `undo` (null après) et le statut détermine les vues disponibles |

---

<a name="11"></a>
## 11. Cas limites

| Situation | Comportement |
|---|---|
| Anomalie en tête de trace (suppression touchant le début) | `anchorLeftId = null` ; réinsertion par ancre droite uniquement ; si l'ancre droite a aussi disparu → refus toasté (issue : cycle « Générer → recharger ») |
| Suppression d'un **point isolé** (AR, `ds = de`) | Plage d'un point ; raccord direct entre `ds−1` et `de+1` ; `undo.origPts` d'un seul point |
| Plage RP vide (`fin = début + 1`) | Refus toasté — l'état « rien à supprimer » est réglable mais non validable |
| Routage à intérieur vide (ancres adjacentes) | `inner` vide ; `origPts` vide ; annulation possible (insertedIds retrouvés intacts) |
| Routage sur zone dont le voisinage a déjà été supprimé | Ancres relues sur `working` courant — le bornage de l'IHM garantit la validité |
| Annulation route après une correction ultérieure **chevauchante** | Refus par garde-fou 1 ou 3 (selon que le point est devenu origine d'une autre correction ou que la séquence a été rompue) — la correction ultérieure reste annulable elle |
| Annulation delete dont les deux ancres ont disparu | Refus toasté — issue : le cycle « Générer → recharger » |
| FP absorbé, correction annulée, puis FP re-marqué `pending` puis re-corrigé | Chemin normal : les ids restaurés sont stables, aucune interaction résiduelle |
| Deux corrections successives sur des zones voisines disjointes | Les deux `undo` coexistent ; chacune annulable indépendamment dans un ordre quelconque (ids) |
| `syncIndexes` : id d'emprise introuvable | Erreur console, finding inchangé — dégradation **non silencieuse** ; ne survient pas si C2 est maintenu |

---

<a name="12"></a>
## 12. Régression au niveau données

Tests purs (sans rendu) — exécutables par comparaison d'état
(`working` sérialisé : suite d'`id`/`lat`/`lon`/`ele`) :

| # | Test | Verdict attendu |
|---|---|---|
| D1 | Suppression **amont** d'une anomalie (plage avant son emprise) puis `syncIndexes` | Les index du finding descendent exactement du nombre de points supprimés ; textes régénérés avec les nouveaux numéros ; mesures `d` inchangées |
| D2 | Suppression **aval** puis `syncIndexes` | Index du finding inchangés (l'édition est postérieure à son emprise) |
| D3 | Routage appliqué puis annulation | `working` **identique point par point** (ids, lat/lon, ele, ordre) à l'état pré-correction (C8) |
| D4 | Suppression puis annulation | `working` identique point par point à l'état pré-correction |
| D5 | AR imbriquée dans RP : correction AR, puis correction RP, puis annulations **dans un ordre quelconque** | Chaque annulation restitue exactement son propre état antérieur ; à la fin des deux annulations, `working` = état initial ; les deux findings sont `pending` avec index corrects |
| D6 | FP dont l'emprise est recouverte par un routage, puis annulation du routage | FP absent de la liste pendant la correction ; réapparu en statut `'fp'` après annulation (C9) |
| D7 | Routage dont l'intérieur mord un `pending` tiers | Refus — `working` inchangé (C2) |
| D8 | Routage, puis suppression chevauchant la zone routée, puis annulation du **routage** | Refus (garde-fou 1 ou 3) — la suppression reste annulable elle |
| D9 | Suppression RP avec `fin = début + 1` | Refus « Aucun point à supprimer » |
| D10 | Suppression amenant `working` à 1 point | Refus « trace dégénérée » (C3) |
| D11 | Double tour : suppression RP réduite au 2ᵉ tour, puis re-détection sur le GPX exporté | La boucle n'est plus détectée (le tour redondant a disparu) |
| D12 | `corrections` du bloc d'audit (IHM §19.2) recomputé deux fois à suite de corrections fixée | Identique (déterminisme — dérivé exclusivement de l'état des findings) |

---

**Fin du document CORRECTIONS V1.0.**
Documents associés : **ANALYSE V1.0** — détection AR/RP (contrat
d'entrée des findings) ; **IHM V1.0** — présentation, interactions,
prévisualisations, échanges (les paramètres d'entrée du moteur : plages,
profil, coordonnées ORS).
