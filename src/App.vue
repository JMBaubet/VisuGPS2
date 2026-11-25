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
import { onMounted, onUnmounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './stores/app'
import { useCommunesStore } from './stores/communes'
import { useHermesStore } from './stores/hermes'
import { useSettingsStore } from './stores/settings'
import { useTunnelsStore } from './stores/tunnels'
import { useUiStore } from './stores/ui'

const appStore = useAppStore()
const communesStore = useCommunesStore()
const hermesStore = useHermesStore()
const settingsStore = useSettingsStore()
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

  // Le serveur se met en veille par défaut : trame WoL systématique au
  // lancement (l'état « réveil en cours » est posé avant le premier await).
  void hermesStore.wake()

  // Moniteur de santé global : ping API permanent, ping système en secours.
  // Les popups d'alerte (service ou serveur) sont rendues dans la vue Hermes.
  void settingsStore.loadSettings()
  hermesStore.startHealthMonitor()

  // Fenêtre principale : charger les displays sans ouvrir automatiquement la 2e fenêtre
  await appStore.loadDisplays()

  // Tâche de fond de rattrapage : détecter les communes des traces déjà
  // nettoyées dont la détection n'a pas encore abouti (traces antérieures à la
  // fonctionnalité). Rien à faire → aucune tâche ne démarre. Lancée dans la
  // **seule** fenêtre principale : la fenêtre secondaire monte aussi l'accueil
  // le temps d'une frame, et deux tâches se disputeraient le serveur hermes.
  void communesStore.startCatchUp()

  // Le serveur hermes dort au lancement : la détection ne peut aboutir qu'une
  // fois l'API éveillée. On relance le rattrapage dès qu'elle redevient
  // joignable — `startCatchUp` est idempotent (rien d'éligible → aucune tâche).
  watch(
    () => hermesStore.apiOnline,
    online => {
      if (online === true) void communesStore.startCatchUp()
    },
  )

  // Progression des téléchargements de tunnels : l'écoute est posée une fois
  // pour la fenêtre principale. Le rattrapage enfile les traces propres qui
  // n'ont pas encore d'analyse de tunnels validée ; sans support OSM configuré,
  // il ne démarre rien.
  void tunnelsStore.bindProgress()
  void tunnelsStore.startCatchUp()
})

onUnmounted(() => {
  hermesStore.stopHealthMonitor()
})
</script>
