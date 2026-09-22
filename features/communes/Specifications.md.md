# Spécifications — Communes GPX

**Détection des communes traversées par une trace GPX**

| | |
|---|---|
| **Version** | 1.0 |
| **Statut** | Draft |
| **Date** | 2026-09-18 |
| **Type** | Application web locale, mono-fichier |
| **Public cible** | Randonneurs, cyclotouristes, organisateurs, analystes SIG |

---

## 1. Spécification fonctionnelle

### 1.1 Import

| ID | Spécification |
|---|---|
| **SF-I-01** | L'utilisateur peut déposer un fichier `.gpx` par glisser-déposer dans la zone dédiée. |
| **SF-I-02** | L'utilisateur peut cliquer sur la zone pour ouvrir le sélecteur de fichier natif. |
| **SF-I-03** | Le fichier doit avoir l'extension `.gpx`, sinon une erreur est affichée. |
| **SF-I-04** | Le fichier doit contenir au moins 2 points valides (lat/lon numériques, hors `(0,0)`). |
| **SF-I-05** | Les points `<trkpt>`, `<rtept>` et `<wpt>` sont supportés (priorité trkpt > rtept > wpt). |

### 1.2 Analyse

| ID | Spécification |
|---|---|
| **SF-A-01** | La trace est simplifiée avec une tolérance de 5 m avant analyse. |
| **SF-A-02** | La bounding box est élargie de 0.01° (~1 km) autour de la trace simplifiée. |
| **SF-A-03** | Une requête Overpass récupère les relations `boundary=administrative`, `admin_level=8` dans la bbox. |
| **SF-A-04** | En cas d'échec, la requête bascule automatiquement sur 3 miroirs alternatifs. |
| **SF-A-05** | Les communes non intersectantes sont éliminées via `booleanIntersects`. |
| **SF-A-06** | Les polygones dont la surface dépasse 10× la médiane sont exclus. |
| **SF-A-07** | La trace est échantillonnée à pas de 100 m minimum (2000 échantillons max). |
| **SF-A-08** | En cas de chevauchement, la commune dont le centroïde est le plus proche gagne. |
| **SF-A-09** | Les points d'entrée/sortie sont affinés par `lineIntersect` avec la frontière communale. |
| **SF-A-10** | Les km sont calculés par interpolation locale entre les échantillons encadrants. |
| **SF-A-11** | Les passages < 15 m sont exclus. |

### 1.3 Restitution

| ID | Spécification |
|---|---|
| **SF-R-01** | Un résumé affiche : nombre de passages, communes distinctes, km cumulés. |
| **SF-R-02** | Un mini-graphique affiche une barre horizontale par passage. |
| **SF-R-03** | Les barres sont positionnées en pourcentage de la longueur totale. |
| **SF-R-04** | Les barres ont des couleurs déterministes (`hsl((i × 47) % 360, ...)`). |
| **SF-R-05** | Un tableau chronologique liste les passages triés par km d'entrée. |
| **SF-R-06** | Le tableau affiche : `Km entrée`, `Commune`, `Km sortie`, `Longueur`. |
| **SF-R-07** | Un badge `passage N/M` apparaît pour les passages multiples. |
| **SF-R-08** | La console affiche la timeline compacte pour diagnostic. |

### 1.4 Export

| ID | Spécification |
|---|---|
| **SF-E-01** | Un bouton `Télécharger CSV` génère un fichier CSV. |
| **SF-E-02** | Le CSV contient : `Km_entree`, `Commune`, `Passage`, `Nb_passages_total`, `Km_sortie`, `Longueur_km`. |
| **SF-E-03** | Un bouton `Télécharger JSON` génère un fichier JSON. |
| **SF-E-04** | Le JSON est un tableau d'objets `{ commune, passage, nb_passages_total, km_entree, km_sortie, longueur_km }`. |
| **SF-E-05** | Aucune géométrie n'est incluse dans les exports. |
| **SF-E-06** | Encodage UTF-8, séparateur virgule, guillemets sur les champs texte. |

---

## 2. Spécification technique

### 2.1 Stack

