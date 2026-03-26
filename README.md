# Template Tauri + Vue + Vuetify + Pinia + Router

Template générique et réutilisable pour créer des applications desktop multiplateformes avec Tauri, Vue 3, Vuetify 3, Pinia et Vue Router.

## ⚠️ IMPORTANT : Lire en premier

**Ce dossier est un TEMPLATE, pas un projet à utiliser directement.**

📖 **[USAGE.md](./USAGE.md)** - Lisez ce fichier en premier pour comprendre comment utiliser ce template correctement.

**TL;DR** : Copiez ce dossier vers un nouveau projet, puis lancez `./setup.sh`

```bash
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"
./setup.sh
```

## Caractéristiques

- **Multiplateforme** : Support macOS et Windows
- **Scripts modulaires** : Scripts bash et PowerShell organisés et réutilisables
- **Installation automatisée** : Un seul script pour tout installer
- **Application minimum fonctionnelle** : Exemple avec navigation et thème dark/light
- **Environnement minimal** : Pas de linting, juste l'essentiel pour démarrer

## Technologies

- **Tauri** - Framework desktop léger et sécurisé
- **Vue 3** - Framework JavaScript progressif avec Composition API
- **Vuetify 3** - Framework UI Material Design
- **Pinia** - Store management officiel pour Vue
- **Vue Router** - Routing officiel pour Vue
- **TypeScript** - Typage statique
- **Vite** - Build tool ultra-rapide

## Documentation

Ce template inclut une **documentation complète pour développeurs et IA** :

📚 **[docs/](./docs/)** - Documentation technique détaillée
- **[CONTEXT.md](./docs/CONTEXT.md)** - Contexte général du projet (à lire en premier)
- **[ARCHITECTURE.md](./docs/ARCHITECTURE.md)** - Architecture technique détaillée
- **[CONVENTIONS.md](./docs/CONVENTIONS.md)** - Conventions de code et bonnes pratiques
- **[EXTENDING.md](./docs/EXTENDING.md)** - Guide d'extension avec exemples

💡 **Pour Claude Code et IA** : Ces fichiers fournissent le contexte nécessaire pour comprendre le projet et respecter les conventions.

## Prérequis

### Tous les systèmes

