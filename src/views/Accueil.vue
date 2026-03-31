<template>
  <v-toolbar>
    <v-spacer></v-spacer>
    <v-btn icon :to="{ name: 'editionCamera' }">
      <v-icon>mdi-camera-image</v-icon>
    </v-btn>
    <v-btn icon @click="appStore.toggleDarkMode()">
      <v-icon>{{ appStore.isDarkMode ? 'mdi-weather-night' : 'mdi-weather-sunny' }}</v-icon>
    </v-btn>
  </v-toolbar>
</template>

<script setup lang="ts">
import { useAppStore } from '../stores/app'

const appStore = useAppStore()
</script>
