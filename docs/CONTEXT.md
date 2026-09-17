# Contexte du Projet - VisuGPS2

> **Documentation destinée à Claude Code et aux développeurs**
> Ce fichier fournit le contexte complet du projet pour faciliter le développement assisté par IA.

## Vue d'ensemble

VisuGPS2 est une **application desktop multiplateformes** basée sur Tauri + Vue 3 + Vuetify, conçue pour fonctionner en dual-screen avec communication inter-fenêtres et synchronisation de thème.

Le **workflow principal** de l'application est : **import GPX → audit (si anomalies) → passages multiples (si portions répétées) → édition caméra**. Une trace n'est **valide** que si elle est auditée ; les traces importées présentant des anomalies (aller-retours ponctuels, boucles de giratoire) doivent être auditées via la vue `/audit` avant de pouvoir entrer en édition caméra, et les portions parcourues plusieurs fois détectées par le module Multiride doivent être **validées** via la vue `/multiride`. Les deux barrières s'enchaînent : l'audit d'abord, les passages multiples ensuite.

### Objectifs du projet
- Créer une application desktop native multi-fenêtres
- Détecter automatiquement la configuration multi-écran
- Ouvrir une fenêtre secondaire sur l'écran opposé
- Synchroniser l'interface et l'état entre fenêtres
- Gérer les affichages (resolution, position, scale factor)

### Caractéristiques principales
1. **Dual-screen support** : Détection automatique et placement sur écrans différents
2. **Synchronisation inter-fenêtres** : Thème dark/light et communication d'événements
3. **Modularité Rust** : Code organisé en modules (display.rs, lib.rs)
4. **Multi-plateforme** : macOS (NSScreen) et Windows (Tauri API, Win32)
5. **Architecture clean** : Séparation claire des responsabilités

## Stack technique

### Frontend
- **Vue 3.4+** avec Composition API et `<script setup>`
- **TypeScript** pour le typage statique
- **Vuetify 3.5+** pour les composants UI Material Design
- **Pinia** pour la gestion d'état (setup stores pattern)
- **Vue Router 4.2+** pour la navigation
- **Vite** comme build tool

### Backend/Desktop
- **Tauri 2.x** pour l'application desktop native
- **Rust** pour le backend Tauri (code modulaire)
- **objc2** + **objc2-app-kit** : Détection NSScreen sur macOS
- **windows crate** : Win32 API pour Windows

### Outils
- **npm** comme gestionnaire de packages
- **Sass** pour les styles (via Vuetify)
- **vite-plugin-vuetify** pour l'auto-import des composants
- **cargo** comme gestionnaire de dépendances Rust

## Architecture du projet

### Structure des fichiers

```
VisuGPS2/
├── src/                       # Code source frontend (Vue 3 + TypeScript)
│   ├── router/
│   │   └── index.ts          # Routes (Accueil, Visualisation, EditionCamera, Audit, Multiride, ScreenBis)
│   ├── stores/
│   │   ├── index.ts          # Configuration Pinia
│   │   ├── app.ts            # Store applicatif (thème, displays, modes d'exécution)
│   │   ├── settings.ts       # Store des paramètres de configuration
│   │   ├── traces.ts         # Store des traces GPX importées
│   │   ├── keyframes.ts      # Store de persistance des keyframes (loadKeyframes, saveKeyframes, clearKeyframes)
│   │   ├── edition.ts        # Store de la vue d'édition caméra (trace, lecture, keyframes)
│   │   ├── audit.ts          # Store du module Audit GPX (détection AR/RP, archive, corrections)
│   │   ├── multiride.ts      # Store du module Multiride (détection, ajustements, validation)
│   │   └── ui.ts             # Store des notifications (snackbar)
│   ├── algorithms/           # Logique métier isolée, sans dépendance UI
│   │   └── keyframeGenerator.ts  # Génération + interpolation des keyframes caméra
│   ├── utils/
│   │   ├── format.ts         # Helpers de formatage (distance, élévation, durée)
│   │   └── geo.ts            # Utilitaires géographiques (Haversine, tri par distance)
│   ├── plugins/
│   │   └── vuetify.ts        # Configuration Vuetify
│   ├── views/                # Pages de l'application
│   │   ├── Accueil.vue       # Fenêtre principale avec comm. inter-fenêtres
│   │   ├── ScreenBis.vue     # Fenêtre secondaire
│   │   ├── EditionCamera.vue # Édition caméra (carte satellite+terrain, cadre ViewPort)
│   │   ├── Audit.vue         # Audit GPX (carte, panneau des anomalies, panneau d'action)
│   │   ├── Multiride.vue     # Passages multiples (carte, panneau des segments, validation)
│   │   └── Visualisation.vue # Page visualisation
│   ├── components/           # Composants réutilisables
│   │   ├── Accueil/          # Composants de la page d'accueil
│   │   ├── Edition/          # Composants de la vue d'édition caméra (carte, toolbar, cadre, lecture, HUD télémétrie/distance, graphe)
│   │   ├── Audit/            # Composants de la vue d'audit (toolbar, carte, anomalies, action, dialogues)
│   │   ├── Multiride/        # Composants de la vue des passages multiples (toolbar, carte, segments, fenêtre d'action, synthèse, dialogues)
│   │   └── parameters/       # Composants d'édition des paramètres
│   ├── assets/               # Images, styles
│   ├── App.vue              # Layout racine (détection multi-fenêtres)
│   └── main.ts              # Point d'entrée
│
├── src-tauri/                # Code Rust (Tauri 2.x)
│   ├── src/
│   │   ├── lib.rs            # Point d'entrée, orchestration, run()
│   │   ├── display.rs        # Détection écrans (MonitorInfo, get_displays)
│   │   ├── gestionMode.rs   # Gestion modes d'exécution (CRUD, .env)
│   │   ├── settings.rs       # Paramètres de configuration (TOML, secrets)
│   │   ├── import_gpx.rs     # Import de fichiers GPX (parsing, stats, registre)
│   │   ├── gpx_audit/        # Module Audit GPX (détecteurs AR/RP, corrections, export, archive, commandes)
│   │   ├── gpx_multiride/    # Module Multiride (portions répétées, ajustements, fichier de description)
│   │   └── main.rs           # Auto-généré (délègue à lib.rs)
│   ├── capabilities/
│   │   └── default.json      # Permissions fenêtres (main, screen-bis)
│   ├── settings.default.toml # Paramètres par défaut (embarqué)
│   ├── icons/                # Icônes application
│   ├── Cargo.toml            # Dépendances Rust
│   └── tauri.conf.json       # Config Tauri (windows, build, security)
│
├── docs/                     # Documentation
│   ├── CONTEXT.md           # Ce fichier
│   ├── ARCHITECTURE.md       # Architecture détaillée
│   ├── EXTENDING.md         # Guide d'extension
│   ├── SPEC_IMPORT_GPX.md   # Spécification module import GPX
│   ├── SPEC_AFFICHAGE_TRACES.md # Spécification favoris & affichage carte (à implémenter)
│   └── …
│
├── Configuration
│   ├── vite.config.ts       # Configuration Vite (allowedHosts)
│   ├── tsconfig.json        # Configuration TypeScript
│   ├── package.json         # Dépendances npm
│   ├── .gitignore
│   └── .env.example
│
└── Documentation utilisateur
    ├── README.md
    └── QUICKSTART.md
```

