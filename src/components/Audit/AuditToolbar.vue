<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Retour à l'accueil"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <v-icon icon="mdi-map-marker-path" class="mr-2" />
      Audit GPX — {{ traceName }}
    </v-app-bar-title>

    <template #append>
      <AuditProgressChip
        :pending="pendingCount"
        :corrected="correctedCount"
        :fp="fpCount"
        class="mr-3"
      />

      <v-btn
        icon="mdi-cog-outline"
        variant="text"
        :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
        :title="
          appStore.isSettingsDrawerOpen
            ? 'Fermer les paramètres'
            : 'Paramètres de la vue Audit'
        "
        @click="emit('open-settings')"
      />

      <v-btn
        color="success"
        :disabled="!canApply"
        prepend-icon="mdi-check"
        :title="
          canApply
            ? 'Appliquer les corrections et fermer'
            : 'Toutes les anomalies doivent être traitées (ou marquées faux positif)'
        "
        @click="emit('apply')"
      >
        Appliquer
      </v-btn>
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Audit : retour, titre, badge de progression,
 * panneau Paramètres (flip-flop) et bouton « Appliquer ».
 *
 * Le bouton « Appliquer » reste désactivé tant que des anomalies sont à traiter
 * (décision 8 : gate strict `pending === 0`).
 */
import { useAppStore } from '../../stores/app'
import AuditProgressChip from './AuditProgressChip.vue'

defineProps<{
  traceName: string
  pendingCount: number
  correctedCount: number
  fpCount: number
  canApply: boolean
}>()

const emit = defineEmits<{
  back: []
  apply: []
  'open-settings': []
}>()

const appStore = useAppStore()
</script>