| Composant | Version | Rôle |
|---|---|---|
| HTML5 | — | Structure |
| CSS3 | — | Style (inline dans le `<style>`) |
| JavaScript ES2020+ | — | Logique (inline dans le `<script>`) |
| [Turf.js](https://turfjs.org/) | 6.x | Géométrie |
| [osmtogeojson](https://github.com/tyrasd/osmtogeojson) | 3.0.0-beta.5 | Conversion OSM |
| API Overpass | v1 | Récupération des relations OSM |

### 2.2 Contraintes de performance

| Métrique | Cible |
|---|---|
| Temps de parsing GPX (500 points) | < 100 ms |
| Temps de simplification | < 50 ms |
| Temps d'attente Overpass | 1-30 s (variable) |
| Temps d'analyse locale | < 500 ms |
| Temps de rendu | < 100 ms |

### 2.3 Contraintes techniques

- **Aucun backend** : tout côté client.
- **Aucun stockage persistant** : pas de `localStorage`, pas de cookies.
- **Aucune télémétrie** : aucune donnée envoyée à un serveur tiers (sauf Overpass pour la requête de bbox).
- **Mono-fichier** : un seul `.html`, prêt à être sauvegardé et partagé.

---

## 3. Spécification des données

### 3.1 Format d'entrée (GPX)

Le fichier doit être un XML valide conforme à GPX 1.1, avec au minimum :

```xml
<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="...">
  <trk>
    <trkseg>
      <trkpt lat="48.8639" lon="2.8159"><ele>114.69</ele></trkpt>
      ...
    </trkseg>
  </trk>
</gpx>
```

Extraction :
- `<trkpt>` prioritaires.
- `<rtept>` en fallback si < 2 `<trkpt>`.
- `<wpt>` en dernier recours.
- `<ele>` et `<time>` ignorés (v1.0).

### 3.2 Format de sortie (JSON)

```json
[
  {
    "commune": "Nice",
    "passage": 1,
    "nb_passages_total": 1,
    "km_entree": 0,
    "km_sortie": 5.131,
    "longueur_km": 5.131
  }
]
```

### 3.3 Format de sortie (CSV)

```csv
"Km_entree","Commune","Passage","Nb_passages_total","Km_sortie","Longueur_km"
"0.000","Nice","1","1","5.131","5.131"
"5.131","La Trinité","1","1","8.000","2.870"
```

---

## 4. Spécification de l'interface

### 4.1 Zone de dépôt

- Bordure pointillée 3px.
- Fond blanc, texte gris.
- Effet visuel au survol (bordure bleue, fond bleu très clair).
- Contient un `<input type="file">` masqué.

### 4.2 Barre de progression

- Hauteur 6px, fond gris clair, remplissage bleu.
- Largeur en pourcentage (0 à 100).
- Visible uniquement pendant l'analyse.

### 4.3 Statut

- Texte en haut, sous la zone de dépôt.
- Couleurs sémantiques :
    - `info` (bleu) : en cours
    - `success` (vert) : terminé
    - `warn` (orange) : attention
    - `error` (rouge) : échec

### 4.4 Résumé

- Encadré blanc bordé.
- Ligne 1 : nombre de passages, de communes, km cumulés.
- Ligne 2 : statistiques OSM (relations reçues, retenues).

### 4.5 Mini-graphique

- Chaque ligne : nom de commune (155 px), axe horizontal (flexible), km (65 px).
- Barre colorée positionnée en `%` de la longueur totale.
- Infobulle au survol (attribut `title`).

### 4.6 Tableau

- En-tête gris clair.
- Lignes alternées au survol.
- Colonnes : `Km entrée`, `Commune`, `Km sortie`, `Longueur`.
- Badges bleus pour les passages multiples.

### 4.7 Boutons d'export

- Bouton principal bleu pour CSV.
- Bouton secondaire gris pour JSON.

---

## 5. Spécification des messages d'erreur

| Situation | Message |
|---|---|
| Fichier non `.gpx` | "Veuillez fournir un fichier `.gpx`." |
| GPX mal formé | "Fichier GPX illisible (XML invalide)." |
| Moins de 2 points | "Trace vide ou invalide (moins de 2 points)." |
| Aucune commune dans la bbox | "Aucune commune trouvée dans la zone de la trace." |
| Aucune commune intersectante | "Aucune commune traversée détectée." |
| Tous les miroirs Overpass échouent | "Tous les serveurs Overpass ont échoué. Dernière erreur : [détail]." |

---

## 6. Spécification des logs de diagnostic

Format : `[GPX] <message>` avec couleur bleue (`#2b6cb0`).

| Étape | Log |
|---|---|
| Import | `Fichier reçu : <nom> (<taille> octets)` |
| Parsing | `Points extraits : <n>` |
| Simplification | `Longueur : <km> km \| BBox : [<...>]` |
| Requête | `Overpass → <url>` |
| Réponse | `OK <url> <ms> ms` |
| OSM | `Éléments OSM reçus : <n>` |
| Filtrage | `Communes intersectant la trace : <n>` |
| Surface | `Surface médiane des candidates : <m²>` |
| Enrichi | `Candidates :` puis ligne par ligne |
| Échantillonnage | `Échantillons : <n> \| pas = <km>` |
| Timeline | `Timeline : <chaîne>` |
| Chevauchement | `Chevauchements : <n> — sans match : <n>` |
| Départ/Arrivée | `Point de départ dans : <nom>` |
| Filtrage | `Segment ignoré (< 15 m) dans <commune> : <km>` |

---

## 7. Spécification des exports

### 7.1 CSV

- Nom : `communes.csv`
- MIME : `text/csv;charset=utf-8;`
- Séparateur : `,`
- Encodage : UTF-8
- Guillemets : systématiques sur tous les champs
- En-tête obligatoire

### 7.2 JSON

- Nom : `communes.json`
- MIME : `application/json`
- Indentation : 2 espaces
- Tableau racine (pas d'objet englobant)
- Aucune géométrie

---

## 8. Tests et validation

### 8.1 Jeu de tests

| Fichier | Description | Résultat attendu |
|---|---|---|
| `Test_Magny.gpx` | 13.57 km autour de Magny-le-Hongre | 3 communes : Magny, Bailly, Serris |
| Trace Nice-La Trinité | 8 km | 2 communes : Nice, La Trinité |
| Trace multi-pays | France-Italie | Communes des deux pays |
| Trace en boucle | 10 km | Communes listées dans l'ordre de traversée |
| Trace avec aller-retour | 20 km | Communes avec passage multiples |

### 8.2 Critères de non-régression

- Sur `Test_Magny.gpx`, Magny doit apparaître **exactement 2 fois** (passage 1/2 et 2/2).
- La somme des longueurs doit être ≈ 13.57 km.
- Aucun segment < 15 m ne doit apparaître.
- Aucune commune-enveloppe ne doit couvrir 100 % de la trace.

---

**Fin du document.**