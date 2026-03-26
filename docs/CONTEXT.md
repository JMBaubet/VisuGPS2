# Contexte du Projet - Template Tauri + Vue + Vuetify + Pinia + Router

> **Documentation destinée à Claude Code et aux développeurs**
> Ce fichier fournit le contexte complet du projet pour faciliter le développement assisté par IA.

## Vue d'ensemble

Ce projet est un **template réutilisable** pour créer des applications desktop multiplateformes. Il n'est PAS une application finale, mais une base de départ à copier pour de nouveaux projets.

### Objectif du template
- Fournir une installation automatisée en un seul script
- Créer une structure de projet cohérente et organisée
- Inclure toutes les technologies modernes pré-configurées
- Être réutilisable pour de multiples projets

### Principes de conception
1. **Simplicité** : Environnement minimal sans outils de linting
2. **Modularité** : Scripts bash séparés orchestrés par un script principal
3. **Multiplateforme** : Support macOS et Windows (bash + PowerShell)
4. **Documentation complète** : Chaque aspect est documenté
5. **Prêt à l'emploi** : Application exemple fonctionnelle incluse

## Stack technique

### Frontend
- **Vue 3.4+** avec Composition API et `<script setup>`
- **TypeScript** pour le typage statique
- **Vuetify 3.5+** pour les composants UI Material Design
- **Pinia** pour la gestion d'état (setup stores pattern)
- **Vue Router 4.2+** pour la navigation
- **Vite** comme build tool

### Backend/Desktop
- **Tauri 1.x** pour l'application desktop native
- **Rust** pour le backend Tauri

### Outils
- **npm** comme gestionnaire de packages
- **Sass** pour les styles (via Vuetify)
- **vite-plugin-vuetify** pour l'auto-import des composants

## Architecture du projet

### Structure des fichiers

```
Base Tauri/
├── scripts/                    # Scripts d'installation (à la racine)
│   ├── setup.sh               # Script principal
│   ├── check-requirements.sh
│   ├── install-dependencies.sh
│   ├── create-structure.sh
│   ├── configure-app.sh
│   ├── dev.sh
│   └── build.sh
│
├── src/                       # Code source frontend
│   ├── router/
│   │   └── index.ts          # Configuration des routes
│   ├── stores/
│   │   ├── index.ts          # Configuration Pinia
│   │   └── app.ts            # Store exemple (thème)
│   ├── plugins/
│   │   └── vuetify.ts        # Configuration Vuetify
│   ├── views/                # Pages de l'application
│   │   ├── Home.vue
│   │   └── About.vue
│   ├── components/           # Composants réutilisables (vide au départ)
│   ├── assets/               # Images, styles
│   ├── App.vue              # Layout principal
│   └── main.ts              # Point d'entrée
│
├── src-tauri/                # Code Rust (généré par Tauri)
│   ├── src/
│   │   └── main.rs
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── docs/                     # Documentation de contexte
│   ├── CONTEXT.md           # Ce fichier
│   ├── ARCHITECTURE.md
│   └── CONVENTIONS.md
│
├── Configuration
│   ├── vite.config.ts
│   ├── tsconfig.json
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

```typescript
import { invoke } from '@tauri-apps/api/tauri'

// Appeler une commande Rust
const result = await invoke<string>('my_command', { arg: 'value' })
```

Côté Rust :
```rust
#[tauri::command]
fn my_command(arg: String) -> String {
    format!("Result: {}", arg)
}
```

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

## Application exemple

L'application générée par `setup.sh` contient :

### Pages
- **Home** (`/`) : Page d'accueil avec liste des technologies
- **About** (`/about`) : Page à propos avec infos sur le template

### Fonctionnalités
- Navigation drawer (menu hamburger)
- Toggle thème dark/light (via store Pinia)
- App bar avec titre
- Routing fonctionnel

### Store exemple
`src/stores/app.ts` gère le thème :
- `isDarkMode` : état du thème
- `theme` : computed qui retourne 'dark' ou 'light'
- `toggleDarkMode()` : action pour basculer

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

1. Modifier `src-tauri/src/main.rs`
2. Définir `#[tauri::command]`
3. Ajouter dans `.invoke_handler()`
4. Appeler avec `invoke()` côté frontend

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

---

**Dernière mise à jour** : 2026-03-24
**Version du template** : 1.0.0
