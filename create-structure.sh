#!/bin/bash

# Script de création de la structure de dossiers et fichiers
# Crée l'arborescence du projet et les fichiers de base

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "=============================================="
echo "Création de la structure du projet"
echo "=============================================="
echo ""

echo -e "${BLUE}[INFO]${NC} Création des dossiers..."

# Créer la structure de dossiers
mkdir -p src/router
mkdir -p src/stores
mkdir -p src/plugins
mkdir -p src/views
mkdir -p src/components
mkdir -p src/assets/styles

echo -e "${GREEN}[OK]${NC} Dossiers créés"
echo ""

echo -e "${BLUE}[INFO]${NC} Création des fichiers de base..."

# Créer le fichier router
cat > src/router/index.ts << 'EOF'
import { createRouter, createWebHistory } from 'vue-router'
import Home from '../views/Home.vue'
import About from '../views/About.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: Home
    },
    {
      path: '/about',
      name: 'about',
      component: About
    }
  ]
})

export default router
EOF

# Créer le store Pinia principal
cat > src/stores/index.ts << 'EOF'
import { createPinia } from 'pinia'

export const pinia = createPinia()
EOF

# Créer un store exemple
cat > src/stores/app.ts << 'EOF'
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAppStore = defineStore('app', () => {
  const isDarkMode = ref(false)

  const theme = computed(() => isDarkMode.value ? 'dark' : 'light')

  function toggleDarkMode() {
    isDarkMode.value = !isDarkMode.value
  }

  return {
    isDarkMode,
    theme,
    toggleDarkMode
  }
})
EOF

# Créer le plugin Vuetify
cat > src/plugins/vuetify.ts << 'EOF'
import 'vuetify/styles'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import { aliases, mdi } from 'vuetify/iconsets/mdi'
import '@mdi/font/css/materialdesignicons.css'

export default createVuetify({
  components,
  directives,
  icons: {
    defaultSet: 'mdi',
    aliases,
    sets: {
      mdi,
    },
  },
  theme: {
    defaultTheme: 'light',
    themes: {
      light: {
        colors: {
          primary: '#1976D2',
          secondary: '#424242',
        },
      },
      dark: {
        colors: {
          primary: '#2196F3',
          secondary: '#616161',
        },
      },
    },
  },
})
EOF

# Créer la vue Home
cat > src/views/Home.vue << 'EOF'
<template>
  <v-container>
    <v-row>
      <v-col cols="12">
        <h1>Accueil</h1>
        <p>Bienvenue dans votre application Tauri + Vue + Vuetify + Pinia + Router</p>

        <v-card class="mt-4">
          <v-card-title>Technologies utilisées</v-card-title>
          <v-card-text>
            <v-list>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-language-typescript</v-icon>
                </template>
                <v-list-item-title>TypeScript</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-vuejs</v-icon>
                </template>
                <v-list-item-title>Vue 3</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-material-design</v-icon>
                </template>
                <v-list-item-title>Vuetify 3</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-database</v-icon>
                </template>
                <v-list-item-title>Pinia</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-routes</v-icon>
                </template>
                <v-list-item-title>Vue Router</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-application</v-icon>
                </template>
                <v-list-item-title>Tauri</v-list-item-title>
              </v-list-item>
            </v-list>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
</script>
EOF

# Créer la vue About
cat > src/views/About.vue << 'EOF'
<template>
  <v-container>
    <v-row>
      <v-col cols="12">
        <h1>À propos</h1>
        <p>Cette application est un template réutilisable pour vos projets.</p>

        <v-card class="mt-4">
          <v-card-title>Fonctionnalités</v-card-title>
          <v-card-text>
            <ul>
              <li>Navigation avec Vue Router</li>
              <li>Gestion d'état avec Pinia</li>
              <li>Interface Material Design avec Vuetify</li>
              <li>Application desktop native avec Tauri</li>
              <li>Support TypeScript</li>
            </ul>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
</script>
EOF

# Créer le fichier main.ts
cat > src/main.ts << 'EOF'
import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { pinia } from './stores'
import vuetify from './plugins/vuetify'

createApp(App)
  .use(router)
  .use(pinia)
  .use(vuetify)
  .mount('#app')
EOF

# Créer le fichier App.vue
cat > src/App.vue << 'EOF'
<template>
  <v-app :theme="appStore.theme">
    <v-app-bar color="primary" prominent>
      <v-app-bar-nav-icon @click="drawer = !drawer"></v-app-bar-nav-icon>
      <v-app-bar-title>Mon Application</v-app-bar-title>

      <v-spacer></v-spacer>

      <v-btn icon @click="appStore.toggleDarkMode()">
        <v-icon>{{ appStore.isDarkMode ? 'mdi-weather-night' : 'mdi-weather-sunny' }}</v-icon>
      </v-btn>
    </v-app-bar>

    <v-navigation-drawer v-model="drawer" temporary>
      <v-list>
        <v-list-item
          prepend-icon="mdi-home"
          title="Accueil"
          :to="{ name: 'home' }"
        ></v-list-item>
        <v-list-item
          prepend-icon="mdi-information"
          title="À propos"
          :to="{ name: 'about' }"
        ></v-list-item>
      </v-list>
    </v-navigation-drawer>

    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useAppStore } from './stores/app'

const drawer = ref(false)
const appStore = useAppStore()
</script>
EOF

echo -e "${GREEN}[OK]${NC} Fichiers de base créés"
echo ""
echo "=============================================="
echo -e "${GREEN}[OK]${NC} Structure du projet créée avec succès"
echo "=============================================="
