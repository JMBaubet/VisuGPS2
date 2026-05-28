<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useAppStore } from '../stores/app'
import Map from '../components/Accueil/Map.vue'
import AppBar from '../components/Accueil/AppBar.vue'
import CircuitsDrawer from '../components/Accueil/CircuitsDrawer.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import ModeExecutionCard from '../components/Accueil/ModeExecutionCard.vue'

const appStore = useAppStore()
const setting = ref(false)

onMounted(async () => {
  await appStore.loadExecutionEnv()
  await appStore.loadModes()
  await appStore.loadDisplays()
})
</script>

<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <CircuitsDrawer />
      
      <AppBar 
        :theme="appStore.theme" 
        @update:theme="appStore.toggleDarkMode()" 
        @open-settings="setting = !setting"
      />  

      <v-main fill-height style="padding-left: 492px;">       
        <Map />
      </v-main>

      <!-- Utiliser le composant avec v-model -->
      <SettingsDrawer v-model="setting" />

      <!-- Dialogue pour la gestion des modes d'exécution -->
      <v-dialog v-model="appStore.showModeDialog" max-width="600px" persistent>
        <ModeExecutionCard />
      </v-dialog>
    </v-layout>
  </v-app>
</template>
