# PRD — Générateur d'étiquettes SVG pour trace GPX sur Mapbox

**Version** : 3.1
**Portée** : Étiquettes ponctuelles affichées sur une carte Mapbox satellite 3D

---

## Changelog v3.0 → v3.1

| Domaine | Modification |
|---|---|
| **svgW obliques** | Calcul **dynamique** : `svgW = max(150, W + 30)`. Le tableau à valeurs fixes (`150`, `tipInVbX = 15/135`) ne s'applique qu'à `W ≤ 120`. |
| **offsetX oblique** | Formule générale `±(W/2 + 10) × scale`. Les `±60` documentés en v3.0 ne valent que pour `W = 100`. |
| **Règle texte clair** | Extension à **Blue Grey 100–300** (en plus des familles déjà listées). |
| **PANEL_H** | Constante `34` désormais explicitée en §3.2. |
| **Variante "toutes"** | Pas de labels textuels à droite des bulles ; SVG imbriqués (`<svg>` dans `<svg>`), viewBox calculé dynamiquement. |

## Changelog v2.0 → v3.0

| Domaine | Modification |
|---|---|
| **Slug** | Enrichi avec couleur et niveau (`col-red-500-center` au lieu de `col-center`) |
| **Ancrage pointe** | Variantes gauche/droite ancrées précisément sur la pointe (offset X et Y) |
| **Zoom** | Facteur vectoriel 0,4× → 4× (net + ancrage préservé) |
| **Pin centre** | Base affinée : 14 px au lieu de 20 px |
| **Intégration** | Mapbox GL JS v3 + terrain 3D + icônes MDI + auto-détection fond |

---

## 1. Contexte

Application de visualisation de trace GPX sur une carte Mapbox en vue satellite 3D. Des **étiquettes ponctuelles** sont affichées sur des points remarquables du parcours (départ, col, arrivée, km, consignes de navigation, points d'intérêt).

Chaque étiquette est un **SVG autonome** inséré soit :

- dans un `mapboxgl.Marker` (mode Mapbox satellite + terrain 3D),
- soit dans un `<div>` en position absolue (mode fallback image / procédural).

---

## 2. Objectif du présent document

Permettre la génération de **nouvelles étiquettes** en respectant scrupuleusement les caractéristiques visuelles et techniques des étiquettes existantes, afin de garantir une **cohérence graphique** sur l'ensemble de la carte, quelle que soit la variante et le facteur de zoom.

---

## 3. Format visuel — Bulle type "speech bubble"

Chaque étiquette est une **bulle unique** composée d'un seul `<path>` combinant :

- Un **rectangle arrondi** (rayon `8px`) contenant l'icône et le texte.
- Une **queue oblique** qui pointe vers la coordonnée GPS.
- La queue est **intégrée au path** (pas de triangle séparé) → contour continu et propre.

### 3.1 Trois variantes de positionnement de la queue

| Variante | Description | Usage |
|---|---|---|
| **Centre** | Queue centrée sous le panneau, base étroite (14 px) | Par défaut, point isolé |
| **Gauche** | Queue oblique partant du coin inférieur gauche | Décaler la bulle vers la droite |
| **Droite** | Queue oblique partant du coin inférieur droit | Décaler la bulle vers la gauche |

La variante **Gauche** est obtenue par **symétrie horizontale exacte** de la variante Droite :

```
x_gauche = x_left + x_right − x_droite
```

### 3.2 Dimensions de référence

| Élément | Constante | Valeur |
|---|---|---|
| Hauteur du panneau | `PANEL_H` | **34 px** (de `y=6` à `y=40`) |
| Position haut panneau | `PANEL_TOP` | 6 |
| Position bas panneau | `PANEL_BOTTOM` | 40 |
| Position pointe | `TIP_Y` | 58 |
| Hauteur totale | `TOTAL_H` | 70 |
| Rayon des coins (sauf coin avec queue) | `PANEL_RADIUS` | 8 px |
| Longueur de la queue | — | ~18 px (pointe à `y=58`) |
| **Largeur base queue — variante centre** | — | **14 px** (`cx−7` à `cx+7`) |
| **Largeur base queue — variantes obliques** | — | 18 px |
| Marge intérieure | `PANEL_PAD` | ~15 px de chaque côté |
| viewBox élargi (variantes obliques) | — | `-15 0 {svgW} 70` |

### 3.3 Gabarit de path — Variante CENTRE

```
M {x_left+8} 6
L {x_right-8} 6
Q {x_right} 6 {x_right} 14
L {x_right} 32
Q {x_right} 40 {x_right-8} 40
L {cx+7} 40
L {cx} 58
L {cx-7} 40
L {x_left+8} 40
Q {x_left} 40 {x_left} 32
L {x_left} 14
Q {x_left} 6 {x_left+8} 6
Z
```

> ⚠️ **v3.0** : la base est `±7` (au lieu de `±10` en v2.0), pour un rendu plus fin.

### 3.4 Gabarit de path — Variante DROITE (oblique)

```
M {x_left+8} 6
L {x_right-8} 6
Q {x_right} 6 {x_right} 14
L {x_right} 32
Q {x_right} 40 {x_right-8} 40
L {x_right+10} 58
L {x_right-18} 40
L {x_left+8} 40
Q {x_left} 40 {x_left} 32
L {x_left} 14
Q {x_left} 6 {x_left+8} 6
Z
```

### 3.5 Gabarit de path — Variante GAUCHE (miroir exact de Droite)

```
M {x_right-8} 6
L {x_left+8} 6
Q {x_left} 6 {x_left} 14
L {x_left} 32
Q {x_left} 40 {x_left+8} 40
L {x_left-10} 58
L {x_left+18} 40
L {x_right-8} 40
Q {x_right} 40 {x_right} 32
L {x_right} 14
Q {x_right} 6 {x_right-8} 6
Z
```

---

## 4. Style visuel

### 4.1 Remplissage

**Dégradé vertical** à 3 stops (effet 3D bombé) :

```xml
<linearGradient id="grad-{fullSlug}" x1="0" y1="0" x2="0" y2="1">
  <stop offset="0%"   stop-color="{couleur_claire}"/>
  <stop offset="45%"  stop-color="{couleur_principale}"/>
  <stop offset="100%" stop-color="{couleur_foncee}"/>
</linearGradient>
```

### 4.2 Contour

Liseré blanc semi-transparent appliqué **en overlay** :

```xml
<path d="..." fill="none" stroke="#FFFFFF" stroke-width="1.5" opacity="0.9"/>
```

### 4.3 Reflet supérieur

```xml
<rect x="{x_left+8}" y="8" width="{largeur_panneau-16}" height="2.5"
      rx="1.25" fill="#FFFFFF" opacity="0.35"/>
```

### 4.4 Ombre (obligatoire)

**Double ombre** :

**a) Ombre de contact** — ellipse floue centrée sous la pointe

