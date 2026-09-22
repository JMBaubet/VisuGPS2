# PRD — Intégration d'étiquettes SVG dans une application Mapbox existante

**Version** : 1.1
**Portée** : Spécifications pour intégrer des étiquettes SVG déjà produites dans une application Mapbox GL JS existante.

---

## Changelog v1.0 → v1.1

| Domaine | Modification |
|---|---|
| **svgW obliques** | Calcul **dynamique** : `svgW = max(150, W + 30)`. Le tableau à valeurs fixes (`150`, `tipInVbX ∈ {15, 135}`) n'est valable que pour `W ≤ 120`. |
| **offsetX oblique** | Formule générale `±(W/2 + 10) × scale`. Les `±60` ne valent que pour `W = 100`. |
| **Règle texte clair** | Extension à **Blue Grey 100–300**. |
| **§2.3** | Tableau réécrit avec `total = max(150, W+30)`. |

---

## 1. Objet du document

Ce document définit les **spécifications d'intégration** d'étiquettes SVG dans une application Mapbox GL JS. Il s'adresse à un développeur qui dispose :

- d'une application Mapbox fonctionnelle,
- d'un lot d'étiquettes SVG déjà produites,
- et qui doit les **afficher au bon endroit**, à la **bonne taille**, sans déformer ni décaler la pointe de l'étiquette.

Le document ne traite **que** :

1. le **positionnement** des étiquettes (variantes centre, gauche, droite),
2. la **mise à l'échelle** (étiquettes plus ou moins grandes).

Tout ce qui concerne le style de l'étiquette est **hors périmètre**.

---

## 2. Contrat d'entrée

### 2.1 Données obligatoires

| Champ | Type | Description |
|---|---|---|
| `svg` | string | Le SVG complet et autonome (`<svg>…</svg>`) |
| `lngLat` | `[lon, lat]` | Coordonnée GPS où la **pointe** doit se poser |
| `variant` | `'center' \| 'left' \| 'right'` | Position de la queue de la bulle |

### 2.2 Données géométriques du SVG

| Constante | Valeur | Signification |
|---|---|---|
| `TIP_Y` | 58 | Position verticale de la pointe dans le `viewBox` |
| `TOTAL_H` | 70 | Hauteur totale du `viewBox` |
| `PANEL_H` | 34 | Hauteur du panneau (hors queue) |
| `PANEL_TOP` | 6 | Position verticale du haut du panneau |
| `PANEL_BOTTOM` | 40 | Position verticale du bas du panneau |
| `W` | variable | Largeur naturelle du panneau (calculée selon le contenu texte + icône) |

### 2.3 Données géométriques dépendantes de la variante

Soit `total = max(150, W + 30)` pour les variantes obliques.

| Variante | `vbX` | `vbW` | `svgW` | `xL` | `xR` | `tipX − vbX` |
|---|---|---|---|---|---|---|
| **center** | `0` | `W` | `W` | `0` | `W` | `W/2` |
| **left** | `−15` | `total` | `total` | `−15 + (total−W)/2` | `xL + W` | `(total−W)/2 − 10` |
| **right** | `−15` | `total` | `total` | `−15 + (total−W)/2` | `xL + W` | `(total−W)/2 + W + 10` |

> ⚠️ **v1.1** : pour `W ≤ 120`, `total = 150` (valeur historique, cas de référence). Au-delà, `total = W + 30` pour éviter un viewBox trop étroit.

**Cas de référence `W = 100`** (valeur historique) :

| Variante | `vbX` | `vbW` | `svgW` | `tipX − vbX` |
|---|---|---|---|---|
| **center** | 0 | 100 | 100 | 50 |
| **left** | −15 | 150 | 150 | 15 |
| **right** | −15 | 150 | 150 | 135 |

> Le **viewBox** du SVG doit être lu tel quel depuis le SVG reçu. Les valeurs de `vbX`, `vbW`, `svgW` doivent en être déduites si elles ne sont pas fournies explicitement.