## Conventions de code

### Vue/TypeScript

1. **Composition API uniquement** avec `<script setup lang="ts">`
2. **Imports explicites** : toujours importer ce qui est utilisé
3. **Typage fort** : utiliser TypeScript partout
4. **Pas de Options API** : tout en Composition API

### Stores Pinia

- **Setup Stores pattern** (fonction avec `ref` et `computed`)
- Pas de syntaxe `defineStore({ state, actions, getters })`
- Un store = un fichier dans `src/stores/`

Exemple :
```typescript
export const useMyStore = defineStore('mystore', () => {
  const data = ref<string>('')
  const computed = computed(() => data.value.toUpperCase())

  function action() {
    data.value = 'new value'
  }

  return { data, computed, action }
})
```

### Composants Vue

- **Single File Components** (.vue)
- Template, script, style dans cet ordre
- Pas de scoped styles sauf cas particulier (Vuetify gère les styles)

Exemple :
```vue
<template>
  <v-card>
    <v-card-title>{{ title }}</v-card-title>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const title = ref('Mon titre')
</script>

<style>
/* Styles globaux si nécessaire */
</style>
```

### Vuetify

- **Auto-import activé** via vite-plugin-vuetify
- Pas besoin d'importer les composants `v-*`
- Utiliser la syntaxe v3 (pas de `v-slot:prepend` mais `<template v-slot:prepend>`)
- Icônes MDI : `mdi-icon-name`

### Router

- **Routes déclarées dans** `src/router/index.ts`
- Lazy loading pour les routes non critiques : `component: () => import('../views/Page.vue')`
- Navigation avec `useRouter()` et `router.push()`

## Patterns et bonnes pratiques

### 1. Séparation des responsabilités

- **Views** : Pages complètes avec layout
- **Components** : Composants réutilisables sans logique métier
- **Stores** : Logique métier et état partagé
- **Router** : Configuration des routes uniquement

### 2. Gestion d'état

- État local : `ref()` dans le composant
- État partagé : Store Pinia
- Props pour la communication parent → enfant
- Emits pour enfant → parent

### 3. Styling

- Utiliser les composants Vuetify autant que possible
- Pas de CSS custom sauf nécessaire
- Classes utilitaires Vuetify pour spacing : `ma-4`, `pa-2`, etc.

### 4. Communication Tauri

**Invoquer une commande Rust** :
```typescript
import { invoke } from '@tauri-apps/api/core'

// Appeler get_displays pour détecter les écrans
const displays = await invoke<MonitorInfo[]>('get_displays')

// Appeler open_second_window pour ouvrir ScreenBis
await invoke('open_second_window')
```

**Commandes disponibles** (voir [COMMANDS.md](./COMMANDS.md) pour la référence complète avec signatures) :

**Application & affichage** :
- `exit_app()` : Quitte proprement l'application depuis le backend Rust
- `get_displays()` : Retourne la liste des écrans (MonitorInfo[])
- `open_second_window()` : Ouvre la fenêtre ScreenBis sur l'écran secondaire
- `close_second_window()` : Ferme la fenêtre ScreenBis

