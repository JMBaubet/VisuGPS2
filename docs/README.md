# Documentation du Template

> Documentation complète pour développeurs et IA (Claude Code)

## Vue d'ensemble

Cette documentation fournit un contexte complet pour travailler avec ce template Tauri + Vue + Vuetify + Pinia + Router. Elle est particulièrement utile pour :

- **Claude Code et autres IA** : Comprendre l'architecture et les conventions
- **Nouveaux développeurs** : Onboarding rapide sur le projet
- **Référence** : Guide des bonnes pratiques et patterns

## Documents disponibles

### 📋 [CONTEXT.md](./CONTEXT.md)
**Contexte général du projet**

Le point de départ essentiel. Explique :
- L'objectif et la philosophie du template
- La stack technique utilisée
- La structure générale des fichiers
- Les patterns et conventions de base
- Les points d'extension

👉 **Commencer par ce fichier** si vous découvrez le projet.

### 🏗️ [ARCHITECTURE.md](./ARCHITECTURE.md)
**Architecture technique détaillée**

Plongée approfondie dans l'architecture :
- Couches de l'application (Frontend, Router, State, Build, Desktop)
- Communication entre les couches
- Configuration de chaque outil (Vite, Tauri, Vuetify, etc.)
- Flux de données
- Patterns architecturaux
- Diagrammes

👉 **Lire ce fichier** pour comprendre le fonctionnement interne.

### 📐 [CONVENTIONS.md](./CONVENTIONS.md)
**Conventions de code et bonnes pratiques**

Guide exhaustif des conventions :
- Nommage (fichiers, variables, fonctions)
- Structure des fichiers (Vue, Stores, Router)
- TypeScript (types, interfaces, typage)
- Vue et Composition API
- Vuetify
- Gestion des erreurs
- Imports/exports

👉 **Consulter ce fichier** avant d'écrire du code.

### 🔧 [EXTENDING.md](./EXTENDING.md)
**Guide d'extension du template**

Tutoriels pratiques pour ajouter :
- Une nouvelle page
- Un nouveau store Pinia
- Un composant réutilisable
- Une API externe
- Des commandes Tauri
- L'authentification
- Une base de données locale
- L'internationalisation (i18n)
- Des notifications
- La gestion de formulaires

👉 **Utiliser ce fichier** comme référence lors du développement.

## Comment utiliser cette documentation

### Pour Claude Code (IA)

Lors d'une session de développement, indiquez à Claude Code :

```
"Consulte docs/CONTEXT.md pour comprendre le projet"
"Respecte les conventions dans docs/CONVENTIONS.md"
"Suis l'architecture décrite dans docs/ARCHITECTURE.md"
```

Ou fournissez simplement le chemin vers ces fichiers pour que Claude comprenne le contexte.

### Pour les développeurs

1. **Première fois** : Lire CONTEXT.md puis ARCHITECTURE.md
2. **Avant de coder** : Parcourir CONVENTIONS.md
3. **Ajout de fonctionnalités** : Référencer EXTENDING.md
4. **En cas de doute** : Rechercher dans la documentation appropriée

## Structure de la documentation

```
docs/
├── README.md           # Ce fichier (index)
├── CONTEXT.md          # Contexte général (8KB)
├── ARCHITECTURE.md     # Architecture technique (12KB)
├── CONVENTIONS.md      # Conventions de code (15KB)
└── EXTENDING.md        # Guide d'extension (18KB)
```

## Maintien de la documentation

### Quand mettre à jour

Mettre à jour la documentation quand :
- ✅ Ajout d'une nouvelle convention
- ✅ Modification de l'architecture
- ✅ Changement de la structure des fichiers
- ✅ Ajout d'un pattern important
- ✅ Nouvelle dépendance majeure

### Comment mettre à jour

1. Identifier le document concerné
2. Ajouter/modifier la section appropriée
3. Maintenir la cohérence avec les autres documents
4. Mettre à jour la date en bas du document

## Principes de documentation

Cette documentation suit ces principes :

1. **Clarté** : Explications simples et directes
2. **Exemples** : Code concret plutôt que théorie
3. **Cohérence** : Même format dans tous les documents
4. **Complétude** : Couvre tous les aspects importants
5. **Maintenance** : Mise à jour régulière avec le code

## Ressources externes

### Documentation officielle

- **Tauri** : https://tauri.app/v1/guides/
- **Vue 3** : https://vuejs.org/guide/introduction.html
- **Vuetify 3** : https://vuetifyjs.com/en/getting-started/installation/
- **Pinia** : https://pinia.vuejs.org/introduction.html
- **Vue Router** : https://router.vuejs.org/guide/
- **Vite** : https://vitejs.dev/guide/
- **TypeScript** : https://www.typescriptlang.org/docs/

### Communautés

- **Discord Tauri** : https://discord.com/invite/tauri
- **Discord Vue** : https://discord.com/invite/vue
- **Discord Vuetify** : https://discord.gg/vuetify
- **Forum Vue** : https://forum.vuejs.org/

## FAQ

### Pourquoi pas d'ESLint/Prettier ?

Choix délibéré pour un environnement minimal. Les conventions sont documentées dans CONVENTIONS.md.

### Pourquoi le Setup Stores pattern pour Pinia ?

Plus proche de la Composition API, meilleure inférence TypeScript, plus flexible.

### Pourquoi Vuetify et pas un autre framework UI ?

Material Design mature, composants riches, documentation excellente, grande communauté.

### Comment contribuer à la documentation ?

1. Suivre le format existant
2. Ajouter des exemples concrets
3. Tester les exemples de code
4. Maintenir la cohérence terminologique

## Changelog de la documentation

### Version 1.0.0 (2026-03-24)
- ✨ Création initiale de la documentation
- 📋 CONTEXT.md : Contexte général du projet
- 🏗️ ARCHITECTURE.md : Architecture technique
- 📐 CONVENTIONS.md : Conventions de code
- 🔧 EXTENDING.md : Guide d'extension
- 📚 README.md : Index de la documentation

---

**Dernière mise à jour** : 2026-03-24
**Version du template** : 1.0.0
**Mainteneur** : Template générique réutilisable
