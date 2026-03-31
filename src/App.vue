<template>
  <v-app :theme="appStore.theme">
    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './stores/app'

const appStore = useAppStore()
const router = useRouter()

onMounted(async () => {
  const currentWindow = getCurrentWindow()

  if (currentWindow.label === 'screen-bis') {
    // Fenêtre secondaire : naviguer directement vers le composant ScreenBis
    await router.replace({ name: 'screenBis' })
    return
  }

  // Fenêtre principale : charger les displays et ouvrir la 2e fenêtre si bi-écran
  await appStore.loadDisplays()
  if (appStore.displays.length >= 2) {
    await invoke('open_second_window')
  }
})
</script>
