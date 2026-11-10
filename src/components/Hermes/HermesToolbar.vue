<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Retour à l'accueil"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <div class="hermes-toolbar-title">
        <v-icon
          icon="mdi-server-network"
          size="18"
          :color="stateColor"
          class="mr-2"
        />
        Hermes
      </div>
    </v-app-bar-title>

    <template #append>
      <v-btn
        icon="mdi-refresh"
        variant="text"
        title="Actualiser les données"
        :loading="loading"
        @click="emit('refresh')"
      />
      <v-btn
        icon="mdi-cog-outline"
        variant="text"
        :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
        title="Paramètres Hermes"
        @click="emit('open-settings')"
      />
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Hermes : retour, titre, état du serveur,
 * actualisation et réglages.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import type { HermesServerState } from '../../utils/hermes'

const props = defineProps<{
  loading: boolean
  serverState: HermesServerState
}>()

const emit = defineEmits<{
  back: []
  refresh: []
  'open-settings': []
}>()

const appStore = useAppStore()

const stateColor = computed(() => {
  switch (props.serverState) {
    case 'online':
      return 'green'
    case 'offline':
      return 'red'
    case 'waking':
      return 'orange'
    default:
      return 'grey'
  }
})
</script>

<style scoped>
.hermes-toolbar-title {
  display: flex;
  align-items: center;
  font-size: 0.95rem;
  font-weight: 600;
}
</style>
