<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Fermer la vue Altimétrie"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <div class="alt-toolbar-title">
        <div class="alt-toolbar-title__main">
          <v-icon icon="mdi-terrain" size="18" class="mr-2" />
          Altimétrie — {{ traceName }}
        </div>
        <div
          class="alt-toolbar-title__causes"
          :class="`text-${settingsStatus}`"
          :title="settingsTitle"
        >
          {{ causesText }}
        </div>
      </div>
    </v-app-bar-title>

    <template #append>
      <v-btn
        v-if="paramsOutdated"
        color="warning"
        variant="tonal"
        prepend-icon="mdi-refresh"
        class="mr-2"
        title="Rejouer la détection avec les paramètres actuels"
        @click="emit('recalculate')"
      >
        Recalculer
      </v-btn>

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
        :variant="appStore.isSettingsDrawerOpen ? 'tonal' : 'text'"
        :color="settingsStatus"
        :title="settingsTitle"
        @click="emit('open-settings')"
      />
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Altimétrie : retour, titre, synthèse des réglages,
 * sauvegarde, recalcul explicite et panneau Paramètres.
 *
 * L'icône Paramètres prend la couleur du diagnostic des réglages (vert, orange
 * ou rouge) ; les causes sont résumées en tiny sous le titre et détaillées dans
 * l'infobulle.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import type { AltimetrieStatus } from '../../utils/altimetrieDiagnostics'

const props = defineProps<{
  traceName: string
  /** Des modifications de montées restent à enregistrer. */
  dirty: boolean
  /** Une sauvegarde est en cours. */
  saving: boolean
  /** Statut global des paramètres courants. */
  settingsStatus: AltimetrieStatus
  /** Causes du statut dégradé, ou liste vide si cohérent. */
  settingsCauses: string[]
  /** Le fichier a été généré avec d'autres paramètres. */
  paramsOutdated: boolean
}>()

const emit = defineEmits<{
  back: []
  save: []
  recalculate: []
  'open-settings': []
}>()

const appStore = useAppStore()

const causesText = computed(() => {
  if (props.settingsCauses.length === 0) return 'Paramètres cohérents'
  const shown = props.settingsCauses.slice(0, 2).join(' · ')
  const remaining = props.settingsCauses.length - 2
  return remaining > 0 ? `${shown} (+${remaining})` : shown
})

const settingsTitle = computed(() => {
  const causes =
    props.settingsCauses.length > 0
      ? props.settingsCauses.join(' · ')
      : 'Paramètres cohérents'
  return props.paramsOutdated
    ? `${causes}. Paramètres modifiés depuis la génération : recalcul requis.`
    : causes
})
</script>

<style scoped>
.alt-toolbar-title {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
  line-height: 1.15;
}

.alt-toolbar-title__main {
  font-size: 0.9rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.alt-toolbar-title__causes {
  font-size: 0.65rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 560px;
}
</style>