---

## 3. Positionnement — Principe général

### 3.1 Contrainte fondamentale

La **pointe de l'étiquette** doit coïncider **exactement** avec la coordonnée GPS `lngLat`, quelle que soit :

- la variante (center, left, right),
- la taille actuelle de l'étiquette,
- le niveau de zoom de la carte.

Cette contrainte doit être **respectée en permanence**, y compris pendant les transitions (drag, zoom, animation).

### 3.2 Solution : ancrage par offset

Mapbox GL JS ancre un marker par son **bas-centre** lorsqu'on utilise `anchor: 'bottom'`. Comme la pointe de l'étiquette peut être ailleurs, il faut fournir un **offset** en pixels :

```
offset = [offsetX, offsetY]
```

- **`offsetX`** compense le décalage horizontal entre le centre du SVG et la pointe.
- **`offsetY`** compense le décalage vertical entre le bas du SVG et la pointe.

### 3.3 Formule de calcul de l'offset

Soient :

- `tipInVbX = tipX − vbX` : position horizontale de la pointe dans le repère du `viewBox`,
- `centerVbX = vbW / 2` : centre horizontal du `viewBox`,
- `scale` : facteur d'échelle actuel (voir §4).

Alors :

```
offsetX = (centerVbX − tipInVbX) × scale
offsetY = (TOTAL_H − TIP_Y) × scale
        = (70 − 58) × scale
        = 12 × scale
```

### 3.4 Valeurs résultantes par variante (v1.1)

La formule ci-dessus se simplifie en :

| Variante | `offsetX` (à `scale = 1`) | `offsetY` (à `scale = 1`) |
|---|---|---|
| **center** | `0` | `12` |
| **left** | `+(W/2 + 10)` | `12` |
| **right** | `−(W/2 + 10)` | `12` |

**Vérification pour `W = 100`** (cas de référence) :

| Variante | `tipInVbX` | `centerVbX` | `offsetX` | `offsetY` |
|---|---|---|---|---|
| **center** | 50 | 50 | **0** | 12 |
| **left** | 15 | 75 | **+60** | 12 |
| **right** | 135 | 75 | **−60** | 12 |

**Vérification pour `W = 150`** :

| Variante | `tipInVbX` | `centerVbX` | `offsetX` | `offsetY` |
|---|---|---|---|---|
| **left** | 5 | 90 | **+85** | 12 |
| **right** | 175 | 90 | **−85** | 12 |

> ⚠️ **v1.1** : les valeurs `±60` de la v1.0 ne valent **que pour W = 100**. Toujours utiliser la formule générale.

**Interprétation :**

- **Center** : aucun décalage horizontal.
- **Left** : la pointe est à gauche du SVG → on pousse la bulle vers la **droite**.
- **Right** : la pointe est à droite du SVG → on pousse la bulle vers la **gauche**.

### 3.5 Application à chaque modification

L'offset doit être **recalculé et réappliqué** à chaque événement suivant :

1. Création du marker.
2. Changement de variante.
3. Changement de taille (voir §4).
4. Remplacement de l'étiquette (nouveau texte, nouvelle icône).

Ne jamais conserver un offset calculé pour une autre variante ou une autre échelle.

---

## 4. Mise à l'échelle des étiquettes

### 4.1 Objectif

Pouvoir afficher une même étiquette à différentes tailles sans dégradation visuelle et sans casser l'ancrage de la pointe.

### 4.2 Méthode obligatoire — redimensionnement vectoriel

Modifier les attributs `width` et `height` du SVG en conservant le `viewBox` inchangé.

```
Nouvelle largeur  = svgW    × scale
Nouvelle hauteur  = TOTAL_H × scale
viewBox           = inchangé
```

### 4.3 Méthode interdite — transformation CSS

Ne **jamais** utiliser :

```
element.style.transform = "scale(...)";
```

Cette méthode **rastérise** le SVG à sa taille d'origine, puis **étire l'image bitmap** obtenue.

