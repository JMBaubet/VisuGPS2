<script setup lang="ts">
import { useAppStore } from '../stores/app'

const appStore = useAppStore()
</script>

<template>
  <!-- Informations sur le mode d'exécution-->
  <v-btn-toggle density="compact" class="ml-4">
    <v-btn
      v-if="appStore.isDev"
      icon="mdi-tools"
      :class="appStore.activeMode !== 'OPE' ? 'bg-blue' : 'bg-green'"
      variant="tonal"
      @click="appStore.showModeDialog = true"
      title="Environnement de développement"
    ></v-btn>
    <v-btn
      :text="appStore.activeMode !== 'OPE' ? appStore.activeModeDescription : 'OK'"
      :class="appStore.activeMode !== 'OPE' ? 'bg-blue' : 'bg-green'"
      variant="tonal"
      @click="appStore.showModeDialog = true"
      title="Gérer les modes d'exécution"
    ></v-btn>
    <v-btn
      v-if="appStore.isDebug"
      icon="mdi-bug"
      :class="appStore.activeMode !== 'OPE' ? 'bg-blue' : 'bg-green'"
      title="Mode Debug actif"
      variant="tonal"
      @click="appStore.toggleDebug()"
    ></v-btn>
  </v-btn-toggle>
</template>
