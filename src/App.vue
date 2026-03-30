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
