# Prompt de génération d'étiquettes SVG Mapbox

Documentation et template pour générer de nouvelles étiquettes cohérentes avec la collection existante.

---

## Template de prompt

```
En te basant strictement sur le PRD joint, génère-moi une étiquette SVG
conforme avec les caractéristiques suivantes :

- Texte     : {LIBELLÉ}
- Slug      : {slug-unique}
- Couleur   : {Famille Material} {Niveau}
- Icône     : {description de l'icône OU "aucune"}
- Texte     : {blanc / noir / auto}
- Variante  : {centre / gauche / droite / toutes}

Contraintes : respecte le PRD à la lettre, ne modifie ni la forme de la
bulle, ni les ombres, ni le dégradé. Livre uniquement le code SVG.
```

---

## Explication des champs

| Champ | Description | Exemple |
|---|---|---|
| **{LIBELLÉ}** | Texte affiché dans la bulle (en majuscules) | `FONTAINE` |
| **{slug-unique}** | Identifiant court, unique, en kebab-case — sert à suffixer les IDs SVG (`grad-`, `sh-`, `ground-`) pour éviter les collisions dans le DOM | `fontaine` |
| **{Famille Material} {Niveau}** | Couleur Material Design (famille + niveau). Les variantes claire et foncée sont dérivées automatiquement (±2 niveaux) | `Light Blue 700` |
| **{description de l'icône}** | Picto à afficher à gauche du texte, ou `aucune` | `une goutte d'eau blanche` |
| **{blanc / noir / auto}** | Couleur du texte. `auto` = déduit par le PRD selon la famille | `auto` |
| **{centre / gauche / droite / toutes}** | Position de la queue de la bulle. `toutes` = génère les 3 variantes empilées dans un seul SVG | `gauche` |

---

## Variantes de queue

### `centre`
Queue droite, centrée sous le panneau.

- viewBox : `0 0 W 70` (où `W` = largeur naturelle du panneau)
- Ellipse d'ombre : `cx = centre du panneau`, `cy = 58`

### `gauche`
Queue oblique partant du **coin inférieur gauche**.

- viewBox : `-15 0 svgW 70` où `svgW = max(150, W + 30)`
- Ellipse d'ombre : `cx = x_left − 10`, `cy = 60`
- **Doit être générée par symétrie exacte de la variante Droite**

### `droite`
Queue oblique partant du **coin inférieur droit**.

- viewBox : `-15 0 svgW 70` où `svgW = max(150, W + 30)`
- Ellipse d'ombre : `cx = x_right + 10`, `cy = 60`

### `toutes`
Génère un SVG unique contenant les **trois variantes empilées verticalement**, alignées sur leur pointe.

- Structure : chaque variante est un `<svg>` **imbriqué** (avec attributs `x`, `y`, `width`, `height`, `viewBox`) dans le SVG racine.
- **Pas de label textuel** à droite des bulles.
- `viewBox` du SVG racine **calculé dynamiquement** (voir PRD §17) :

```
minL  = min_v ( leftVp(v)  − tipVp(v) )
maxR  = max_v ( rightVp(v) − tipVp(v) )
width = ceil(maxR − minL)
height = 3 × 70 + 2 × 10 = 230
```

- Hauteur fixe : **230 px** (3 × 70 + 2 × 10 de gap).

---

## Exemple 1 — Étiquette simple (variante centre)

```
En te basant strictement sur le PRD joint, génère-moi une étiquette SVG
conforme avec les caractéristiques suivantes :

- Texte     : FONTAINE
- Slug      : fontaine
- Couleur   : Light Blue 700
- Icône     : une goutte d'eau blanche
- Texte     : auto
- Variante  : centre

Contraintes : respecte le PRD à la lettre, ne modifie ni la forme de la
bulle, ni les ombres, ni le dégradé. Livre uniquement le code SVG.
```

---

## Exemple 2 — Étiquette avec les 3 variantes empilées

```
En te basant strictement sur le PRD joint, génère-moi un fichier SVG unique
contenant les trois variantes empilées verticalement de l'étiquette
suivante :

- Texte     : COL
- Slug      : col
- Couleur   : Red 500
- Icône     : une montagne (trait blanc)
- Texte     : auto
- Variante  : toutes
```

---

## Exemple 3 — Variante d'une étiquette existante

```
En te basant strictement sur le PRD joint et sur l'étiquette ATTENTION
déjà générée, génère-moi la variante suivante :

- Texte     : ATTENTION
- Slug      : warn
- Couleur   : Amber 500
- Icône     : triangle noir avec point d'exclamation
- Texte     : noir
- Variante  : droite
```

---

## Prompt minimal (si vous êtes pressé)

```
PRD joint. Génère une étiquette : FONTAINE, Light Blue 700, icône goutte d'eau, variante gauche.
```

L'IA lira le PRD et appliquera automatiquement toutes les règles (forme, dégradé 3 stops, double ombre correctement positionnée, IDs suffixés, viewBox adapté à la variante, texte blanc/noir selon la famille).

---

## Garde-fous à ajouter (recommandé)

Pour éviter toute dérive stylistique, ajoutez cette phrase à la fin du prompt :

```
Ne change rien au style existant, respecte strictement le PRD.
Notamment : la symétrie exacte entre les variantes gauche et droite,
le positionnement de l'ellipse d'ombre sous la pointe, et le calcul
dynamique de svgW = max(150, W + 30) pour les variantes obliques.
```

---

## Points de vigilance

| Piège | Solution |
|---|---|
| IDs SVG non suffixés → dégradés qui se mélangent | Toujours suffixer par le slug + la variante (`grad-col-left`, `grad-col-right`) |
| Ombre de contact mal positionnée | Vérifier que `cx` de l'ellipse = `x` de la pointe (voir PRD §4.4) |
| Variante gauche décalée | La générer par miroir exact de la droite (`x_gauche = x_left + x_right − x_droite`) |
| Contour blanc décalé | Le path du contour doit être **strictement identique** à celui de la bulle remplie |
| Texte qui touche le bord | Prévoir une marge intérieure de 15 px de chaque côté |
| Couleur non Material | Vérifier dans la palette officielle avant de la proposer |
| `svgW` figé à 150 | Utiliser `max(150, W + 30)` — sinon, libellés longs débordent du viewBox |
| Offset X figé à ±60 | Utiliser `±(W/2 + 10) × scale` — sinon, dérive pour W ≠ 100 |
| Labels dans la variante `toutes` | Aucun label textuel : SVG imbriqués uniquement (cf. PRD §17) |