**Modes d'exécution** :
- `get_execution_env()` : Retourne les infos d'environnement (is_dev, active_mode_dev, active_mode_prod)
- `get_modes()` : Retourne la liste des modes d'exécution (ModeInfo[])
- `create_mode(nom, descrition)` : Crée un nouveau mode d'exécution
- `update_mode(old_nom, new_nom, descrition)` : Modifie un mode existant
- `delete_mode(nom)` : Supprime un mode d'exécution
- `select_mode(nom)` : Change le mode actif (redémarre l'app)

**Paramètres** :
- `get_settings()` : Retourne les paramètres fusionnés (défaut + surcharges, secrets masqués)
- `get_settings_meta()` : Retourne l'organisation du drawer (table `[_meta]` : vues, groupes système, actions, handlers, libellés/icônes)
- `update_setting(path, value)` : Met à jour un paramètre (chiffre les secrets)
- `reset_setting(path)` : Rétablit un paramètre à sa valeur par défaut
- `get_setting_value(path)` : Retourne la valeur effective d'un paramètre (déchiffrée pour les secrets)

**Traces GPX** :
- `import_gpx_file()` : Importe un fichier GPX via le sélecteur natif (retourne TraceMetadata)
- `get_traces()` : Retourne la liste des traces importées pour le mode actif
- `delete_trace(traceId)` : Supprime une trace (fichier GPX + entrée du registre)
- `update_trace(traceId, favorite?, isDisplayed?)` : Met à jour partiellement une trace (PATCH)
- `get_trace_geometry(traceId)` : Retourne la géométrie GeoJSON d'une trace (cache, ou régénéré depuis le GPX)
- `get_trace_points(traceId)` : Retourne les points d'une trace avec altitude et distance cumulée 3D (re-parse le GPX)
- `save_keyframes(traceId, keyframesJson)` : Sauvegarde un jeu de keyframes dans `keyframes/{trace_id}.json` (écriture atomique)
- `get_keyframes(traceId)` : Charge les keyframes persistés d'une trace (`null` si absent)
- `delete_keyframes(traceId)` : Supprime le fichier keyframes d'une trace (tolérant si absent)

**Audit GPX** (détection AR + RP, corrections, réécriture GPX ; l'état de travail est **archivé** dans `traces/{trace_id}/audit.json`) :
- `audit_run_detection(traceId, params)` : Charge le GPX, consolide, détecte les **aller-retours** (AR) et les **boucles giratoires** (RP), retourne la trace de travail + les findings + la distance totale
- `audit_map_overlay(points, findings, closeM)` : Éléments de rendu (ancres de routage RP, étiquettes des points)
- `audit_delete_preview(points, finding, start, end, closeM)` : Aperçu prospectif d'une suppression (sans effet sur la trace de travail)
- `audit_routes_identical(car, carDistance, bike, bikeDistance)` : Test d'identité des deux tracés OpenRouteService (2 %, Hausdorff ≤ 15 m)
- `audit_apply_delete(traceId, points, findings, findingId, ds, de, nextPointId)` : Applique la suppression, resynchronise les index, absorbe les faux positifs imbriqués
- `audit_apply_route(traceId, points, findings, findingId, start, end, coords, profile, nextPointId)` : Remplace le segment par le tracé ORS (profil `car` / `bike`)
- `audit_mark_fp(findings, findingId)` / `audit_unmark_fp(findings, findingId)` : Marque / démarque un faux positif
- `audit_undo_correction(traceId, points, findings, findingId)` : Annule la correction d'une anomalie
- `audit_validate(traceId, points, findings)` : **Point de non-retour** — refuse tant qu'un finding est `pending`, réécrit le GPX (backup `{filename}.gpx.orig` une seule fois), régénère geojson/stats/hash et pose `audit_status = "clean"` **et** `audit_archived = true`
- `audit_save_state(traceId, params, totalDistanceM, points, findings, validated)` : Écrit l'**archive d'audit** (`traces/{trace_id}/audit.json`, écriture atomique) et retourne son horodatage — après la détection puis après **chaque** traitement, jamais depuis les aperçus
- `audit_load_archive(traceId)` : Relit l'archive d'une trace (`null` si absente ou inexploitable) — reprise d'une session interrompue, consultation d'un audit appliqué

**Passages multiples (module Multiride ; l'état est écrit dans `traces/{trace_id}/multiride.json`, le statut dans le registre)** :
- `multiride_detect(traceId, params)` : Relit le GPX, rééchantillonne, apparie, assemble et qualifie les portions répétées ; écrit la description et pose `multiride_status` (`none` / `pending`)
- `multiride_load(traceId)` : Relit la description d'une trace (`null` si absente ou inexploitable)
- `multiride_validate_segment(traceId, archive, segment)` : Approuve un segment dans son état courant — un vrai passage multiple, rien à changer — et réécrit la description (refusé sur un segment écarté)
- `multiride_merge_segment(traceId, archive, segment)` : Fusionne un segment avec le précédent (même sens, écart ≤ 1 km), enregistre les emprunts d'avant (`avant_fusion`) et réécrit la description (refusé si un des deux camps est écarté)
- `multiride_toggle_fp(traceId, archive, segment)` : Marque ou démarque un segment en faux positif (refusé sur un segment fusionné)
- `multiride_undo_segment(traceId, archive, segment)` : Annule un geste — retire le verdict, ou réinstalle les emprunts d'avant une fusion — et réécrit la description
- `multiride_reset(traceId, archive)` : Rejoue la détection avec les paramètres enregistrés ; **sans bouton dans la vue** (elle reste au catalogue comme recours, notamment pour une fusion enregistrée avant le champ d'annulation)
- `multiride_validate(traceId, archive)` : **Point de sortie** — marque l'état validé et lève la barrière de l'édition caméra

À l'exception de `multiride_load`, toutes réécrivent la description ; les commandes de geste — approbation, fusion, faux positif, annulation, réinitialisation — **reposent aussi le statut** du registre, un geste invalidant la validation de la détection.

### 5. Communication inter-fenêtres

Les fenêtres communiquent via les événements Tauri :

```typescript
import { emit, listen } from '@tauri-apps/api/event'

// Fenêtre Accueil : Envoyer un nombre aléatoire à ScreenBis
const value = Math.floor(Math.random() * 100) + 1
await emit('accueil-to-screenbis', value)

// Fenêtre ScreenBis : Écouter les événements d'Accueil
const unlisten = await listen<number>('accueil-to-screenbis', (event) => {
  receivedValue.value = event.payload
})
```

**Synchronisation du thème** :
- Accueil émet 'theme-changed' avec isDarkMode.value
- Toutes les fenêtres écoutent et mettent à jour leur appStore.isDarkMode
- Le thème Vuetify se met à jour automatiquement via :theme="appStore.theme"

## Scripts d'installation

### Workflow d'installation

```
setup.sh
  ↓
  ├─→ check-requirements.sh    (Vérifie Node, Rust, etc.)
  ├─→ install-dependencies.sh  (npm install + packages)
  ├─→ create-structure.sh      (Crée dossiers + fichiers)
  └─→ configure-app.sh         (Configure Vite + TS)
```

### Modifications des scripts

Si vous modifiez les scripts :
1. **check-requirements.sh** : Ajouter de nouveaux prérequis
2. **install-dependencies.sh** : Ajouter de nouvelles dépendances npm
3. **create-structure.sh** : Modifier la structure ou les fichiers générés
4. **configure-app.sh** : Changer les configs Vite/TypeScript

⚠️ **Important** : Maintenir la compatibilité macOS/Windows (bash + PowerShell)

## Application VisuGPS2

### Pages
- **Accueil** (`/`) : Fenêtre principale — drawer gauche (liste circuits triée par distance au centre de la carte ; **icône Éditer colorée selon l'avancement de l'édition pour le viewport paramétré** — forcée visible quand l'édition est incomplète, masquée uniquement si complète (verte) et non survolée, visible au survol quelle que soit la couleur), carte Mapbox (clusters de points de départ, popups ; **centrage/zoom mémorisés** dans les paramètres cachés `Carte.Vue.*` et restitués au lancement / au retour sur la vue), drawer droit (paramètres)
- **ScreenBis** (`/screen-bis`) : Fenêtre secondaire pour le dual-screen
- **Visualisation** (`/visualisation`) : Stub (toolbar Home uniquement, vue réservée à la visualisation 3D)
- **Audit** (`/audit`) : Audit d'une trace GPX. Le module détecte deux familles d'anomalies — **aller-retours ponctuels** (AR : rebonds, aiguilles de traceur) et **boucles de giratoire** (RP : 270°, 360° et plus) — et propose des corrections : **suppression de points**, **routage OpenRouteService** (profils voiture et vélo, deux clés en bascule automatique sur quota épuisé), **faux positif**. Chaque correction est **annulable par anomalie**. Le module **remplace** l'ancien pipeline de nettoyage en 3 étapes. Carte Mapbox GL dédiée (3ᵉ instance), panneau des anomalies et panneau d'action, indicateur de progression, synthèse (compteurs AR / RP / faux positifs), dialogues de confirmation (`ConfirmExitDialog`, `ConfirmApplyDialog`). Le bouton **« Appliquer »** est **strict** (actif seulement quand plus aucune anomalie n'est `pending`), **ferme la vue** et n'est **pas annulable** : il réécrit le GPX (backup `{filename}.gpx.orig` posé une seule fois), régénère geojson/stats/hash et pose `audit_status = "clean"` **et** `audit_archived = true`. **L'état de travail est archivé sur disque** (`traces/{trace_id}/audit.json`, écrit après la détection puis après **chaque** traitement) : la vue s'ouvre donc en **détection**, en **reprise** (session interrompue d'une trace `needs_review`) ou en **consultation** — une trace déjà auditée rouvre ses anomalies et leurs corrections en **lecture seule**, sans annulation possible. À la sortie, le store est réinitialisé ; une confirmation rappelle, selon le cas, les anomalies encore à traiter ou le fait que **le fichier GPX n'a pas été créé** (consultation exclue : aucun message). Le `traceId` circule par la **query** de la route (`/audit?traceId=…`). Accessible depuis le bouton Éditer d'un circuit « À auditer » (icône `mdi-map-marker-path` orange), depuis le bouton **Voir les anomalies de la source** d'un circuit dont l'audit est **archivé** (section info, icône verte — le bouton est absent sinon) et via le garde-fou d'`EditionCamera`. Une trace non « clean » ne peut pas entrer en édition caméra.
- **Multiride** (`/multiride`) : vue des **passages multiples** — portions de trace parcourues plusieurs fois (aller-retour sur un tronçon, reconnaissance repassant sur une section, boucle locale). Détection automatique à l'import d'une trace sans anomalie et après l'application d'un audit ; **restitution** par une carte Mapbox dédiée (4ᵉ instance : trace de fond bleue, emprunts en trois couches — référence verte épaisse **sous** les allers vert-lime et les retours rouges —, bornes de la trace, popups au clic, épaississement au survol, cadrage du segment sélectionné) et un panneau latéral (un bloc par segment — en-tête portant la longueur et le début de l'emprunt de **référence**, badge d'état aligné à droite, **ruban multi-rails** positionnant chaque emprunt sur la trace entière et le colorant par sens, emprunts Aller/Retour réduits à leur début —, fermé par la **synthèse**, repliée : segments, emprunts, faux positifs, approuvés, fusions, km répétés, paramètres actifs un par ligne). **Gestes par segment**, dans la **fenêtre d'action** ouverte par le clic sur la ligne ou sur un emprunt de la carte (même mécanisme que l'audit, `MultirideActionPanel`) : **approuver** — un vrai passage multiple, rien à changer —, **fusionner** avec le précédent (même sens, écart ≤ 1 km ; refusé si un des deux camps est écarté), **faux positif** (segment estompé sur la carte et barré dans la liste), **annuler** le geste en cours. Deux ordres de gestes, et deux règles : un **verdict** (approuvé, écarté) porte sur le segment dans son état courant et les deux s'excluent ; une **fusion** réorganise la détection sans la juger — le segment fusionné, neuf, s'approuve comme un autre et perd les approbations de ses deux camps. **Tout geste invalide la validation de la détection** : le statut repasse à « à valider » (fichier et registre) et la barrière de l'édition caméra se referme. **Compteur d'avancement** `x/y` dans la barre : segments jugés — approuvés ou écartés — sur le total, faux positifs rappelés à part ; un segment fusionné n'a rien dit de sa justesse, il reste à examiner jusqu'à son verdict. **Validation** : elle lève la barrière et **revient à l'accueil** ; l'édition caméra s'ouvre depuis la carte du circuit, plus depuis cette vue, dont c'est la sortie avec le retour. Les quatre paramètres `Multiride.Detection.*` (tolérance, longueur min, pas d'échantillonnage, fusion des références) sont réglables dans le drawer ; **enregistrer un paramètre rejoue la détection** — la relance est automatique, sans bouton — et les paramètres sont inaccessibles tant qu'un geste change le résultat ou qu'une analyse est en cours. Accessible depuis le bouton Éditer d'un circuit « à valider » (icône `mdi-repeat` ambre), depuis le bouton **Passages multiples** d'un circuit **validé** (section info, icône verte — le bouton est absent tant que les passages restent à valider ; un geste y invalide la validation) et via le garde-fou d'`EditionCamera`. La trace est désignée par la **query** de la route (`/multiride?traceId=…`).
- **EditionCamera** (`/edition-camera`) : Vue d'édition caméra — carte Mapbox satellite + terrain, trace sélectionnée, curseur (CircleLayer WebGL synchronisé terrain, couleur paramétrable), **au lancement la vue reste vierge (carte masquée, spinner centré) jusqu'au chargement complet des tuiles, puis est révélée directement au km 0** (sans vue globale intermédiaire), lecture (keyframes générés par l'algorithme **frustum** (spec §3.3, placement récursif par visibilité + occlusion relief via grille `terrain-rgb` fine + réorientation oblique du cap) ou **simple** (échantillonnage régulier) — **algorithme et gap min gérés dans le panneau Paramètres** (drawer) — cap **lissé** entre les RdV (`lerpAngle`), boucle rAF), **deux fichiers keyframes par trace** (16:9 → `{trace_id}_169.json`, 4:3 → `{trace_id}_43.json`) sélectionnés via le **bouton ViewPort** de la toolbar (flip-flop 16:9 / 4:3 — icône monitor / monitor-small, couleur = avancement du verrouillage : cadre + fichier du ratio choisi, l'autre garanti en arrière-plan), **panneau Paramètres** (bouton `mdi-cog-outline` en **flip-flop** → `SettingsDrawer` limité aux **catégories de la vue** — sections système masquées via `show-system=false` : Génération (algorithme, gap min), Caméra (zoom/pitch/viewport par défaut, durée du déplacement caméra `dureeFlyTo` 100–1000 ms, affichage HUD télémétrie et panneau caps), Lecture (accélération `acceleration` ×1.5–8) et Couleurs (couleur/épaisseur de la trace, curseur) — appliqués en direct au store), contrôle de lecture compact (colonne boutons à droite du graphe : RdV précédent/suivant verts, km0 / Play-Pause / **lecture accélérée** (double chevron, ×`acceleration`) / dernier point ; vitesse 1× ou facteur accéléré paramétrable), **graphe SVG d'avancement** (timeline proportionnelle 3px/100m alignée à droite sur traces courtes, 3 zones RdV/avancement/graduation — **ticks RdV bicolores** : zone supérieure zoom (rouge si ≠ défaut), zone inférieure pitch (orange si ≠ défaut), vert par défaut — curseur étendu dont la couleur suit le paramètre `Edition.Couleurs.curseur`, clic seek + sélection, tooltip survol altitude, auto-scroll), **changements de cap brutaux** (bande colorée entre deux ticks RdV sur la timeline — couleur = sens (teal horaire / deep-purple anti-horaire), **épaisseur** = intensité du taux (2 px sous le seuil, +2 px par bande de 30 °/km), seuil réglable dans le tableau — + **tableau à la demande** cliquable — Départ/Arrivée en jaune sur le segment courant, sens en Δ dist/Δ cap, Taux coloré jaune→rouge par bande), **verrous de segments** (mode **validation** via bouton `mdi-camera-lock` : **clic carte ou touche Entrée** = problème, segments sans clic verrouillés pendant la lecture — trait rouge en haut de la timeline pour les segments non verrouillés, **double-clic** sur un segment pour basculer le verrou, keyframes bordant un verrou non modifiables ; **en mode validation**, le **cadre ViewPort** et le **fond de l'indicateur de distance** passent au **bleu**), **Composant B — édition des keyframes** (CameraEditor : switch Cible, sliders Pitch/Zoom customs avec valeurs par défaut issues des paramètres (`pitchDefaut`/`zoomDefaut`), CompassBandeau défilement infini, Undo/Supprimer/Sauvegarder, km0 non supprimable), **raccourcis clavier** (Espace Play/Pause, flèches ←/→ navigation RdV), HUD télémétrie (caméra + altitude traceur + relation caméra↔curseur, **masqué par défaut** — paramètre `afficherTelemetrie`, **bouton Fermer** valable pour la session, **lignes Zoom/Pitch colorées** — vert au défaut, orange/rouge sinon), **HUD distance** (barre bas, distance parcourue orange / totale), cadre ViewPort au ratio sélectionné (overlay CSS). Déclenchée par le bouton Éditer d'un circuit.

### Fonctionnalités
- Import/suppression/mise à jour de traces GPX (favoris et affichage persistés)
- **Audit GPX** : détection des **aller-retours ponctuels** (AR) et des **boucles de giratoire** (RP) avec les paramètres `Audit.*` (consolidation, seuils AR/RP). Corrections : **suppression de points**, **routage OpenRouteService** (deux clés chiffrées en bascule), **faux positif** — toutes **annulables par anomalie**. **L'état de travail est archivé sur disque** (`traces/{trace_id}/audit.json`, écrit après la détection et après chaque traitement) : une session interrompue est **reprise** à la réouverture, et un audit appliqué reste **consultable** en lecture seule depuis la carte du circuit (bouton « Voir les anomalies de la source »). Le GPX n'est réécrit qu'au bouton **« Appliquer »** (strict : `pending === 0`, ferme la vue, non annulable), avec backup `{filename}.gpx.orig` posé une seule fois. Une trace non `"clean"` ne peut pas entrer en édition caméra.
- **Passages multiples (Multiride)** : détection des portions **parcourues plusieurs fois** avec les paramètres `Multiride.Detection.*` (tolérance de superposition, longueur minimale, pas d'échantillonnage, fusion des références proches). **L'état complet est écrit sur disque** (`traces/{trace_id}/multiride.json`, à la nomenclature de la spécification) : c'est le **contrat de sortie** que consommera la Visualisation, et il porte aussi les gestes de l'utilisateur — approbation d'un segment (`valide`), fusion (`avant_fusion`, qui rend la fusion annulable), faux positif. Gestes par segment depuis une **fenêtre d'action**, annulables individuellement : approuver, fusionner avec le précédent (même sens, écart ≤ 1 km), écarter, annuler. Une trace dont les passages ne sont pas validés (`multiride_status = "pending"`) ne peut pas entrer en édition caméra ; **tout geste invalide la validation** et repose ce statut, la barrière se refermant alors — un ajustement postérieur à la validation n'est donc possible qu'en repassant par une nouvelle validation. La détection suit l'audit et le précède : elle est déclenchée à l'import d'une trace sans anomalie, par l'application d'un audit et par l'enregistrement d'un paramètre de détection, en **best-effort** pour les chaînes automatiques (un échec laisse la trace permissive plutôt que de bloquer l'import ou l'audit).
- Carte Mapbox GL avec **clustering** des points de départ (token via le paramètre `Systeme.Key.mapBox`)
- **Clés de licence regroupées** : les trois clés API — `Systeme.Key.mapBox`, `Systeme.Key.openRouteServiceClePrimaire`, `Systeme.Key.openRouteServiceCleSecondaire` (type `secret`, chiffrées AES-256-GCM, critiques) — forment le groupe **système** `Systeme.Key` (libellé « Licences », icône de paramètre `mdi-account-key-outline`). Elles se saisissent donc depuis le **drawer de la vue Accueil**, les vues Audit et Édition masquant les sections système (`show-system=false`) tout en continuant de les lire par chemin.
- Affichage des traces favorites et forcées (dégradé bleu → rouge, couches Mapbox dédiées)
- **Synchronisation carte ↔ liste** : la liste des circuits est triée par distance au centre courant de la carte (Haversine), recalculée en temps réel sur `moveend` (debounce 150 ms)
- **Focus carte** : clic sur Info d'un circuit isole et cadre la trace (`fitBounds`), puis revient à la vue précédente (`flyTo`) — durée paramétrable (`Carte.Traces.dureeFlyTo`)
- Clic sur un cluster → zoom d'expansion ; clic sur un point → popup (nom, source, coordonnées)
- Toggle thème dark/light (synchronisé entre fenêtres)
- Gestion des modes d'exécution (OPE / EVAL_*)
- Système de paramètres TOML avec chiffrement des secrets
- Détection multi-écrans et placement automatique

### Store exemple
`src/stores/app.ts` gère le thème et l'environnement d'exécution :
- `isDarkMode` : état du thème
- `theme` : computed qui retourne 'dark' ou 'light'
- `toggleDarkMode()` : action pour basculer
- `displays` : liste des écrans détectés
- `isDev`, `activeMode`, `modes` : environnement et modes d'exécution

Autres stores existants :
- `src/stores/settings.ts` : paramètres de configuration (pattern Setup Store)
- `src/stores/traces.ts` : traces GPX importées (pattern Setup Store). Expose `traces`, `loading`, `mapCenter`, `focusedTraceId` (trace « focus » temporaire, clic Info), `visibleTraceIds` (IDs visibles dans le viewport), `traceCount`, `sortedTracesByDistance` (tri Haversine par rapport au centre de la carte), `visibleTracesByDistance` (filtrage viewport, tri par distance), et les actions `loadTraces`, `importerGpx`, `supprimerTrace`, `updateTrace`, `getTraceGeometry`, `getTracePoints` (points avec altitude + distance cumulée 3D), `updateMapCenter`, `setVisibleTraceIds`.
- `src/stores/keyframes.ts` : persistance des keyframes sur disque (pattern Setup Store). Actions `loadKeyframes(traceId, viewportAspect)` (charge `traces/{trace_id}/keyframes_169.json` ou `keyframes_43.json` selon le ratio, retourne `null` si absent/invalide), `saveKeyframes(set)` (sauvegarde via écriture atomique, le ratio est déduit du champ `viewport` du jeu), `clearKeyframes(traceId, viewportAspect)` (supprime le fichier, tolérant si absent).
- `src/stores/edition.ts` : état de la vue d'édition caméra (pattern Setup Store). Côté sélection : `selectedTraceId`, `showViewportFrame` (affiché par défaut), **`viewportAspect`** (`'16:9'` défaut / `'4:3'`, + `setViewportAspect` qui recharge le fichier keyframes du ratio) + actions `selectTrace`/`clearSelection`/`toggleViewportFrame`. Côté algorithme : `keyframeAlgorithm` (`'frustum'` défaut / `'simple'`), `minKeyframeGapM` (200–5000, pas 50) + `setKeyframeAlgorithm`/`setMinKeyframeGapM` (forcent la régénération des deux fichiers). Côté lecture : `keyframeSet`, `isPlaying`, `speed`, `currentTimeMs` ; getters `interpolatedCam`/`interpolatedTraceur` (interpolation caméra/curseur avec altitude, bearing lissé par `lerpAngle`), `currentDistanceKm`, `totalDistanceKm`, `markerDistanceM`, `markerRelativeBearing`, `altitudeAtDistance(m)` (closure capturant la polyligne — altitude interpolée à une distance donnée, utilisée par le tooltip du ProgressGraph), `currentKeyframe` (keyframe à la position courante, pilote le CameraEditor), `canGoNextRdv`/`canGoPrevRdv` ; **changements de cap** : `headingChanges`/`brutalHeadingChanges` (analyse des virages entre keyframes, module pur `algorithms/headingChanges.ts`), `headingChangeThresholdDegPerKm` (45°/km, 10–1000, pas 5, + `setHeadingChangeThreshold` — filtre d'affichage), `showHeadingChangesPanel` (+ toggle) ; **verrous de segments** : `validationMode` (+ toggle via `mdi-camera-lock`), `lockedSegmentFromDistances`, `currentSegmentFromDistance`, `markValidationClick`, `toggleSegmentLock`/`lockSegment`/`unlockSegment` (persistés via les flags `locked`/`marks` de chaque keyframe — **traits bleus persistés** dans le fichier keyframes, protection par parcours), gardes sur `updateKeyframe`/`removeKeyframe`/`addKeyframe` pour les keyframes verrouillés ; **édition keyframes (Composant B)** : `updateKeyframe(distance, updates)` (mute cam sans sauvegarde auto), `addKeyframe` (insertion + interpolation voisins), `removeKeyframe` (**km 0 intouchable**, min 2 conservés), `saveKeyframes()` (sauvegarde explicite), `selectKeyframe` ; lecture : `setKeyframeSet(set, feature, tracePoints?)` (construit la polyligne enrichie altitude depuis les points backend), `play`/`pause`/`togglePlay`, `setSpeed`, `seekToDistance`, `tick(deltaMs)`, `goToNextRdv`/`goToPrevRdv` (pause auto + seek + sélection). État UI éphémère non persisté.
- `src/stores/audit.ts` : état du module Audit GPX (pattern Setup Store, état **archivé sur disque** au fil des actions). Expose les types miroir des structs Rust (`AuditParams`, `AuditPoint`, `Finding`, `FindingKind`, `FindingStatus`, `AuditState`, `AuditDetectionResult`, `AuditArchive`, `DeletePreview`, `FindingOverlay`), l'état `traceId`, `points` (trace de travail), `findings`, `params`, `selectedFindingId`, les aperçus (`deletePreview`, `routePreview`), l'état de mode (`isConsultation`, `archivedAt`), les getters de progression (`hasWorkInProgress`, compteurs d'anomalies traitées / restantes, `canApply` — faux en consultation), et les actions `runAudit(traceId, params)` (détection initiale via `audit_run_detection`), **`restore(traceId, consultation)`** (recharge l'archive et positionne le mode — reprise ou consultation), `selectFinding`, `applyDelete`, `applyRoute`, `markFp`, `unmarkFp`, `undoCorrection` (chacune archive son résultat et refuse d'agir en consultation), `clearDeletePreview`, `clearRoutePreview`, `setRouteRange`, **`validateAndRewrite`** (bouton « Appliquer » : refuse tant qu'un finding est `pending`, puis marque l'archive `validated`), `reset` (réinitialisation à la sortie de la vue — décision 9 ; l'archive, elle, survit). Les appels réseau OpenRouteService vivent dans la composable `src/composables/useAuditOrs.ts`.
- `src/stores/multiride.ts` : état du module Multiride (pattern Setup Store). Expose les types miroir des structs Rust (`MultirideParams`, `MultiridePassage`, `MultirideArchive`, `MultirideDetectionResult`, `MultirideSens`), l'état `currentTraceId`, `archive` (l'état complet, gestes compris), `selectedSegment`, `analysisDurationMs`, `loading`, les getters `passages`, `segmentNumbers`, `segmentCount`, `status` (dérivé de l'archive, comme côté Rust), `needsValidation`, **`hasAdjustments`** (les gestes qui changent le résultat — écarté, fusionné —, et donc le verrouillage des paramètres), **`treatedSegmentCount`** (les verdicts — approuvé, écarté —, le `x` du compteur d'avancement), `falsePositiveSegmentCount`, `pendingSegmentCount`, `repeatedKm` (longueurs des emprunts de référence, faux positifs exclus), et les actions `runDetection(traceId, params)`, **`restore(traceId)`** (recharge la description écrite ; `false` si absente, la vue relance alors la détection), **`validateSegment(segment)`**, `mergeSegment(segment)`, `toggleFp(segment)`, **`undoSegment(segment)`**, **`validate()`** (lève la barrière), `reset` (sortie de la vue — la description, elle, survit), avec les helpers `segmentPassages`, `referenceOf` (« la longueur et le début », lus sur l'emprunt de référence) et `previousSegmentPassages`.
- `src/stores/ui.ts` : notifications snackbar mutualisées (pattern Setup Store)

## Variables d'environnement

Fichier `.env` (copier depuis `.env.example`) :

```env
VITE_APP_NAME="Mon Application"
VITE_API_URL="http://localhost:3000"
```

Utilisation dans le code :
```typescript
const appName = import.meta.env.VITE_APP_NAME
```

⚠️ Seules les variables préfixées `VITE_` sont accessibles côté frontend.

## Points d'extension

### Ajouter une nouvelle page

1. Créer `src/views/MaPage.vue`
2. Ajouter la route dans `src/router/index.ts`
3. Ajouter un lien dans `src/App.vue` (navigation drawer)

### Ajouter un nouveau store

1. Créer `src/stores/monstore.ts`
2. Utiliser le pattern setup store
3. Importer et utiliser dans les composants

### Ajouter une dépendance

1. Modifier `install-dependencies.sh` et `install-dependencies.ps1`
2. Ajouter `npm install package-name`
3. Documenter dans README.md

### Ajouter une commande Tauri

1. Créer un module dans `src-tauri/src/` (ou ajouter la commande dans un module existant)
2. Définir `#[tauri::command]`
3. Enregistrer le module dans `src-tauri/src/lib.rs` (`mod mon_module;`)
4. Ajouter la commande dans `.invoke_handler()` de `lib.rs`
5. Appeler avec `invoke()` côté frontend
6. Si la commande utilise un plugin, l'ajouter via `.plugin(...)` dans `lib.rs` et la permission dans `capabilities/default.json`

## Configuration Tauri

Fichier `src-tauri/tauri.conf.json` :

```json
{
  "package": {
    "productName": "mon-app",
    "version": "1.0.0"
  },
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devPath": "http://localhost:1420",
    "distDir": "../dist"
  },
  "tauri": {
    "windows": [
      {
        "title": "Mon Application",
        "width": 800,
        "height": 600
      }
    ]
  }
}
```

## Réutilisation du template

Pour créer un nouveau projet :

```bash
# 1. Copier le template
cp -r "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"

# 2. Nettoyer les fichiers générés
rm -rf node_modules src-tauri package-lock.json

# 3. Réinstaller
./setup.sh

# 4. Personnaliser
# - src-tauri/tauri.conf.json (productName, version)
# - package.json (name, version, description)
# - src/App.vue (titre)
# - src-tauri/icons/ (icônes de l'app)
```

## Dépannage pour Claude Code

### Erreurs communes

1. **Import non résolu** : Vérifier que le fichier existe et le chemin est correct
2. **Composant Vuetify non reconnu** : C'est normal, ils sont auto-importés
3. **Store non trouvé** : Vérifier que le store est exporté avec `export const`
4. **Route ne fonctionne pas** : Vérifier le `name` dans router et `router-link :to`

### Commandes utiles

```bash
# Développement
./dev.sh

# Vérifier les erreurs TypeScript
npx tsc --noEmit

# Voir les routes
cat src/router/index.ts

# Voir les stores
ls src/stores/
```

## Philosophie du code

Ce template privilégie :
- ✅ **Simplicité** plutôt que complexité
- ✅ **Convention** plutôt que configuration
- ✅ **Lisibilité** plutôt qu'optimisation prématurée
- ✅ **Documentation** plutôt que commentaires dans le code
- ✅ **Réutilisabilité** plutôt que cas particuliers

## Notes importantes pour Claude Code

1. **Ne pas ajouter ESLint/Prettier** : C'est un choix délibéré (environnement minimal)
2. **Ne pas modifier les scripts sans raison** : Ils sont testés et fonctionnels
3. **Toujours documenter** : Nouveaux stores, routes, composants
4. **Respecter la structure** : Ne pas créer de nouveaux dossiers sans justification
5. **TypeScript strict** : Toujours typer les paramètres et retours

## Ressources

- Tauri : https://tauri.app/
- Vue 3 : https://vuejs.org/
- Vuetify 3 : https://vuetifyjs.com/
- Pinia : https://pinia.vuejs.org/
- Vue Router : https://router.vuejs.org/

## Fonctionnalités spécifiques à VisuGPS2

### Dual-screen Support

**Détection automatique** :
1. App.vue charge les displays via `appStore.loadDisplays()` (commande Rust)
2. Si 2+ écrans détectés → `invoke('open_second_window')`
3. Rust vérifie la position de la fenêtre main et place screen-bis sur l'autre écran

**Placement des fenêtres (Configurable)** :
Le choix de l'écran (principal, secondaire) pour chaque fenêtre est paramétrable par l'utilisateur via le panneau des réglages (paramètres de type `monitor_selection` : `Affichage.moniteurs.principal`, `Affichage.moniteurs.secondaire`).
Les valeurs peuvent être des critères génériques (`origin`, `other`, `builtin`, `external`) ou un nom d'écran spécifique détecté.

- **macOS** : Utilise `NSScreen.screens()` pour lister les écrans.
  - Différenciation entre écran interne (`builtin`) et externe (`external`).
  - Position X/Y = frame.origin pour chaque écran
  - Scale factor = backingScaleFactor

- **Windows** : Utilise Tauri `available_monitors()` et Win32 API
  - Active monitor rect détecté au setup avec `GetForegroundWindow()`
  - Les écrans génériques de type "DISPLAY_X" détectés physiquement peuvent être filtrés si besoin.

**MonitorInfo struct** :
```rust
pub struct MonitorInfo {
    pub name: Option<String>,      // Nom de l'écran
    pub position: (i32, i32),      // X, Y coordinates
    pub size: (u32, u32),          // Width, height pixels
    pub scale_factor: f64,         // DPI scaling
}
```

### Communication inter-fenêtres

**Pattern d'implémentation** :
1. Les fenêtres sont déclarées statiquement dans tauri.conf.json (visible: false pour screen-bis)
2. App.vue détecte le label de la fenêtre actuelle avec `getCurrentWindow().label`
3. Router navigue vers le composant approprié
4. Les événements Tauri synchronisent l'état entre fenêtres

**Événements utilisés** :
- `accueil-to-screenbis` : Nombres 1-100 (Accueil → ScreenBis)
- `screenbis-to-accueil` : Nombres 501-600 (ScreenBis → Accueil)
- `theme-changed` : Boolean (isDarkMode) - synchronisé entre tous
- `screenbis-closed` : Émis par ScreenBis lors de sa fermeture pour informer l'AppBar et réinitialiser `appStore.isScreenBisOpen` à `false`

### Architecture Rust modulaire

**display.rs - Détection des écrans**
```rust
pub struct MonitorInfo { ... }
pub fn get_all_monitors() -> Vec<MonitorInfo> { ... }
#[tauri::command]
pub fn get_displays(window: tauri::Window) -> Vec<MonitorInfo> { ... }
```

**gestionMode.rs - Gestion des modes d'exécution**
```rust
pub struct ModeInfo { nom, descrition, création, révision }
pub fn read_active_mode(app_data_dir: &Path, is_dev: bool) -> String { ... }
#[tauri::command]
pub async fn get_execution_env(app: AppHandle) -> Result<ExecutionEnv, String> { ... }
#[tauri::command]
pub async fn get_modes(app: AppHandle) -> Result<Vec<ModeInfo>, String> { ... }
// + create_mode, update_mode, delete_mode, select_mode
```

**settings.rs - Paramètres de configuration**
```rust
pub struct SettingDefinition { path, description, documentation, type, default, value, min, max, step, critical, unit, choices, icon, is_overridden }
// icon: Option<String> — icône MDI optionnelle pour le drawer (défaut = icône par type)
pub struct SettingsMeta { system, views, groups }   // organisation du drawer (table [_meta])
pub fn init_settings_state(app_handle: &AppHandle) -> Result<Arc<RwLock<SettingsState>>, String> { ... }
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Vec<SettingDefinition>, String> { ... }
// + get_settings_meta, update_setting, reset_setting, get_setting_value
```

**import_gpx.rs - Import de traces GPX**
```rust
pub struct Point3D { lat, lon, alt }
pub struct TraceStats { start_point, end_point, distance_m, positive_elevation_m, … }
pub struct TraceMetadata {
    id, name, source, source_url, activity_type, filename, import_date, stats, hash,
    favorite,     // favori (persisté, #[serde(default)])
    is_displayed, // affichage carte (persisté, #[serde(default)])
    audit_status, // "clean" | "needs_review" (#[serde(default)] → "needs_review")
    audit_archived, // audit appliqué archivé, donc consultable (#[serde(default)] → false)
}
#[tauri::command]
pub async fn import_gpx_file(app: AppHandle) -> Result<TraceMetadata, String> { ... }
#[tauri::command]
pub async fn get_traces(app: AppHandle) -> Result<Vec<TraceMetadata>, String> { ... }
#[tauri::command]
pub async fn delete_trace(app: AppHandle, trace_id: String) -> Result<(), String> { ... }
#[tauri::command]
pub async fn update_trace(
    app: AppHandle, trace_id: String,
    favorite: Option<bool>, is_displayed: Option<bool>,
) -> Result<(), String> { ... }
```

**lib.rs - Orchestration principale**
```rust
mod display;
mod gestionMode;
mod settings;
mod import_gpx;
mod gpx_audit;   // module Audit GPX (15 fichiers : types, détecteurs, corrections, archive, commandes)

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| { /* init settings + display */ })
        .invoke_handler(tauri::generate_handler![/* toutes les commandes */])
        .run(tauri::generate_context!())
}
```

**Dépendances Rust** :
- `objc2` + `objc2-app-kit` + `objc2-foundation` : Accès NSScreen sur macOS
- `windows` : Win32 API (GetForegroundWindow, GetMonitorInfo)
- `tauri` : APIs principales (Manager, invoke_handler, generate_handler)
- `tauri-plugin-dialog` : Sélecteur de fichiers natif (Tauri 2.x)
- `tauri-plugin-opener` : Ouverture de liens dans le navigateur
- `serde` + `serde_json` : Sérialisation
- `toml` : Lecture/écriture des fichiers de configuration
- `chrono` : Dates et heures
- `gpx` : Parsing des fichiers GPX
- `geo` : Calculs géodésiques (Haversine)
- `sha2` + `hex` : Empreintes SHA256 (anti-doublon)
- `uuid` : Identifiants uniques
- `keyring` + `aes-gcm` : Chiffrement des secrets
- `dotenvy` : Variables d'environnement

### Gestion Dynamique des Modes d'Exécution

L'application intègre un système robuste de gestion des modes d'exécution (définis dans un fichier de configuration Lu par Rust et exposé au Frontend) :

1. **Variables du Store** :
   - `activeModeDev` correspond à l'environnement pointé en développement.
   - `activeModeProd` correspond à l'environnement actif en production.

2. **Logique d'Interface (`ModeExecutionCard.vue`)** :
   - Un chip **Actif** s'affiche en vert pour le mode `OPE` et en bleu pour tout autre mode de production actif.
   - Un chip **Actif Dev** (orange) s'affiche uniquement en mode développement pour le mode correspondant à `activeModeDev`.
   - Les boutons d'action (Éditer/Supprimer) sont désactivés/masqués pour le mode `OPE`, ainsi que pour les modes actifs de développement (`activeModeDev`) et de production (`activeModeProd`).

---

**Dernière mise à jour** : 2026-09-17
**Version du projet** : 0.0.1
**Status** : En développement actif
**Fonctionnalités** : Dual-screen, inter-window communication, theme sync, display detection, multi-env execution modes, settings management (TOML + secrets chiffrés), import/suppression/mise à jour de traces GPX (favoris/affichage persistés), carte Mapbox (clustering points de départ, traces favorites/affichées dégradé, focus carte sur clic Info, synchronisation liste triée par distance), **audit GPX** (détection AR — aller-retours ponctuels — et RP — boucles de giratoire 270°/360° et plus ; corrections par suppression de points, routage OpenRouteService et faux positif, toutes annulables par anomalie ; **archive d'audit persistée** au fil des traitements (`audit.json`), rouvrable en consultation depuis la carte du circuit ; réécriture du GPX au bouton « Appliquer » strict avec backup `.orig`), **multiride** (détection des portions parcourues plusieurs fois ; restitution carte + panneau avec ruban multi-rails et synthèse repliée ; gestes par segment dans une fenêtre d'action — approuver, fusionner, écarter, annuler —, chacun annulable et **invalidant la validation de la détection** ; compteur d'avancement des segments jugés ; relance automatique à l'enregistrement d'un paramètre ; fichier de description `multiride.json` à la nomenclature de la spécification, écrit à la détection et après chaque geste, avec les emprunts d'avant fusion ; **verrou** `multiride_status` avant l'édition caméra, levé par la validation — qui ramène à l'accueil), vue d'édition caméra (carte satellite + terrain, curseur jaune CircleLayer WebGL, lecture par keyframes + boucle rAF, contrôle compact Play/Pause + navigation RdV/km0/dernier, graphe SVG d'avancement timeline proportionnelle curseur orange alignée à droite sur traces courtes, **édition des keyframes (Composant B) : switch Cible, sliders Pitch/Zoom, compas, Undo/Supprimer/Sauvegarder**, raccourcis clavier Espace/flèches, HUD télémétrie caméra + altitude traceur, HUD distance parcourue/total, cadre ViewPort 16:9)