```xml
<filter id="ground-{fullSlug}" x="-100%" y="-100%" width="300%" height="300%">
  <feGaussianBlur stdDeviation="2.5"/>
</filter>
<ellipse cx="{tip_x}" cy="{ellipse_cy}" rx="10" ry="2.5" fill="#000000"
         opacity="0.4" filter="url(#ground-{fullSlug})"/>
```

**Position de l'ellipse selon la variante** :

| Variante | `cx` de l'ellipse | `cy` |
|---|---|---|
| Centre | `cx = cx` (centre du panneau) | 58 |
| Gauche | `cx = x_left − 10` | 60 |
| Droite | `cx = x_right + 10` | 60 |

**b) Ombre portée** — appliquée au `<g>` englobant la bulle

```xml
<filter id="sh-{fullSlug}" x="-30%" y="-30%" width="160%" height="180%">
  <feDropShadow dx="1.5" dy="3.5" stdDeviation="2.5"
                flood-color="#000000" flood-opacity="0.5"/>
</filter>
```

> ⚠️ Le `dx` de l'ombre portée reste **identique dans les trois variantes**.

### 4.5 Texte

```xml
<text x="{cx_texte}" y="28"
      text-anchor="middle"
      font-family="Arial, sans-serif"
      font-size="13"
      font-weight="bold"
      fill="{couleur_texte}"
      style="text-shadow: 0 1px 1px rgba(0,0,0,0.5)">{LIBELLÉ}</text>
```

Sur fond clair → texte noir `#212121` et `text-shadow` blanc.

### 4.6 Icône (optionnelle)

Placée à gauche du texte, centrée verticalement sur le panneau (zone `y ∈ [14, 32]`).

- Style **trait blanc** : `stroke="#FFFFFF"`, `fill="none"`, `stroke-width="2"`, `stroke-linecap="round"`
- Ou **remplissage blanc** pour les pictos pleins
- **v3.0** : peuvent provenir de la bibliothèque **MDI** (voir §14)

---

