<script setup lang="ts">
import { onMounted } from 'vue'
import { useAppStore } from '../stores/app'
import { useSettingsStore } from '../stores/settings'
import Map from '../components/Accueil/Map.vue'
import AppBar from '../components/Accueil/AppBar.vue'
import CircuitsDrawer from '../components/Accueil/CircuitsDrawer.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import ModeExecutionCard from '../components/Accueil/ModeExecutionCard.vue'

const appStore = useAppStore()
const settingsStore = useSettingsStore()

onMounted(async () => {
  await appStore.loadExecutionEnv()
  await appStore.loadModes()
  await appStore.loadDisplays()
  await settingsStore.loadSettings()
})
</script>

<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <CircuitsDrawer />

      <AppBar
        :theme="appStore.theme"
        @update:theme="appStore.toggleDarkMode()"
        @open-settings="appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen"
      />

      <v-main fill-height style="padding-left: 492px;">
        <Map />
      </v-main>

      <!-- Drawer de paramètres : monté dans le v-layout pour ne pas chevaucher
           l'AppBar. Piloté par _meta, réactif à la route active. -->
      <SettingsDrawer />

      <!-- Dialogue pour la gestion des modes d'exécution -->
      <v-dialog v-model="appStore.showModeDialog" max-width="600px" persistent>
        <ModeExecutionCard />
      </v-dialog>
    </v-layout>
  </v-app>
</template>
