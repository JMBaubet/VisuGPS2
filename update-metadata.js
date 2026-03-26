#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import readline from 'readline';

// Couleurs pour le terminal
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  blue: '\x1b[34m',
};

// Créer une interface de lecture pour les inputs
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
});

// Fonction helper pour poser une question
const question = (prompt) => {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      resolve(answer);
    });
  });
};

// Valider le format de version (semver)
const validateVersion = (version) => {
  const semverRegex = /^\d+\.\d+\.\d+(-[a-zA-Z0-9]+)?$/;
  return semverRegex.test(version);
};

// Valider le nom du produit
const validateProductName = (name) => {
  return name.trim().length > 0 && name.trim().length <= 100;
};

// Valider la description
const validateDescription = (desc) => {
  return desc.trim().length > 0 && desc.trim().length <= 500;
};

// Valider l'auteur
const validateAuthor = (author) => {
  return author.trim().length > 0 && author.trim().length <= 100;
};

// Parser TOML simple pour Cargo.toml
const parseToml = (content) => {
  const lines = content.split('\n');
  const result = {};
  let currentSection = null;

  lines.forEach((line) => {
    line = line.trim();

    // Ignorer les commentaires et lignes vides
    if (!line || line.startsWith('#')) return;

    // Détecter les sections [section]
    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      if (!result[currentSection]) result[currentSection] = {};
      return;
    }

    // Parser les clés = valeurs
    const kvMatch = line.match(/^([^=]+?)=(.+)$/);
    if (kvMatch && currentSection) {
      let key = kvMatch[1].trim();
      let value = kvMatch[2].trim();

      // Supprimer les guillemets
      if ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))) {
        value = value.slice(1, -1);
      }

      result[currentSection][key] = value;
    }
  });

  return result;
};

// Reconstruire le TOML à partir de l'objet parsé
const stringifyToml = (obj, originalContent) => {
  const lines = originalContent.split('\n');
  let result = [];
  let currentSection = null;
  const processed = new Set();

  lines.forEach((line) => {
    const trimmed = line.trim();

    // Détecter les sections
    const sectionMatch = trimmed.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      result.push(line);
      return;
    }

    // Détecter les clés à mettre à jour
    const kvMatch = trimmed.match(/^([^=]+?)=/);
    if (kvMatch && currentSection && obj[currentSection]) {
      const key = kvMatch[1].trim();
      if (obj[currentSection][key] !== undefined) {
        const newValue = obj[currentSection][key];

        // N'ajouter des guillemets que si la valeur n'est pas déjà formatée (tableau, objet ou déjà citée)
        if (typeof newValue === 'string' &&
          !newValue.startsWith('"') &&
          !newValue.startsWith("'") &&
          !newValue.startsWith('[') &&
          !newValue.startsWith('{')) {
          const comment = key === 'edition' ? ' # Mis à jour vers 2024 par le script update-metadata.js' : '';
          result.push(`${key} = "${newValue}"${comment}`);
        } else {
          result.push(`${key} = ${newValue}`);
        }

        processed.add(`${currentSection}.${key}`);
        return;
      }
    }

    result.push(line);
  });

  return result.join('\n');
};