## 5. Zoom vectoriel de l'étiquette (v3.0)

Chaque étiquette peut être affichée à une échelle **0,4× → 4×** (40 % → 400 %) sans perte de netteté.

### 5.1 Principe — Redessin vectoriel, pas d'étirement bitmap

Le facteur de zoom **modifie les attributs `width` et `height` du SVG**, en conservant le `viewBox`.

```javascript
// ✅ Correct — redessin vectoriel
svgEl.setAttribute('width',  D.svgW  * scale);
svgEl.setAttribute('height', TOTAL_H * scale);
// Le viewBox reste inchangé

// ❌ À NE JAMAIS FAIRE — rastérisation puis étirement
svgEl.style.transform = `scale(${scale})`;
```

### 5.2 Préservation de l'ancrage de la pointe

| Grandeur | Calcul |
|---|---|
| Position pointe X dans le SVG rendu | `(tipX − vbX) × scale` |
| Position pointe Y dans le SVG rendu | `TIP_Y × scale` = `58 × scale` |
| Largeur rendue du SVG | `svgW × scale` |
| Hauteur rendue du SVG | `70 × scale` |

### 5.3 Contraintes

- Le zoom est **purement visuel** — le SVG exporté n'est **pas** modifié.
- L'aperçu dans la sidebar reste **toujours à 100 %**.
- Le curseur de zoom n'est actif qu'en **mode Carte**.

---

## 6. Structure SVG type (ordre des couches)

```
1. <defs>
   ├── linearGradient grad-{fullSlug}
   ├── filter sh-{fullSlug}        (ombre portée)
   └── filter ground-{fullSlug}    (ombre de contact)

2. <ellipse>  ombre de contact au sol (cx selon variante)

3. <g filter="url(#sh-{fullSlug})">
   └── <path>  bulle remplie (path selon variante)

4. <path>  contour blanc (overlay, MÊME path que la bulle)

5. <rect>  reflet haut

6. <path> / <rect> / <circle>  icône (optionnel)

7. <text>  libellé
```

> ⚠️ **Critique** : le path du **contour blanc** doit être **strictement identique** à celui de la bulle remplie.

---

## 7. Conventions de nommage — Slug enrichi (v3.0)

### 7.1 Structure du slug

```
{base}-{famille-couleur}-{niveau}-{variante}
```

Exemples :
- `col-red-500-center`
- `warn-amber-500-right`
- `km-10-blue-grey-500-left`
- `start-green-500-all`

### 7.2 Composants

| Composant | Description | Exemple |
|---|---|---|
| **Base** | Identifiant court, unique et lisible | `col`, `warn`, `km-10`, `start` |
| **Famille-couleur** | Famille Material Design en minuscules | `red`, `amber`, `blue-grey` |
| **Niveau** | Niveau Material Design (100-900) | `300`, `500`, `700` |
| **Variante** | Variante de queue | `center`, `left`, `right`, `all` |

### 7.3 Règles de base

- En **minuscules**
- **Sans accents** ni caractères spéciaux
- Avec des **tirets** au lieu d'espaces
- **Court** et sans ambiguïté

### 7.4 IDs SVG

Toujours **suffixés par le slug complet** pour éviter les collisions :

```
grad-{fullSlug}, sh-{fullSlug}, ground-{fullSlug}
```

### 7.5 Attention critique

Le slug complet doit être **unique par instance**. Deux étiquettes peuvent partager base + variante si la couleur diffère :

```
col-red-500-center
col-blue-500-center   ← mêmes base + variante, couleur différente
```

---

## 8. Ancrage de la pointe — Intégration Mapbox (v3.0)

### 8.1 Problématique

Un marker Mapbox est ancré par son **bas-centre** (`anchor: 'bottom'`). Or la pointe d'une étiquette gauche/droite est **décalée horizontalement** dans le SVG. Un simple offset vertical ne suffit donc pas.

### 8.2 Notations et formules générales

Soit `W` la **largeur naturelle du panneau** (calculée selon le contenu texte + icône).

Pour chaque variante :

| Variante | `vbX` | `vbW` | `svgW` | `xL` | `xR` | `tipX` |
|---|---|---|---|---|---|---|
| **center** | `0` | `W` | `W` | `0` | `W` | `W/2` |
| **left**  | `−15` | `max(150, W+30)` | `max(150, W+30)` | `−15 + (total−W)/2` | `xL + W` | `xL − 10` |
| **right** | `−15` | `max(150, W+30)` | `max(150, W+30)` | `−15 + (total−W)/2` | `xL + W` | `xR + 10` |

