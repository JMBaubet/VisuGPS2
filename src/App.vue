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
import { useUiStore } from './stores/ui'

const appStore = useAppStore()
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
})
</script>
