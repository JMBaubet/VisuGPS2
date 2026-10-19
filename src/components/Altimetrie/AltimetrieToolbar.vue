<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Fermer la vue Altimétrie"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <v-icon icon="mdi-terrain" class="mr-2" />
      Altimétrie — {{ traceName }}
    </v-app-bar-title>

    <template #append>
      <v-btn
        color="success"
        variant="tonal"
        prepend-icon="mdi-content-save"
        :disabled="!dirty || saving"
        :loading="saving"
        :title="
          dirty
            ? 'Enregistrer les modifications de montées'
            : 'Aucune modification à enregistrer'
        "
        @click="emit('save')"
      >
        Sauvegarder
      </v-btn>

      <v-btn
        icon="mdi-cog-outline"
        variant="text"
        :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
        :title="
          appStore.isSettingsDrawerOpen
            ? 'Fermer les paramètres'
            : 'Paramètres de la vue Altimétrie'
        "
        @click="emit('open-settings')"
      />
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Altimétrie : retour, titre, sauvegarde et panneau
 * Paramètres. La synthèse de cohérence des réglages et le bouton « Recalculer »
 * seront ajoutés à l'étape d'édition.
 */
import { useAppStore } from '../../stores/app'

defineProps<{
  traceName: string
  /** Des modifications de montées restent à enregistrer. */
  dirty: boolean
  /** Une sauvegarde est en cours. */
  saving: boolean
}>()

const emit = defineEmits<{
  back: []
  save: []
  'open-settings': []
}>()

const appStore = useAppStore()
</script>
