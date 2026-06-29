<template>
  <v-card min-height="80px" width="500" class="mx-auto">

    <!-- Bande couleur gauche -->
    <div
      :class="backgroundColor"
      class="left-strip"
    ></div>

    <v-card-title
      style="max-height: 40px"
      :class="['d-flex', 'align-center', 'pr-0']"
    >
      <span
        style="display: block; " class="text-headline-small text-truncate"
      >
        {{nom}}
      </span>
      <v-spacer></v-spacer> <!-- A garder pour avoir les menu à droite -->
      <v-menu open-on-hover>
        <template v-slot:activator="{ props }">
          <v-btn icon="mdi-dots-vertical" variant="text" v-bind="props"></v-btn>
        </template>

        <v-list density="compact">
          <v-list-item @click="toggleInfo()">
            <v-icon left small color="blue">mdi-information-outline</v-icon>
            <span class="ml-2">Informations</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small>mdi-pencil</v-icon>
            <span class="ml-2">Éditer</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small>mdi-account-group</v-icon>
            <span class="ml-2">Gérer les groupes...</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small>mdi-sun-thermometer-outline</v-icon>
            <span class="ml-2">Gérer la météo...</span>
          </v-list-item>

          <v-list-item @click="isSelected = !isSelected">
            <v-icon left small color="orange">mdi-star-outline</v-icon>
            <span class="ml-2">{{isSelected ? 'Défavoriser' : 'Favoriser'}}</span>
          </v-list-item>

          <v-list-item @click="isDisplayed = !isDisplayed">
            <v-icon left small color="blue">mdi-map-check-outline</v-icon>
            <span class="ml-2">{{ isDisplayed ? 'Masquer' : 'Afficher' }}</span>
          </v-list-item>

          <v-list-item @click="">
            <v-icon left small color="blue">mdi-export</v-icon>
            <span class="ml-2">Exporter...</span>
          </v-list-item>

          <v-list-item @click="visualiserCircuit">
            <v-icon left small color="green">mdi-video-image</v-icon>
            <span class="text-green-darken-3 ml-2">Visualiser</span>
          </v-list-item>

          <v-divider></v-divider>

          <v-list-item @click="">
            <v-icon left small color="red-darken-3">mdi-delete</v-icon>
            <span class="text-red-darken-3 ml-2">Supprimer...</span>
          </v-list-item>
        </v-list>
      </v-menu>
    </v-card-title>

    <!-- Affichage des données principales -->
    <v-card-text
      style="display: flex; align-items: center"
      class="pt-2"
    >
      <span> Distance : 123.5 km | Dénivelé : 2094 m</span>
      <v-spacer></v-spacer>
      <v-icon v-if="communes && communes > 99" color="green-darken-2" icon="mdi-city-variant" ></v-icon>
      <v-icon v-if="isSelected" color="yellow-darken-2" icon="mdi-star" @click="isSelected = false"></v-icon>
      <v-icon v-if="isDisplayed" color="blue" icon="mdi-map-check" @click="isDisplayed = false"></v-icon>
    </v-card-text>
    
    <!-- Affichage des informations -->
    <v-expand-transition>
      <div v-if="info" @click="info = false" style="cursor: pointer;"  :class="backgroundColor">
        <v-divider></v-divider>
        <v-card-text class="py-2 px-4">
        <b>{{ nom }}</b>
        {{ url }}
        </v-card-text>
      </div>
    </v-expand-transition>

  </v-card>
  <v-divider></v-divider>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { useAppStore } from '../../stores/app'

const appStore = useAppStore()
const router = useRouter()

const info = ref(false)
let infoTimer: ReturnType<typeof setTimeout> | null = null

function toggleInfo() {
  info.value = !info.value

  if (infoTimer) clearTimeout(infoTimer)

  if (info.value) {
    infoTimer = setTimeout(() => {
      info.value = false
    }, 10000) // 10 secondes, à ajuster
  }
}
const isDisplayed = ref(false)
const isSelected = ref(false)

// Props pour le composant Circuit
defineProps<{
  nom?: string
  backgroundColor?: string
  communes?: number
  url?: string
}>()

async function visualiserCircuit() {
  // Recharger les informations des écrans pour détecter en temps réel un branchement
  await appStore.loadDisplays()
  
  if (appStore.displays.length >= 2) {
    try {
      await invoke('open_second_window')
      appStore.isScreenBisOpen = true
    } catch (e) {
      console.error('Erreur lors de l\'ouverture de la seconde fenêtre :', e)
    }
  } else {
    // Configuration mono-écran : afficher ScreenBis.vue dans la fenêtre actuelle
    appStore.isScreenBisOpen = true
    router.push({ name: 'screenBis' })
  }
}
</script>

<style scoped>
.left-strip {
  position: absolute;
  left: 0;
  top: 0;
  width: 10px;
  height: 100%;
  border-radius: 8px 0 0 8px; /* optionnel */
}

.clickable-title {
  cursor: pointer;
}
</style>
