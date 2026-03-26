# Démarrage Rapide

Guide express pour utiliser ce template en 3 minutes.

## ⚠️ IMPORTANT

**Ce dossier "Base Tauri" est un TEMPLATE.** Vous devez d'abord le copier vers un nouveau projet.

Voir **[USAGE.md](./USAGE.md)** pour les détails complets.

## Installation en 4 étapes

### macOS / Linux

```bash
# 1. COPIER vers un nouveau projet
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"

# 2. Vérifier les prérequis
./check-requirements.sh

# 3. Installer tout
./setup.sh

# 4. Lancer l'app
./dev.sh
```

### Windows (PowerShell)

```powershell
# 1. COPIER vers un nouveau projet
Copy-Item -Recurse "Base Tauri" "MonProjet"
cd "MonProjet"

# 2-4. Installer et lancer
.\check-requirements.ps1
.\setup.ps1
.\dev.ps1
```

### Windows (Git Bash - recommandé)

```bash
# 1. COPIER vers un nouveau projet
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"

# 2-4. Installer et lancer
./check-requirements.sh
./setup.sh
./dev.sh
```

## Que fait setup.sh ?

Le script d'installation fait automatiquement :

1. Vérifie Node.js, npm, Rust, Cargo
2. Crée le projet Tauri avec Vue + TypeScript
3. Installe Vuetify, Pinia, Vue Router
4. Crée la structure de dossiers
5. Génère les fichiers de configuration
6. Crée une application exemple fonctionnelle

## Commandes principales

```bash
./setup.sh     # Installation complète
./dev.sh       # Mode développement
./build.sh     # Compiler pour production
```

## Structure créée

```
src/
├── router/index.ts      # Routes (Home, About)
├── stores/app.ts        # Store Pinia (thème dark/light)
├── plugins/vuetify.ts   # Configuration Vuetify
├── views/
│   ├── Home.vue         # Page d'accueil
│   └── About.vue        # Page à propos
├── components/          # Vos composants
├── App.vue              # Layout avec navigation
└── main.ts              # Point d'entrée
```

## Premiers pas

### 1. Modifier l'interface

Éditez `src/App.vue` ou les vues dans `src/views/`

### 2. Ajouter une route

`src/router/index.ts` :

```typescript
{
  path: '/ma-page',
  name: 'ma-page',
  component: () => import('../views/MaPage.vue')
}
```

### 3. Créer un store

`src/stores/user.ts` :

```typescript
import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useUserStore = defineStore('user', () => {
  const name = ref('')

  function setName(value: string) {
    name.value = value
  }

  return { name, setName }
})
```

Utilisation :

```vue
<script setup lang="ts">
import { useUserStore } from '@/stores/user'
const user = useUserStore()
</script>

<template>
  <p>{{ user.name }}</p>
</template>
```

### 4. Utiliser Vuetify

Tous les composants Vuetify sont disponibles :

```vue
<template>
  <v-btn color="primary">Mon bouton</v-btn>

  <v-card>
    <v-card-title>Titre</v-card-title>
    <v-card-text>Contenu</v-card-text>
  </v-card>

  <v-icon>mdi-home</v-icon>
</template>
```

Voir : https://vuetifyjs.com/en/components/all/

### 5. Variables d'environnement

```bash
cp .env.example .env
```

Dans `.env` :

```
VITE_API_URL="http://localhost:3000"
```

Dans le code :

```typescript
const apiUrl = import.meta.env.VITE_API_URL
```

## Réutiliser le template

### Pour un nouveau projet

```bash
# Copier le dossier
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"

# Nettoyer et réinstaller
rm -rf node_modules src-tauri package-lock.json
./setup.sh

# Personnaliser
# - src-tauri/tauri.conf.json (nom de l'app)
# - package.json (name, version)
```

## Dépannage rapide

### Prérequis manquants

```bash
# Vérifier les installations
node --version   # doit être v18+
npm --version    # doit être 9+
rustc --version  # doit être installé
cargo --version  # doit être installé
```

### Port 1420 utilisé

Modifiez `vite.config.ts` ligne `port: 1420` vers `port: 1421`

### Erreur de dépendances

```bash
rm -rf node_modules package-lock.json
npm install
```

### Build Rust échoue

```bash
cd src-tauri
cargo clean
cd ..
npm run tauri build
```

## Exemples de code

### Navigation programmatique

```vue
<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()

function goToAbout() {
  router.push({ name: 'about' })
}
</script>

<template>
  <v-btn @click="goToAbout">Aller à About</v-btn>
</template>
```

### Thème dark/light (déjà inclus)

```vue
<script setup lang="ts">
import { useAppStore } from '@/stores/app'
const appStore = useAppStore()
</script>

<template>
  <v-app :theme="appStore.theme">
    <v-btn @click="appStore.toggleDarkMode()">
      Toggle theme
    </v-btn>
  </v-app>
</template>
```

### Appeler une API Tauri

```typescript
import { invoke } from '@tauri-apps/api/tauri'

// Appeler une commande Rust
const result = await invoke('ma_commande', { arg: 'valeur' })
```

Côté Rust (`src-tauri/src/main.rs`) :

```rust
#[tauri::command]
fn ma_commande(arg: String) -> String {
    format!("Reçu: {}", arg)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ma_commande])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Ressources

- **Tauri** : https://tauri.app/
- **Vue 3** : https://vuejs.org/
- **Vuetify** : https://vuetifyjs.com/
- **Pinia** : https://pinia.vuejs.org/
- **Vue Router** : https://router.vuejs.org/

## Aide

Consultez le [README.md](README.md) pour la documentation complète.