- **Node.js** v18+ et npm v9+
- **Rust** et Cargo (https://rustup.rs/)

### macOS

```bash
xcode-select --install
```

### Windows

- Visual Studio Build Tools (recommandé)
- Git Bash (pour les scripts bash)

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget \
  file libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

## Installation rapide

⚠️ **Attention** : Ne lancez PAS setup.sh dans ce dossier "Base Tauri" directement !

### macOS / Linux

```bash
# 1. COPIER le template vers un nouveau projet
cd ..  # Remonter d'un niveau
cp -r "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"

# 2. Vérifier les prérequis
chmod +x check-requirements.sh
./check-requirements.sh

# 3. Installer le projet
chmod +x setup.sh
./setup.sh

# 4. Démarrer l'application
./dev.sh
```

### Windows

```powershell
# 1. COPIER le template
Copy-Item -Recurse "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"

# 2. Avec PowerShell
.\check-requirements.ps1
.\setup.ps1
.\dev.ps1

# OU avec Git Bash (recommandé)
./check-requirements.sh
./setup.sh
./dev.sh
```

## Scripts disponibles

### Scripts principaux

| Script | Description |
|--------|-------------|
| `setup.sh` / `setup.ps1` | Installation complète (orchestrateur) |
| `dev.sh` / `dev.ps1` | Démarrer en mode développement |
| `build.sh` / `build.ps1` | Compiler pour production |

### Scripts modulaires

| Script | Description |
|--------|-------------|
| `check-requirements.sh` | Vérifier les prérequis |
| `install-dependencies.sh` | Installer npm, Tauri, Vue, etc. |
| `create-structure.sh` | Créer la structure de dossiers et fichiers |
| `configure-app.sh` | Configurer Vite et TypeScript |

## Structure du projet

```
.
├── src/
│   ├── router/
│   │   └── index.ts          # Configuration Vue Router
│   ├── stores/
│   │   ├── index.ts          # Configuration Pinia
│   │   └── app.ts            # Store exemple (thème)
│   ├── plugins/
│   │   └── vuetify.ts        # Configuration Vuetify
│   ├── views/
│   │   ├── Home.vue          # Page d'accueil
│   │   └── About.vue         # Page à propos
│   ├── components/           # Vos composants
│   ├── assets/               # Images, styles
│   ├── App.vue              # Layout principal
│   └── main.ts              # Point d'entrée
├── src-tauri/               # Code Rust (Tauri)
├── Scripts de maintenance
│   ├── update-metadata.js   # Mise à jour métadonnées
│   └── update-version.js    # Mise à jour version (X.Y.Z)
├── Scripts d'installation
│   ├── setup.sh             # Script principal
│   ├── check-requirements.sh
│   ├── install-dependencies.sh
│   ├── create-structure.sh
│   └── configure-app.sh
├── Scripts de développement
│   ├── dev.sh              # Démarrage développement
│   └── build.sh            # Build production
├── Configuration
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── .gitignore
│   └── .env.example
└── Documentation
    ├── README.md           # Ce fichier
    └── QUICKSTART.md       # Guide rapide
```

## Utilisation

### Développement

```bash
./dev.sh          # macOS/Linux
.\dev.ps1         # Windows PowerShell
npm run tauri dev # Alternative
```

L'application s'ouvre avec hot-reload activé.

### Build de production

```bash
./build.sh         # macOS/Linux
.\build.ps1        # Windows PowerShell
npm run tauri build # Alternative
```

Les exécutables sont dans `src-tauri/target/release/bundle/`

### Commandes npm

```bash
npm install          # Installer les dépendances
npm run dev          # Serveur Vite seul
npm run build        # Build assets frontend
npm run tauri dev    # Développement Tauri
npm run tauri build  # Build Tauri production
```

## Personnalisation

### Modifier le thème Vuetify

Éditez `src/plugins/vuetify.ts` :

```typescript
theme: {
  themes: {
    light: {
      colors: {
        primary: '#1976D2',  // Votre couleur
        secondary: '#424242',
      },
    },
  },
}
```

### Ajouter une route

Éditez `src/router/index.ts` :

```typescript
{
  path: '/nouvelle-page',
  name: 'nouvelle-page',
  component: () => import('../views/NouvellePage.vue')
}
```

### Créer un store Pinia

Créez `src/stores/monstore.ts` :

```typescript
import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useMonStore = defineStore('monstore', () => {
  const data = ref('')

  function setData(value: string) {
    data.value = value
  }

  return { data, setData }
})
```

### Variables d'environnement

Copiez `.env.example` vers `.env` et personnalisez :

```bash
cp .env.example .env
```

Les variables préfixées `VITE_` sont accessibles dans le code :

```typescript
const apiUrl = import.meta.env.VITE_API_URL
```

### Configuration Tauri

Modifiez `src-tauri/tauri.conf.json` pour :
- Changer le nom de l'application
- Modifier la taille de fenêtre
- Configurer les permissions
- Ajouter des icônes

## Réutilisation du template

### Méthode 1 : Copie directe

```bash
cp -r "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"
./setup.sh
```

### Méthode 2 : Repository Git

```bash
git clone votre-repo nouveau-projet
cd nouveau-projet
./setup.sh
```

## Personnalisation et Maintenance

Après avoir copié le template, vous pouvez utiliser les scripts suivants pour configurer et maintenir votre projet :

### Mise à jour des métadonnées

Le script `update-metadata.js` permet de configurer globalement le projet (nom, version, description, auteur). Il met à jour simultanément :
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `vite.config.ts`

Pour l'exécuter :
```bash
node update-metadata.js
```

Ce script effectue également :
- La mise à jour de l'édition Rust vers `2024` dans `Cargo.toml`.
- La mise à jour de la cible ES vers `es2024` dans `vite.config.ts`.
- La création automatique de sauvegardes (`.backup`) avant modification.

### Mise à jour de la version uniquement

Le script `update-version.js` permet de mettre à jour uniquement le numéro de version (format X.Y.Z) dans `package.json`, `Cargo.toml`, et `tauri.conf.json`. C'est utile pour les montées en version rapides sans changer les autres métadonnées.

Pour l'exécuter :
```bash
node update-version.js
```

### Personnalisation manuelle

Si vous préférez ne pas utiliser les scripts, vous devez modifier manuellement : 
1. `src-tauri/tauri.conf.json` (nom, identifiant, titre de la fenêtre)
2. `package.json` (nom, version, description, auteur)
3. `src-tauri/Cargo.toml` (nom, version, description, auteur)
4. Remplacer les icônes dans `src-tauri/icons/`
5. Adapter le contenu des vues dans `src/views/`

## Dépannage

### Erreur "prérequis manquants"

Vérifiez l'installation de Node.js, npm, Rust et Cargo :

```bash
node --version
npm --version
rustc --version
cargo --version
```

### Port 1420 déjà utilisé

Modifiez le port dans `vite.config.ts` :

```typescript
server: {
  port: 1421,  // Nouveau port
}
```

### Erreur build Rust

Nettoyez et recompilez :

```bash
cd src-tauri
cargo clean
cd ..
npm run tauri build
```

### Dépendances npm corrompues

```bash
rm -rf node_modules package-lock.json
npm install
```

### Windows : Scripts PowerShell bloqués

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## Ressources

- [Documentation Tauri](https://tauri.app/)
- [Documentation Vue 3](https://vuejs.org/)
- [Documentation Vuetify 3](https://vuetifyjs.com/)
- [Documentation Pinia](https://pinia.vuejs.org/)
- [Documentation Vue Router](https://router.vuejs.org/)

## Contribution

Ce template est conçu pour être réutilisable et adaptable. N'hésitez pas à le personnaliser selon vos besoins.

## Licence

Libre d'utilisation pour projets personnels et commerciaux.