### 4.4 Préservation de l'ancrage sous mise à l'échelle

Changer la taille du SVG **déplace la pointe** dans le marker. Deux actions **simultanées** sont requises :

1. Redimensionner le SVG (voir §4.2).
2. Recalculer et réappliquer l'offset (voir §3.3) avec le nouveau `scale`.

Omettre l'une des deux fait **dériver la pointe** par rapport à la coordonnée.

### 4.5 Plage recommandée

Facteur d'échelle utilisable : **0,4× à 4×** (40 % → 400 %).

### 4.6 Point d'ancrage du redimensionnement

Le redimensionnement doit **toujours se faire autour de la pointe** de l'étiquette. C'est ce que garantit la combinaison :

- redimensionnement vectoriel,
- recalcul d'offset.

---

## 5. Combinaison positionnement + mise à l'échelle

### 5.1 Formule complète

Pour intégrer une étiquette avec la variante `v`, la coordonnée `L`, et l'échelle `s` :

```
1. Lire les métadonnées du SVG : vbX, vbW, svgW, tipX.

2. Calculer la position de la pointe dans le viewBox :
   tipInVbX = tipX − vbX

3. Calculer l'offset :
   offsetX = (vbW / 2 − tipInVbX) × s
   offsetY = (TOTAL_H − TIP_Y) × s = 12 × s

4. Redimensionner le SVG :
   svgEl.setAttribute("width",  svgW    × s)
   svgEl.setAttribute("height", TOTAL_H × s)

5. Créer ou mettre à jour le marker :
   marker.setLngLat(L)
   marker.setOffset([offsetX, offsetY])
```

### 5.2 Propriétés à respecter

- L'offset doit être un **tableau de deux nombres**, en pixels.
- Le SVG doit **conserver son `viewBox` d'origine**.
- Le marker doit utiliser `anchor: 'bottom'`.
- L'élément DOM du marker **ne doit recevoir aucun `transform` CSS**.

---

## 6. Contraintes d'intégration avec Mapbox

### 6.1 Type de marker

```
new mapboxgl.Marker({
  element: customDiv,
  anchor: 'bottom',
  offset: [offsetX, offsetY],
  draggable: true | false
})
.setLngLat(lngLat)
.addTo(map);
```

### 6.2 Propriétés CSS obligatoires sur l'élément

- `pointer-events: auto`
- `cursor: grab` (ou autre)
- `overflow: visible` sur le SVG interne

### 6.3 Comportement en zoom/déplacement de la carte

- Le marker **suit la carte** automatiquement.
- L'offset reste **constant** tant que l'échelle de l'étiquette ne change pas.
- L'étiquette **ne change pas de taille** quand on zoome la carte.

### 6.4 Comportement en interaction

- **Drag du marker** : mettre à jour la coordonnée associée à l'étiquette.
- **Clic sur la carte** : si un mode « placer l'étiquette » est actif, déplacer le marker au point cliqué.
- **Changement de variante** : mettre à jour le SVG et recalculer l'offset.

---

## 7. Erreurs fréquentes à éviter

| Erreur | Conséquence | Correction |
|---|---|---|
| Utiliser `transform: scale()` pour redimensionner | Pixellisation à toutes les échelles | Modifier `width`/`height` du SVG |
| Ne pas recalculer l'offset après un changement de taille | La pointe dérive au zoom | Recalculer et réappliquer `offset` |
| Utiliser un offset uniquement vertical | La pointe gauche/droite ne tombe pas sur la coordonnée | Calculer `offsetX` et `offsetY` |
| Supposer `svgW = 150` pour tous les contenus | Ancrage faux pour `W > 120` | Utiliser `svgW = max(150, W + 30)` |
| Supposer `offsetX = ±60` | Ancrage faux pour `W ≠ 100` | Utiliser `±(W/2 + 10) × scale` |
| Appliquer un `transform` CSS sur l'élément du marker | Le marker ne suit plus la carte | Laisser Mapbox gérer le `transform` |
| Ne pas définir `pointer-events: auto` | Le marker n'est ni cliquable ni draggable | Définir explicitement `pointer-events` |
| Fournir un `svgW` erroné après modification du contenu | L'ancrage horizontal est faux | Relire `svgW` depuis le SVG à jour |
| Oublier le `viewBox` dans le SVG | Le contenu ne s'adapte pas au redimensionnement | Toujours conserver le `viewBox` |

