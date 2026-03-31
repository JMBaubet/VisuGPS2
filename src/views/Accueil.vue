<template>
  <div>
    <v-toolbar>
      <v-spacer></v-spacer>
      <v-btn icon :to="{ name: 'editionCamera' }">
        <v-icon>mdi-camera-image</v-icon>
      </v-btn>
      <v-btn icon @click="appStore.toggleDarkMode()">
        <v-icon>{{ appStore.isDarkMode ? 'mdi-weather-night' : 'mdi-weather-sunny' }}</v-icon>
      </v-btn>
    </v-toolbar>

    <v-container class="mt-4">
      <h1>Accueil</h1>

      <!-- Communication bi-écran -->
      <div v-if="appStore.displays.length >= 2" class="mt-4 d-flex align-center ga-4">
        <v-btn @click="sendRandom" color="primary">
          Envoyer à ScreenBis
        </v-btn>
        <v-label class="text-h5">{{ receivedValue ?? '—' }}</v-label>
      </div>

      <div v-if="appStore.loading" class="mt-4">
        <v-progress-circular indeterminate></v-progress-circular>
        <p>Chargement des informations d'affichage...</p>
      </div>

      <div v-else class="mt-4">
        <h2>Informations d'affichage</h2>
        <div v-if="appStore.displays.length === 0" class="mt-2">
          <v-alert type="warning">Aucun affichage détecté</v-alert>
        </div>

        <div v-for="(display, index) in appStore.displays" :key="index" class="mt-4">
          <v-card>
            <v-card-title>
              Écran {{ index + 1 }}
              <span v-if="display.name" class="ml-2 text-subtitle-2">{{ display.name }}</span>
            </v-card-title>
            <v-card-text>
              <v-list>
                <v-list-item>
                  <template v-slot:prepend>
                    <v-icon>mdi-monitor</v-icon>
                  </template>
                  <v-list-item-title>Résolution</v-list-item-title>
                  <v-list-item-subtitle>{{ display.size[0] }} × {{ display.size[1] }} px</v-list-item-subtitle>
                </v-list-item>
                <v-list-item>
                  <template v-slot:prepend>
                    <v-icon>mdi-crosshairs</v-icon>
                  </template>
                  <v-list-item-title>Position</v-list-item-title>
                  <v-list-item-subtitle>X: {{ display.position[0] }}, Y: {{ display.position[1] }}</v-list-item-subtitle>
                </v-list-item>
                <v-list-item>
                  <template v-slot:prepend>
                    <v-icon>mdi-magnify-plus-outline</v-icon>
                  </template>
                  <v-list-item-title>Facteur d'échelle</v-list-item-title>
                  <v-list-item-subtitle>{{ display.scale_factor.toFixed(2) }}x</v-list-item-subtitle>
                </v-list-item>
              </v-list>
            </v-card-text>
          </v-card>
        </div>
      </div>
    </v-container>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/app'

const appStore = useAppStore()
const receivedValue = ref<number | null>(null)
let unlisten: UnlistenFn | null = null

onMounted(async () => {
  unlisten = await listen<number>('screenbis-to-accueil', (event) => {
    receivedValue.value = event.payload
  })
})

onUnmounted(() => {
  unlisten?.()
})

async function sendRandom() {
  const value = Math.floor(Math.random() * 100) + 1
  await emit('accueil-to-screenbis', value)
}
</script>
