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
          <v-icon
            :icon="mode === 'tunnels' ? 'mdi-tunnel' : 'mdi-terrain'"
            size="18"
            class="mr-2"
          />
          {{ mode === 'tunnels' ? 'Tunnels' : 'Altimétrie' }} — {{ traceName }}
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

      <!--
        Tant que l'opérateur n'a pas sauvegardé l'analyse, le bouton vaut
        « Valider » : il persiste l'analyse telle quelle et vaut validation.
        Dès qu'une montée est modifiée, il redevient « Sauvegarder » — la
        validation ne disparaît qu'une fois une sauvegarde effectuée.
      -->
      <v-btn
        v-if="showValidate"
        color="success"
        variant="tonal"
        prepend-icon="mdi-check-circle"
        :disabled="!ready || saving"
        :loading="saving"
        :title="validateTitle"
        @click="emit('save')"
      >
        Valider
      </v-btn>

      <v-btn
        v-else
        color="success"
        variant="tonal"
        prepend-icon="mdi-content-save"
        :disabled="!ready || !dirty || saving"
        :loading="saving"
        :title="saveTitle"
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
  /** Onglet actif : adapte l'icône, le titre et le libellé des actions. */
  mode: 'climbs' | 'tunnels'
  traceName: string
  /** Des modifications de montées restent à enregistrer. */
  dirty: boolean
  /** Une sauvegarde est en cours. */
  saving: boolean
  /** L'analyse a déjà été sauvegardée / validée par l'opérateur. */
  validated: boolean
  /** L'analyse est chargée : les actions de persistance sont disponibles. */
  ready: boolean
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

/**
 * « Valider » n'existe que tant qu'aucune sauvegarde n'a eu lieu **et**
 * qu'aucune montée n'a été modifiée. Une modification bascule le bouton en
 * « Sauvegarder » : c'est la sauvegarde qui vaut validation.
 */
const showValidate = computed(() => !props.validated && !props.dirty)

const validateTitle = computed(() =>
  props.mode === 'tunnels'
    ? 'Valider les tunnels (aucune sauvegarde effectuée)'
    : "Valider l'altimétrie (aucune sauvegarde effectuée)",
)

const saveTitle = computed(() =>
  props.dirty
    ? props.mode === 'tunnels'
      ? 'Enregistrer les modifications de tunnels'
      : 'Enregistrer les modifications de montées'
    : 'Aucune modification à enregistrer',
)

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