---

## 8. Critères d'acceptation

### Positionnement

- [ ] La pointe de l'étiquette tombe **exactement** sur la coordonnée GPS, pour chaque variante.
- [ ] **Center** : la queue est centrée sous le panneau.
- [ ] **Left** : la queue part en oblique du coin inférieur gauche ; la bulle est décalée vers la droite.
- [ ] **Right** : la queue part en oblique du coin inférieur droit ; la bulle est décalée vers la gauche.
- [ ] La symétrie **left/right** est **exacte** (miroir horizontal).

### Mise à l'échelle

- [ ] Le redimensionnement se fait **sans pixellisation** à toutes les échelles.
- [ ] À toute échelle, **la pointe reste ancrée** sur la coordonnée.
- [ ] Le `viewBox` du SVG est **inchangé** après redimensionnement.
- [ ] L'offset est **recalculé** à chaque changement d'échelle.

### Comportement Mapbox

- [ ] Le marker suit la carte en déplacement et en zoom.
- [ ] Le marker est **draggable** (si configuré).
- [ ] Le changement de variante **met à jour** le SVG et l'offset simultanément.
- [ ] Aucun `transform` CSS externe n'est appliqué à l'élément du marker.
- [ ] Le SVG reçoit `overflow: visible` pour la queue et l'ombre.

### Robustesse

- [ ] Le système gère les trois variantes **sans code spécifique** autre que le calcul d'offset.
- [ ] Le système gère les changements d'échelle **sans reconstruire** le marker.
- [ ] Le système gère les changements de contenu (texte, icône) **en préservant l'ancrage**.
- [ ] Le système gère les libellés longs (`W > 120`) avec un `svgW` **dynamique**.

---

## 9. Résumé des formules

```
Constantes :
  TIP_Y   = 58
  TOTAL_H = 70
  PANEL_H = 34

Pour une étiquette donnée :
  W        = largeur naturelle du panneau
  total    = max(150, W + 30)         # variantes obliques
  tipInVbX = tipX − vbX

Offset à appliquer au marker :
  offsetX = (vbW / 2 − tipInVbX) × scale
  offsetY = 12 × scale

  # Simplification :
  #   center : offsetX = 0
  #   left   : offsetX = +(W/2 + 10) × scale
  #   right  : offsetX = −(W/2 + 10) × scale

Redimensionnement du SVG :
  svg.width  = svgW    × scale
  svg.height = TOTAL_H × scale
  (viewBox inchangé)

Ordre d'application à chaque changement :
  1. Redimensionner le SVG
  2. Recalculer offsetX et offsetY
  3. marker.setOffset([offsetX, offsetY])
```

---

## 10. Résumé du contrat

| Élément | Valeur |
|---|---|
| Type de marker | `mapboxgl.Marker` avec élément DOM personnalisé |
| Ancrage vertical | `anchor: 'bottom'` |
| Offset | `[offsetX, offsetY]` recalculé dynamiquement |
| Méthode de mise à l'échelle | Modifier `width`/`height` du SVG |
| Méthode interdite | `transform: scale()` |
| Point d'ancrage visuel | **La pointe de l'étiquette** |
| Point d'ancrage Mapbox | Bas-centre du DOM, corrigé par l'offset |
| Plage d'échelle | 0,4× → 4× |
| Recalcul d'offset | À chaque changement de variante ou d'échelle |
| `svgW` obliques | `max(150, W + 30)` (dynamique) |

---

*Fin du document — PRD Intégration Mapbox v1.1*