// Fonction principale
async function main() {
  try {
    console.log(`\n${colors.bright}${colors.blue}🔧 Mise à jour des métadonnées Tauri/Vite${colors.reset}\n`);

    // Vérifier que les fichiers existent
    const files = ['package.json', 'src-tauri/Cargo.toml', 'src-tauri/tauri.conf.json', 'vite.config.ts'];
    const missingFiles = files.filter(f => !fs.existsSync(f));

    if (missingFiles.length > 0) {
      console.log(`${colors.red}❌ Fichiers manquants: ${missingFiles.join(', ')}${colors.reset}`);
      process.exit(1);
    }

    // Lire les fichiers actuels
    const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf-8'));
    const cargoToml = fs.readFileSync('src-tauri/Cargo.toml', 'utf-8');
    const parsedCargo = parseToml(cargoToml);
    const tauriConf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf-8'));

    // Afficher les valeurs actuelles
    console.log(`${colors.yellow}Valeurs actuelles:${colors.reset}`);
    console.log(`  • Produit: ${colors.bright}${packageJson.name}${colors.reset}`);
    console.log(`  • Version: ${colors.bright}${packageJson.version}${colors.reset}`);
    console.log(`  • Description: ${colors.bright}${packageJson.description || '(non définie)'}${colors.reset}`);
    console.log(`  • Auteur: ${colors.bright}${packageJson.author || '(non défini)'}${colors.reset}\n`);

    // Demander les nouvelles valeurs
    let productName = await question(`${colors.blue}Nom du produit${colors.reset} [${packageJson.name}]: `);
    productName = productName.trim() || packageJson.name;
    if (!validateProductName(productName)) {
      throw new Error('Nom du produit invalide');
    }

    let version = await question(`${colors.blue}Version${colors.reset} (format X.Y.Z) [${packageJson.version}]: `);
    version = version.trim() || packageJson.version;
    if (!validateVersion(version)) {
      throw new Error(`Format de version invalide. Utilisez X.Y.Z (ex: 1.2.3)`);
    }

    let description = await question(`${colors.blue}Description${colors.reset} [${packageJson.description || 'N/A'}]: `);
    description = description.trim() || packageJson.description;
    if (!validateDescription(description)) {
      throw new Error('Description invalide');
    }

    let author = await question(`${colors.blue}Auteur${colors.reset} [${packageJson.author || 'N/A'}]: `);
    author = author.trim() || packageJson.author;
    if (!validateAuthor(author)) {
      throw new Error('Auteur invalide');
    }

    // Créer des backups
    console.log(`\n${colors.yellow}📦 Création des backups...${colors.reset}`);
    fs.copyFileSync('package.json', 'package.json.backup');
    fs.copyFileSync('src-tauri/Cargo.toml', 'src-tauri/Cargo.toml.backup');
    fs.copyFileSync('src-tauri/tauri.conf.json', 'src-tauri/tauri.conf.json.backup');

    // Mettre à jour package.json
    packageJson.name = productName.toLowerCase()
      .trim()
      .replace(/\s+/g, '-')
      .replace(/[^a-z0-9._-]/g, '')
      .replace(/^[._]+/, '')
      .slice(0, 214);
    packageJson.version = version;
    packageJson.description = description;
    packageJson.author = author;
    fs.writeFileSync('package.json', JSON.stringify(packageJson, null, 2) + '\n');

    // Mettre à jour Cargo.toml
    if (!parsedCargo.package) parsedCargo.package = {};
    parsedCargo.package.name = productName.toLowerCase().replace(/\s+/g, '-');
    parsedCargo.package.version = version;
    parsedCargo.package.description = description;
    parsedCargo.package.edition = '2024'; // Forcer l'édition 2024

    // authors doit être un tableau dans Cargo.toml
    if (author.startsWith('[') && author.endsWith(']')) {
      parsedCargo.package.authors = author;
    } else {
      parsedCargo.package.authors = `["${author}"]`;
    }
    const updatedCargo = stringifyToml(parsedCargo, cargoToml);
    fs.writeFileSync('src-tauri/Cargo.toml', updatedCargo);

    // Mettre à jour tauri.conf.json
    if (!tauriConf.app) tauriConf.app = {};
    tauriConf.productName = productName;
    tauriConf.version = version;

    const slugName = productName.toLowerCase().replace(/\s+/g, '-');

    // Mettre à jour l'identifier (remplacer tauri-app par le nom du produit)
    if (tauriConf.identifier && tauriConf.identifier.includes('tauri-app')) {
      tauriConf.identifier = tauriConf.identifier.replace('tauri-app', slugName);
    }

    // Mettre à jour le titre de la première fenêtre
    if (tauriConf.app && tauriConf.app.windows && tauriConf.app.windows[0]) {
      tauriConf.app.windows[0].title = productName;
    }

    if (!tauriConf.build) tauriConf.build = {};
    //tauriConf.build.devPath = tauriConf.build.devPath || 'http://localhost:5173';
    fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(tauriConf, null, 2) + '\n');

    // Mettre à jour vite.config.ts
    console.log(`${colors.yellow}⚙️  Mise à jour de vite.config.ts...${colors.reset}`);
    fs.copyFileSync('vite.config.ts', 'vite.config.ts.backup');
    let viteConfig = fs.readFileSync('vite.config.ts', 'utf-8');

    const targetRegExp = /target:\s*\[[^\]]+\]/;
    const newTarget = "target: ['es2024', 'chrome120', 'safari17'], // Mis à jour vers ES2024 par le script update-metadata.js";

    if (targetRegExp.test(viteConfig)) {
      viteConfig = viteConfig.replace(targetRegExp, newTarget);
      fs.writeFileSync('vite.config.ts', viteConfig);
      console.log(`  ✓ vite.config.ts`);
    }

    // Afficher les résultats
    console.log(`\n${colors.green}✅ Mise à jour réussie!${colors.reset}\n`);
    console.log(`${colors.yellow}Fichiers modifiés:${colors.reset}`);
    console.log(`  ✓ package.json`);
    console.log(`  ✓ src-tauri/Cargo.toml`);
    console.log(`  ✓ src-tauri/tauri.conf.json`);
    console.log(`  ✓ vite.config.ts\n`);
    console.log(`${colors.yellow}Nouvelles valeurs:${colors.reset}`);
    console.log(`  • Produit: ${colors.bright}${productName}${colors.reset}`);
    console.log(`  • Version: ${colors.bright}${version}${colors.reset}`);
    console.log(`  • Description: ${colors.bright}${description}${colors.reset}`);
    console.log(`  • Auteur: ${colors.bright}${author}${colors.reset}\n`);
    console.log(`${colors.yellow}Backups créés:${colors.reset}`);
    console.log(`  • package.json.backup`);
    console.log(`  • src-tauri/Cargo.toml.backup`);
    console.log(`  • src-tauri/tauri.conf.json.backup\n`);

    rl.close();
    process.exit(0);

  } catch (error) {
    console.error(`\n${colors.red}❌ Erreur: ${error.message}${colors.reset}\n`);
    rl.close();
    process.exit(1);
  }
}

main();
