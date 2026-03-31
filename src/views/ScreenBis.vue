<template>
  <v-app :theme="appStore.theme">
    <v-container class="d-flex flex-column align-center justify-center fill-height">
      <v-btn @click="sendRandom" color="primary" size="large">
        Envoyer à Accueil
      </v-btn>
      <v-label class="mt-6 text-h4">{{ receivedValue ?? '—' }}</v-label>
    </v-container>
  </v-app>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/app'

const appStore = useAppStore()

const receivedValue = ref<number | null>(null)
let unlisten: UnlistenFn | null = null

onMounted(async () => {
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
</script>