> ⚠️ **v3.1** : pour `W ≤ 120`, `total = 150` (valeur historique). Au-delà, `total = W + 30`, sinon le viewBox serait trop étroit pour contenir le panneau et les débords de pointe.

### 8.3 Calcul de l'offset

```javascript
function computeBubbleOffset(D, scale){
  const tipInVbX = D.tipX - D.vbX;       // position pointe en coord. viewBox
  const centerVbX = D.vbW / 2;           // centre du viewBox
  const offsetX = (centerVbX - tipInVbX) * scale;
  const offsetY = (TOTAL_H - TIP_Y) * scale;  // 12 × scale
  return [offsetX, offsetY];
}
```

### 8.4 Simplification

La formule ci-dessus donne, pour chaque variante :

| Variante | `offsetX` (à `scale = 1`) | `offsetY` |
|---|---|---|
| **center** | `0` | `12` |
| **left**  | `+(W/2 + 10)` | `12` |
| **right** | `−(W/2 + 10)` | `12` |

**Vérification pour `W = 100`** (cas de référence) :
- center → `offsetX = 0`
- left → `offsetX = +60`
- right → `offsetX = −60`

**Vérification pour `W = 150`** :
- left → `offsetX = +85`
- right → `offsetX = −85`

> ⚠️ **Correction v3.1** : les valeurs `±60` documentées en v3.0 ne valent **que pour W = 100**. La formule générale est `±(W/2 + 10) × scale`.

### 8.5 Application dynamique

L'offset doit être **recalculé** :

1. à la création du marker,
2. à chaque modification d'étiquette (texte, couleur, icône…),
3. à chaque changement de variante,
4. à chaque mouvement du curseur de zoom.

```javascript
bubbleMarker.setOffset(computeBubbleOffset(D, bubbleScale));
```

---

## 9. Charte couleur — Material Design uniquement

