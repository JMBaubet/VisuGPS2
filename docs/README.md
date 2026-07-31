# Documentation de VisuGPS2

> Documentation complète pour développeurs et IA (ZCode / Claude Code)

## Vue d'ensemble

VisuGPS2 est une **application desktop** de visualisation de traces GPX, basée sur **Tauri v2** (Rust) + **Vue 3** + **Vuetify 3**. Elle supporte le multi-écrans, les modes d'exécution isolés, et la gestion de paramètres avec chiffrement des secrets.

Cette documentation est particulièrement utile pour :
- **ZCode et autres IA** : Comprendre l'architecture et les conventions avant d'intervenir
- **Nouveaux développeurs** : Onboarding rapide sur le projet
- **Référence** : Catalogue des commandes, du stockage, des conventions

## Documents disponibles

### 📋 [CONTEXT.md](./CONTEXT.md)
**Contexte général du projet**

Le point de départ essentiel. Explique :
- L'objectif et la philosophie de l'application
- La stack technique utilisée
- La structure générale des fichiers
- Les patterns et conventions de base
- Les points d'extension

👉 **Commencer par ce fichier** si vous découvrez le projet.

### 🏗️ [ARCHITECTURE.md](./ARCHITECTURE.md)
**Architecture technique détaillée**

Plongée approfondie dans l'architecture :
- Couches de l'application (Frontend, Router, State, Build, Desktop)
- Communication entre les couches (invoke Tauri, événements inter-fenêtres)
- Configuration de chaque outil (Vite, Tauri v2, Vuetify, etc.)
- Flux de données et patterns architecturaux
- Gestion des modes d'exécution et des paramètres

👉 **Lire ce fichier** pour comprendre le fonctionnement interne.

### 📡 [COMMANDS.md](./COMMANDS.md)
**Référence des commandes Tauri**

Catalogue exhaustif des **20 commandes** backend↔frontend :
- Application, affichage/multi-écrans, modes d'exécution, paramètres, traces GPX
- Signatures Rust, types de retour, règles métier
- Exemples d'appel côté TypeScript
- Procédure pour ajouter une nouvelle commande

👉 **Consulter ce fichier** pour toute interaction avec le backend.

### 💾 [DATA_STORAGE.md](./DATA_STORAGE.md)
**Schéma de stockage des données**

Comprendre où vit chaque donnée sur disque :
- Arborescence complète (`{app_data_dir}/`)
- Fichiers détaillés (`.env`, `ModeExe.toml`, `traces.json`, `config.toml`)
- Résolution des chemins en fonction du mode actif
- Sécurité des secrets (AES-256-GCM, keyring OS)
- Isolation des données par mode d'exécution

👉 **Lire ce fichier** pour comprendre la persistance.

### 📐 [CONVENTIONS.md](./CONVENTIONS.md)
**Conventions de code et bonnes pratiques**

Guide exhaustif des conventions :
- Nommage (fichiers, variables, fonctions)
- Structure des fichiers (Vue, Stores, Router)
- TypeScript (types, interfaces, typage)
- Vue et Composition API
- Vuetify
- Gestion des erreurs

👉 **Consulter ce fichier** avant d'écrire du code.

### 🔧 [EXTENDING.md](./EXTENDING.md)
**Guide d'extension de l'application**

Tutoriels pratiques pour ajouter :
- Une nouvelle page, un nouveau store Pinia, un composant réutilisable
- Des commandes Tauri, un paramètre de configuration
- Une base de données locale, l'internationalisation (i18n), etc.

👉 **Utiliser ce fichier** comme référence lors du développement.

### 🧩 [SPEC_IMPORT_GPX.md](./SPEC_IMPORT_GPX.md)
**Spécification du module d'import GPX**

Document de référence détaillé du module GPX :
- Architecture cible, gestion des chemins et modes d'exécution
- Backend Rust : parsing, stats (Haversine), détection d'éditeur, registre
- Frontend : store Pinia, composants, notifications
- Cas d'erreur et comportements attendus

👉 **Lire ce fichier** pour comprendre le module traces GPX.

### 🗺️ [SPEC_AFFICHAGE_TRACES.md](./SPEC_AFFICHAGE_TRACES.md)
**Spécification — Favoris & affichage des traces sur la carte**

