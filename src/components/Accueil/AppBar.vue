<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from '../../stores/app'
import ModeExecutionBtnToggle from '../ModeExecutionBtnToggle.vue'

const appStore = useAppStore()

const props = defineProps<{
  theme: string
}>()

const emit = defineEmits<{
  (e: 'update:theme', val: string): void
  (e: 'open-settings'): void
}>()

const showQuitDialog = ref(false)

function changeMode() {
  emit('update:theme', props.theme === 'light' ? 'dark' : 'light')
}

function askQuit() {
  showQuitDialog.value = true
}

async function confirmQuit() {
  showQuitDialog.value = false
  await invoke('exit_app')
}

// Écouter l'événement de fermeture de screenBis (mode multi-fenêtres)
let unlistenScreenBisClosed: UnlistenFn | null = null

onMounted(async () => {
  unlistenScreenBisClosed = await listen('screenbis-closed', () => {
    appStore.isScreenBisOpen = false
  })
})

onUnmounted(() => {
  unlistenScreenBisClosed?.()
})
</script>

<template>
  <v-app-bar>
    <!-- Informations sur le mode d'exécution-->
    <ModeExecutionBtnToggle />
    <v-spacer></v-spacer>
    
    <v-icon
      class="ma-2"
      color="green"
      icon="mdi-web-check"
    ></v-icon>
  
    <v-btn
      :icon="props.theme === 'light' ? 'mdi-weather-sunny' : 'mdi-weather-night'"
      slim
      @click="changeMode"
    ></v-btn>
    <v-btn
      icon="mdi-cog-outline"
      slim
      @click="emit('open-settings')"
    ></v-btn>
    <v-btn
      color="red"
      icon="mdi-exit-to-app"
      :disabled="appStore.isScreenBisOpen"
      @click="askQuit"
    ></v-btn>

    <!-- Dialogue de confirmation de fermeture -->
    <v-dialog v-model="showQuitDialog" max-width="400" persistent>
      <v-card>
        <v-card-title class="text-h6">
          Quitter l'application
        </v-card-title>
        <v-card-text>
          Êtes-vous sûr de vouloir quitter VisuGPS2 ?
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="showQuitDialog = false">
            Annuler
          </v-btn>
          <v-btn color="red" variant="flat" @click="confirmQuit">
            Quitter
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-app-bar>
</template>

