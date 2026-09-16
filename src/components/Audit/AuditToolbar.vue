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

      <!-- Consultation : l'audit est déjà appliqué, plus rien ne s'exporte ni
           ne se corrige — le bouton « Appliquer » laisse place à un chip. -->
      <v-chip
        v-if="consultation"
        color="info"
        variant="tonal"
        prepend-icon="mdi-eye-outline"
        :title="consultationTitle"
      >
        Consultation
      </v-chip>

      <v-btn
        v-else
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
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Audit : retour, titre, badge de progression,
 * panneau Paramètres (flip-flop) et bouton « Appliquer ».
 *
 * Le bouton « Appliquer » reste désactivé tant que des anomalies sont à traiter
 * (décision 8 : gate strict `pending === 0`). En **consultation** (audit déjà
 * appliqué, trace `clean`), il cède la place à un chip : rien ne s'applique.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import AuditProgressChip from './AuditProgressChip.vue'

const props = defineProps<{
  traceName: string
  pendingCount: number
  correctedCount: number
  fpCount: number
  canApply: boolean
  /** Audit validé affiché en lecture seule. */
  consultation: boolean
  /** Horodatage ISO de la dernière écriture de l'archive, si connue. */
  archivedAt?: string | null
}>()

const emit = defineEmits<{
  back: []
  apply: []
  'open-settings': []
}>()

const appStore = useAppStore()

/** Info-bulle du chip : date d'archivage de l'audit consulté. */
const consultationTitle = computed(() => {
  const date = props.archivedAt ? new Date(props.archivedAt) : null
  if (!date || Number.isNaN(date.getTime())) {
    return 'Audit appliqué — corrections en lecture seule'
  }
  return `Audit appliqué le ${date.toLocaleString('fr-FR')} — corrections en lecture seule`
})
</script>