Document de référence de l'affichage des traces sur la carte Mapbox :
- (Dés)sélection des favoris : couleur de cluster et tracé paramétrables (`material_extended`)
- Création de LineString GeoJSON à l'import et lien ID ↔ trace (`geojson/{id}.geojson`)
- Affichage/masquage des traces via dégradé bleu → rouge (couleur intermédiaire optionnelle)
- Paramètres `[Carte.*]` (dont `Carte.Traces.dureeFlyTo`) et couches Mapbox (`favorites-line`, `displayed-traces-line`, `focus-traces-line`)
- **Focus carte** : clic Info d'un circuit isole et cadre la trace (§8-bis), via l'état `focusedTraceId`
- Commande backend `get_trace_geometry`

> ✅ **Statut** : **implémenté**. Ce document était à l'origine une spécification « à implémenter » ; les fonctionnalités décrites (favoris, affichage dégradé, couches Mapbox, commande `get_trace_geometry`) sont en place. La fonctionnalité de focus carte (clic Info), le paramètre `Carte.Traces.dureeFlyTo` et l'état `focusedTraceId` ont été ajoutés lors de la refonte de `Circuit.vue`.

👉 **Lire ce fichier** pour comprendre l'architecture d'affichage des traces et le mode focus.

### 🤖 [CLAUDE-CODE-GUIDE.md](./CLAUDE-CODE-GUIDE.md)
**Guide d'utilisation avec un assistant IA**

Comment tirer le meilleur parti de ZCode / Claude Code :
- Fichiers de contexte à fournir selon la tâche
- Exemples de prompts efficaces
- Workflow recommandé (comprendre → planifier → implémenter → vérifier)

👉 **Lire ce fichier** pour travailler efficacement avec l'IA.

## Comment utiliser cette documentation

### Pour ZCode (IA)

Lors d'une session de développement, indiquez à l'IA :

```
"Consulte docs/CONTEXT.md pour comprendre le projet"
"Respecte les conventions dans docs/CONVENTIONS.md"
"Suis l'architecture décrite dans docs/ARCHITECTURE.md"
"Référence-toi à docs/COMMANDS.md pour les commandes Tauri"
```

Ou fournissez simplement le chemin vers ces fichiers pour que l'IA comprenne le contexte.

### Pour les développeurs

1. **Première fois** : Lire CONTEXT.md puis ARCHITECTURE.md
2. **Avant de coder** : Parcourir CONVENTIONS.md
3. **Interaction backend** : Référencer COMMANDS.md et DATA_STORAGE.md
4. **Ajout de fonctionnalités** : Référencer EXTENDING.md
5. **En cas de doute** : Rechercher dans la documentation appropriée

## Structure de la documentation

```
docs/
├── README.md                  # Ce fichier (index)
├── CONTEXT.md                 # Contexte général
├── ARCHITECTURE.md            # Architecture technique
├── COMMANDS.md                # Référence des commandes Tauri (20 commandes)
├── DATA_STORAGE.md            # Schéma de stockage des données
├── CONVENTIONS.md             # Conventions de code
├── EXTENDING.md               # Guide d'extension
├── SPEC_IMPORT_GPX.md         # Spécification du module GPX
├── SPEC_AFFICHAGE_TRACES.md   # Spécification favoris & affichage carte (à implémenter)
└── CLAUDE-CODE-GUIDE.md       # Guide d'utilisation avec IA
```

## Maintenance de la documentation

### Quand mettre à jour

Mettre à jour la documentation quand :
- ✅ Ajout d'une nouvelle commande Tauri → mettre à jour **COMMANDS.md**
- ✅ Modification du stockage disque → mettre à jour **DATA_STORAGE.md**
- ✅ Modification de l'architecture ou de la structure des fichiers → **ARCHITECTURE.md**
- ✅ Ajout de couches Mapbox (clustering, sources, popups) → **ARCHITECTURE.md**
- ✅ Ajout d'un utilitaire dans `src/utils/` → **CONVENTIONS.md** + **EXTENDING.md**
- ✅ Ajout d'une convention ou d'un pattern important → **CONVENTIONS.md**
- ✅ Nouvelle dépendance majeure ou nouvelle fonctionnalité → **CONTEXT.md**

### Comment mettre à jour

1. Identifier le document concerné
2. Ajouter/modifier la section appropriée
3. Maintenir la cohérence avec les autres documents
4. Mettre à jour la date en bas du document

## FAQ

### Pourquoi pas d'ESLint/Prettier ?

Choix délibéré pour un environnement minimal. Les conventions sont documentées dans CONVENTIONS.md.

### Pourquoi le Setup Stores pattern pour Pinia ?

Plus proche de la Composition API, meilleure inférence TypeScript, plus flexible.

### Pourquoi Vuetify et pas un autre framework UI ?

Material Design mature, composants riches, documentation excellente, grande communauté.

---

**Dernière mise à jour** : 2026-07-30
**Version de l'application** : 0.0.1
**Status** : En développement actif
