<template>
  <v-app :theme="appStore.theme">
    <v-container class="d-flex flex-column align-center justify-center fill-height">
      <v-btn @click="sendRandom" color="primary" size="large" class="mb-4">
        Envoyer à Accueil
      </v-btn>
      <v-label class="mt-2 mb-6 text-h4">{{ receivedValue ?? '—' }}</v-label>
      
      <!-- Bouton Close demandé -->
      <v-btn @click="onClose" color="error" variant="outlined" size="large">
        Close
      </v-btn>
    </v-container>
  </v-app>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'

const appStore = useAppStore()
const router = useRouter()

const receivedValue = ref<number | null>(null)
let unlisten: UnlistenFn | null = null

onMounted(async () => {
  await appStore.loadDisplays()
  unlisten = await listen<number>('accueil-to-screenbis', (event) => {
    receivedValue.value = event.payload
  })
})

onUnmounted(() => {
  unlisten?.()
})

async function sendRandom() {
  const value = Math.floor(Math.random() * 100) + 501
  await emit('screenbis-to-accueil', value)
}

async function onClose() {
  const currentWindow = getCurrentWindow()
  
  if (currentWindow.label === 'screen-bis') {
    // Si on est dans la fenêtre secondaire, on appelle Rust pour la masquer
    try {
      await invoke('close_second_window')
      // Notifier la fenêtre principale que screenBis est fermée
      await emit('screenbis-closed')
    } catch (e) {
      console.error('Erreur lors de la fermeture de la seconde fenêtre :', e)
    }
  } else {
    // Si on est dans la fenêtre principale (mode mono-écran), on change de vue pour retourner à l'accueil
    appStore.isScreenBisOpen = false
    router.push({ name: 'accueil' })
  }
}
</script>
