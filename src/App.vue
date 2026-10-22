<template>
  <v-app :theme="appStore.theme">
    <v-main>
      <router-view />
    </v-main>

    <!-- Snackbar mutualisé (piloté par le store ui.ts) -->
    <v-snackbar
      v-model="ui.snackbar.show"
      :color="ui.snackbar.color"
      :timeout="4000"
      location="bottom left"
      rounded="pill"
    >
      {{ ui.snackbar.message }}
      <template #actions>
        <v-btn icon="mdi-close" variant="text" @click="ui.hideSnackbar()" />
      </template>
    </v-snackbar>
  </v-app>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './stores/app'
import { useCommunesStore } from './stores/communes'
import { useTunnelsStore } from './stores/tunnels'
import { useUiStore } from './stores/ui'

const appStore = useAppStore()
const communesStore = useCommunesStore()
const tunnelsStore = useTunnelsStore()
const ui = useUiStore()
const router = useRouter()

onMounted(async () => {
  const currentWindow = getCurrentWindow()

  if (currentWindow.label === 'screen-bis') {
    // Fenêtre secondaire : naviguer directement vers le composant ScreenBis
    await router.replace({ name: 'screenBis' })
    return
  }

  // Fenêtre principale : charger les displays sans ouvrir automatiquement la 2e fenêtre
  await appStore.loadDisplays()

  // Tâche de fond de rattrapage : détecter les communes des traces déjà
  // nettoyées dont la détection n'a pas encore abouti (traces antérieures à la
  // fonctionnalité). Rien à faire → aucune tâche ne démarre. Lancée dans la
  // **seule** fenêtre principale : la fenêtre secondaire monte aussi l'accueil
  // le temps d'une frame, et deux tâches se disputeraient l'API Overpass.
  void communesStore.startCatchUp()

  // Progression des téléchargements de tunnels : l'écoute est posée une fois
  // pour la fenêtre principale. Le rattrapage enfile les traces propres qui
  // n'ont pas encore d'analyse de tunnels validée ; sans support OSM configuré,
  // il ne démarre rien.
  void tunnelsStore.bindProgress()
  void tunnelsStore.startCatchUp()
})
</script>
