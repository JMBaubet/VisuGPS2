<template>
  <v-app :theme="appStore.theme">
    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './stores/app'

const appStore = useAppStore()

onMounted(async () => {
  await appStore.loadDisplays()

  // N'ouvrir la 2e fenêtre que depuis la fenêtre principale
  const currentWindow = getCurrentWindow()
  if (currentWindow.label === 'main' && appStore.displays.length >= 2) {
    const screen2 = appStore.displays[1]
    await invoke('open_second_window', {
      x: screen2.position[0],
      y: screen2.position[1],
      width: screen2.size[0],
      height: screen2.size[1]
    })
  }
})
</script>
