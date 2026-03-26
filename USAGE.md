# Guide d'utilisation du Template

> ⚠️ **IMPORTANT** : Ce dossier "Base Tauri" est un TEMPLATE, pas un projet à utiliser directement.

## Comprendre le template

Ce dossier contient :
- ✅ Scripts d'installation (*.sh, *.ps1)
- ✅ Documentation (docs/, README.md, QUICKSTART.md)
- ✅ Fichiers de configuration (.gitignore, .env.example)
- ❌ **PAS de projet Tauri installé**

Le projet Tauri sera créé automatiquement par `setup.sh` dans un NOUVEAU dossier.

## Workflow d'utilisation

### Méthode 1 : Copie complète (Recommandé)

```bash
# 1. Copier le template vers un nouveau projet
cd /Volumes/Externe/Dev/Methode
cp -r "Base Tauri" "MonNouveauProjet"
cd "MonNouveauProjet"

# 2. Lancer l'installation
chmod +x setup.sh
./setup.sh

# ✅ Le projet Tauri est créé automatiquement
# ✅ Toutes les dépendances sont installées
# ✅ La structure est créée

# 3. Démarrer le développement
./dev.sh
```

### Méthode 2 : Nouveau dossier vide

```bash
# 1. Créer un nouveau dossier
mkdir MonProjet
cd MonProjet

# 2. Copier les scripts depuis le template
cp /chemin/vers/Base\ Tauri/*.sh .
cp /chemin/vers/Base\ Tauri/*.ps1 .
cp /chemin/vers/Base\ Tauri/.gitignore .
cp /chemin/vers/Base\ Tauri/.env.example .
cp -r /chemin/vers/Base\ Tauri/docs .
cp /chemin/vers/Base\ Tauri/README.md .
cp /chemin/vers/Base\ Tauri/QUICKSTART.md .

# 3. Lancer l'installation
chmod +x setup.sh
./setup.sh

# 4. Démarrer
./dev.sh
```

### Méthode 3 : Git (Si vous versionnez le template)

```bash
# 1. Cloner ou copier depuis git
git clone <votre-repo-template> MonProjet
cd MonProjet

# 2. Lancer l'installation
./setup.sh

# 3. Démarrer
./dev.sh
```

## Ce que fait setup.sh

```
setup.sh
   ↓
1. Vérifie les prérequis (Node, Rust, etc.)
   ↓
2. Exécute : npm create tauri-app . --template vue-ts
   → Crée le projet Tauri dans le dossier courant
   ↓
3. Installe : Vuetify, Pinia, Vue Router, Sass
   ↓
4. Crée la structure : src/router/, stores/, plugins/, views/
   ↓
5. Génère les fichiers : App.vue, main.ts, router, stores, etc.
   ↓
6. Configure : vite.config.ts, tsconfig.json
   ↓
✅ Projet prêt à l'emploi
```

## ⚠️ Erreur commune

### Erreur : "Directory is not empty"

**Cause** : Vous avez lancé `./setup.sh` directement dans "Base Tauri" qui contient déjà des fichiers.

**Solution** :

```bash
# Option A : Nettoyer et relancer
rm -rf node_modules src-tauri src package.json package-lock.json
./setup.sh

# Option B : Copier dans un nouveau dossier (recommandé)
cd ..
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"
./setup.sh
```

## Structure avant/après setup.sh

### AVANT (Base Tauri - Template)

```
Base Tauri/
├── scripts (*.sh, *.ps1)
├── docs/ (documentation)
├── .gitignore
├── .env.example
├── README.md
└── QUICKSTART.md
```

### APRÈS (MonProjet - Projet installé)

```
MonProjet/
├── scripts (*.sh, *.ps1)
├── docs/
├── src/                    ⭐ CRÉÉ
│   ├── router/
│   ├── stores/
│   ├── plugins/
│   ├── views/
│   ├── components/
│   ├── assets/
│   ├── App.vue
│   └── main.ts
├── src-tauri/              ⭐ CRÉÉ
├── node_modules/           ⭐ CRÉÉ
├── package.json            ⭐ CRÉÉ
├── vite.config.ts          ⭐ CRÉÉ
├── tsconfig.json           ⭐ CRÉÉ
└── index.html              ⭐ CRÉÉ
```

## Cas d'usage

### Cas 1 : Premier projet

```bash
cp -r "Base Tauri" "MonPremierProjet"
cd "MonPremierProjet"
./setup.sh
./dev.sh
```

### Cas 2 : Deuxième projet (réutilisation)

```bash
# Le template Base Tauri n'a pas changé, on le réutilise
cp -r "Base Tauri" "MonDeuxiemeProjet"
cd "MonDeuxiemeProjet"
./setup.sh
./dev.sh
```

### Cas 3 : Personnalisation du projet

```bash
cp -r "Base Tauri" "MonProjetPerso"
cd "MonProjetPerso"

# Installer
./setup.sh

# Personnaliser
# - src-tauri/tauri.conf.json (nom de l'app)
# - package.json (name, version)
# - src/App.vue (titre, contenu)

# Développer
./dev.sh
```

## Windows

### PowerShell

```powershell
# 1. Copier
Copy-Item -Recurse "Base Tauri" "MonProjet"
cd "MonProjet"

# 2. Installer
.\setup.ps1

# 3. Démarrer
.\dev.ps1
```

### Git Bash (Recommandé)

```bash
# Utiliser les mêmes commandes que macOS/Linux
cp -r "Base Tauri" "MonProjet"
cd "MonProjet"
./setup.sh
./dev.sh
```

## FAQ

### Q : Puis-je modifier "Base Tauri" directement ?

**Non.** "Base Tauri" est votre template de référence. Faites toujours une copie pour un nouveau projet.

### Q : Dois-je relancer setup.sh si je fais une erreur ?

**Oui.** Supprimez `node_modules`, `src-tauri`, `package.json` et relancez `./setup.sh`.

### Q : Puis-je versionner "Base Tauri" dans Git ?

**Oui.** Vous pouvez même créer un repo git avec ce template :

```bash
cd "Base Tauri"
git init
git add .
git commit -m "Template initial"
git remote add origin <votre-repo>
git push
```

Puis pour créer un nouveau projet :

```bash
git clone <votre-repo> MonNouveauProjet
cd MonNouveauProjet
./setup.sh
```

### Q : Comment mettre à jour mes projets si je modifie le template ?

Vous pouvez soit :
1. Copier les scripts modifiés manuellement
2. Utiliser Git pour gérer les mises à jour du template

### Q : Le dossier "Base Tauri" doit-il rester intact ?

**Oui.** Gardez "Base Tauri" comme référence propre. Créez toujours des copies pour vos projets.

## Checklist avant de lancer setup.sh

- [ ] Je suis dans un NOUVEAU dossier (copie de Base Tauri)
- [ ] Le dossier ne contient QUE les scripts et la doc
- [ ] Je n'ai PAS de `package.json` ni `src-tauri/` existant
- [ ] J'ai les permissions d'exécution (`chmod +x setup.sh`)
- [ ] Les prérequis sont installés (vérifier avec `./check-requirements.sh`)

## Support

Si vous avez des questions :
1. Consultez QUICKSTART.md pour le guide rapide
2. Lisez README.md pour la documentation complète
3. Vérifiez docs/CONTEXT.md pour comprendre l'architecture

---

**Résumé** : "Base Tauri" = Template à copier. Nouveau projet = Copie + setup.sh