Toutes les couleurs doivent appartenir à la palette **Material Design** (<https://m2.material.io/design/color/the-color-system.html>).

### 9.1 Règle de dérivation

- Variante claire → niveau **-2** (500 → 300, 700 → 500, 900 → 700)
- Variante foncée → niveau **+2** (300 → 500, 500 → 700, 700 → 900)

Le dégradé vertical suit l'ordre : **claire (0%) → principale (45%) → foncée (100%)**.

### 9.2 Couleurs par défaut (déjà validées)

| Type | Principal | Clair | Foncé | Texte |
|---|---|---|---|---|
| Départ | Green 500 `#4CAF50` | Green 300 `#81C784` | Green 700 `#388E3C` | blanc |
| Col | Red 500 `#F44336` | Red 300 `#E57373` | Red 700 `#D32F2F` | blanc |
| Arrivée | Blue 500 `#2196F3` | Blue 300 `#64B5F6` | Blue 700 `#1976D2` | blanc |
| Km | Blue Grey 500 `#607D8B` | Blue Grey 300 `#90A4AE` | Blue Grey 700 `#455A64` | blanc |
| Attention | Amber 500 `#FFC107` | Amber 300 `#FFD54F` | Amber 700 `#FFA000` | noir |
| Direction | Light Blue 500 `#03A9F4` | Light Blue 300 `#4FC3F7` | Light Blue 700 `#0288D1` | blanc |
| Stop | Red 700 `#D32F2F` | Red 500 `#F44336` | Red 900 `#B71C1C` | blanc |

### 9.3 Couleurs fixes (non négociables)

| Usage | Couleur Material |
|---|---|
| Contour panneau | White `#FFFFFF` (opacité 0.9) |
| Reflet supérieur | White `#FFFFFF` (opacité 0.35) |
| Ombre portée & contact | Black `#000000` |
| Texte sur fond clair | Grey 900 `#212121` |
| Texte sur fond foncé | White `#FFFFFF` |

### 9.4 Nouvelles étiquettes

Toute nouvelle couleur doit être une couleur **Material Design existante**. La variante claire et foncée sont dérivées automatiquement (±2 niveaux).

### 9.5 Règle de décision du texte (v3.1)

**Texte Grey 900 `#212121`** si :

- famille ∈ **{Yellow, Amber, Lime, Light Green}**, **ou**
- famille ∈ **{Grey, Blue Grey}** **et** niveau ≤ 300.

**Texte White `#FFFFFF`** dans tous les autres cas.

> ⚠️ **v3.1** : extension à **Blue Grey 100–300** (auparavant limitée à Grey).

---

## 10. Contraintes techniques

- **SVG autonome** : doit fonctionner seul, sans CSS externe.
- **Dimensions cohérentes** :
  - Variante **centre** : `width = W`, `height = 70`, `viewBox = 0 0 W 70`
  - Variantes **obliques** : `width = svgW`, `height = 70`, `viewBox = -15 0 svgW 70` (où `svgW = max(150, W + 30)`)
- **`viewBox`** : toujours défini, ratio identique aux `width`/`height`.
- **Attribut `preserveAspectRatio="xMinYMin meet"`**.
- **Attribut `shape-rendering="geometricPrecision"`**.
- **Aucune dépendance externe**.

---

## 11. Intégration Mapbox

### 11.1 JavaScript

```javascript
function createBubbleMarker(svgString, coordinates, variant = 'center') {
  const el = document.createElement('div');
  el.className = 'mapbox-bubble-marker';
  el.innerHTML = svgString;

  const D = buildVariantData(/* … */, variant);
  const offset = computeBubbleOffset(D, 1);

  return new mapboxgl.Marker({
    element: el,
    anchor: 'bottom',
    offset: offset,
    draggable: true
  })
    .setLngLat(coordinates)
    .addTo(map);
}
```

### 11.2 CSS

```css
.mapbox-bubble-marker svg {
  display: block;
  overflow: visible;
}

.mapbox-bubble-marker {
  pointer-events: auto;
  cursor: grab;
}
```

### 11.3 Terrain 3D + altitude

```javascript
map.addSource('mapbox-dem', {
  type: 'raster-dem',
  url: 'mapbox://mapbox.mapbox-terrain-dem-v1',
  tileSize: 512,
  maxzoom: 14
});
map.setTerrain({ source: 'mapbox-dem', exaggeration: 1.4 });

const elev = map.queryTerrainElevation(lngLat, { exaggerated: false });
```

---

## 12. Critères d'acceptation

- [ ] La bulle est un **path unique** avec queue intégrée.
- [ ] Le **dégradé vertical** à 3 stops est présent.
- [ ] Le **contour blanc** en overlay est présent (path identique à la bulle).
- [ ] Le **reflet supérieur** est présent.
- [ ] La **double ombre** (contact + portée) est présente.
- [ ] L'**ellipse d'ombre de contact** est centrée sous la **pointe**.
- [ ] Le `viewBox` fait **70 px de haut**.
- [ ] Pour les variantes obliques : `viewBox="-15 0 {svgW} 70"` avec `svgW = max(150, W + 30)`.
- [ ] Les **IDs SVG** sont uniques (suffixés par le slug complet).
- [ ] L'étiquette s'affiche correctement **sur fond satellite**.
- [ ] L'intégration Mapbox fonctionne avec `anchor: 'bottom'` et l'**offset adapté à la variante**.
- [ ] Les couleurs appartiennent à la **palette Material Design**.
- [ ] La variante **Gauche** est le **miroir horizontal exact** de la variante **Droite**.
- [ ] Le **zoom** (0,4× → 4×) préserve la **netteté** et l'**ancrage de la pointe**.
- [ ] Le **slug enrichi** respecte le format `{base}-{famille}-{niveau}-{variante}`.

---

## 13. Template de prompt

### 13.1 Modèle générique

```
En te basant strictement sur le PRD joint, génère-moi une étiquette SVG
conforme avec les caractéristiques suivantes :

- Texte     : {LIBELLÉ}
- Slug base : {base-unique}
- Couleur   : {Famille Material} {Niveau}
- Icône     : {description de l'icône OU "aucune"}
- Texte     : {blanc / noir / auto}
- Variante  : {centre / gauche / droite / toutes}

Contraintes : respecte le PRD à la lettre, ne modifie ni la forme de la
bulle, ni les ombres, ni le dégradé. Livre uniquement le code SVG.
```

### 13.2 Variantes demandées

| Valeur | Description |
|---|---|
| `centre` | Queue centrée sous le panneau (défaut) |
| `gauche` | Queue oblique partant du coin inférieur gauche |
| `droite` | Queue oblique partant du coin inférieur droit |
| `toutes` | Générer les trois variantes dans un même fichier |

### 13.3 Exemple 1 — Fontaine (variante centre)

```
En te basant strictement sur le PRD joint, génère-moi une étiquette SVG
conforme avec :

- Texte     : FONTAINE
- Slug base : fontaine
- Couleur   : Light Blue 700
- Icône     : une goutte d'eau blanche
- Texte     : auto
- Variante  : centre
```

### 13.4 Exemple 2 — Col (les trois variantes empilées)

```
En te basant strictement sur le PRD joint, génère-moi une étiquette SVG
conforme avec :

- Texte     : COL
- Slug base : col
- Couleur   : Red 500
- Icône     : une montagne (trait blanc)
- Texte     : auto
- Variante  : toutes (centre + gauche + droite, empilées verticalement
              dans un seul SVG de démonstration)
```

### 13.5 Exemple 3 — Attention (variante droite)

```
En te basant strictement sur le PRD joint et sur l'étiquette ATTENTION
déjà générée, génère-moi la variante suivante :

- Texte     : ATTENTION
- Slug base : warn
- Couleur   : Amber 500
- Icône     : triangle noir avec point d'exclamation
- Texte     : noir
- Variante  : droite
```

### 13.6 Prompt minimal

```
PRD joint. Génère une étiquette : FONTAINE, Light Blue 700, icône
goutte d'eau, variante gauche.
```

---

## 14. Bibliothèque d'icônes MDI (v3.0)

Les icônes peuvent être :

1. **Locales** (favoris embarqués).
2. **MDI** (Material Design Icons) — chargées depuis
   `https://cdn.jsdelivr.net/npm/@mdi/svg@latest/svg/{nom}.svg`.

### 14.1 Recherche

Un index curated + un mapping mots-clés → icônes permet la recherche par nom, tag ou mot-clé FR/EN.

### 14.2 Styles d'icônes MDI

Les icônes MDI sont **remplies** par défaut. Pour un rendu **trait** :

```xml
<path d="..." fill="none" stroke="{C}" stroke-width="2"
      stroke-linejoin="round" stroke-linecap="round"/>
```

Le `<path>` MDI brut est utilisé, seul le style change.

---

## 15. Astuces d'usage

1. **Conserver ce PRD** dans un fichier versionné.
2. **Le joindre systématiquement** à chaque nouvelle demande.
3. **Ajouter une phrase de garde-fou** : *« Ne change rien au style existant, respecte strictement le PRD. »*
4. **Choisir la variante selon le contexte** :
   - `centre` : point isolé.
   - `gauche` / `droite` : éviter les chevauchements.
5. **Utiliser le zoom** pour vérifier la lisibilité sur fond satellite.

---

## 16. Règle de symétrie (obligatoire)

Pour garantir une **symétrie parfaite** entre les variantes Gauche et Droite, la variante Gauche **doit être générée par miroir horizontal** de la variante Droite :

```
x_gauche = x_left + x_right − x_droite
```

Cette règle s'applique :

- Au path de la bulle.
- Au `cx` de l'ellipse d'ombre de contact.
- À la position de l'icône et du texte (déjà centrés).
- **Au calcul de l'offset Mapbox** — le signe de `offsetX` s'inverse naturellement.

La variante Gauche ne doit **jamais** être construite manuellement à partir de la variante Centre.

---

## 17. Variante "toutes" — Rendu attendu (v3.1)

La variante `toutes` produit un **SVG unique** contenant les trois variantes **empilées verticalement**, alignées sur leur **pointe** respective.

### 17.1 Structure

- Chaque variante est un `<svg>` **imbriqué** dans le SVG racine, avec un attribut `x/y` propre.
- **Pas de label textuel** à droite des bulles (contrairement à ce qu'indiquaient les anciennes versions du Prompt.md).
- Le `viewBox` du SVG racine est **calculé dynamiquement** :

```
Soit, pour chaque variante v :
  tipVp(v)   = tipX(v) − vbX(v)
  leftVp(v)  = min(xL(v) − 3, tipX(v) − 22) − vbX(v)
  rightVp(v) = max(xR(v) + 10, tipX(v) + 22) − vbX(v)

minL  = min_v ( leftVp(v)  − tipVp(v) )
maxR  = max_v ( rightVp(v) − tipVp(v) )
width = ceil(maxR − minL)
shift = −minL
```

Chaque variante est ensuite positionnée à `x = shift − tipVp(v)`.

### 17.2 Dimensions

- **Largeur** : variable selon le contenu (typiquement ~200 px pour `COL` avec icône).
- **Hauteur** : `3 × 70 + 2 × 10 = 230` px (gap = 10 px entre variantes).
- **viewBox** : `0 0 {width} 230`.

---

*Fin du document — PRD v3.1*