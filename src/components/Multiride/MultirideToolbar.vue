<template>
  <v-app-bar density="compact" color="surface" elevation="1">
    <v-btn
      icon="mdi-arrow-left"
      variant="text"
      title="Retour à l'accueil"
      @click="emit('back')"
    />

    <v-app-bar-title>
      <v-icon icon="mdi-repeat" class="mr-2" />
      Passages multiples — {{ traceName }}
    </v-app-bar-title>

    <template #append>
      <!-- État de la détection : c'est lui qui décide de l'accès à l'édition
           caméra (un état « à valider » la ferme). -->
      <v-chip
        v-if="status === 'pending'"
        color="warning"
        variant="tonal"
        prepend-icon="mdi-alert-outline"
        class="mr-3"
        title="Les portions répétées doivent être validées avant l'édition caméra"
      >
        À valider — {{ segmentCount }}
        {{ segmentCount > 1 ? 'segments' : 'segment' }}
      </v-chip>
      <v-chip
        v-else-if="status === 'validated'"
        color="info"
        variant="tonal"
        prepend-icon="mdi-check-decagram-outline"
        class="mr-3"
        :title="validatedTitle"
      >
        Validé
      </v-chip>
      <v-chip
        v-else-if="status === 'none'"
        color="success"
        variant="tonal"
        prepend-icon="mdi-check"
        class="mr-3"
        title="Aucune portion répétée : l'édition caméra est accessible"
      >
        Aucun passage multiple
      </v-chip>

      <v-btn
        prepend-icon="mdi-refresh"
        variant="text"
        :disabled="loading"
        title="Relancer la détection avec les paramètres courants"
        @click="emit('analyze')"
      >
        Relancer
      </v-btn>

      <v-btn
        icon="mdi-cog-outline"
        variant="text"
        :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
        :title="
          appStore.isSettingsDrawerOpen
            ? 'Fermer les paramètres'
            : 'Paramètres de la vue Passages multiples'
        "
        @click="emit('open-settings')"
      />
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Passages multiples : retour, titre, état de la
 * détection, relance de la détection et panneau Paramètres (flip-flop).
 *
 * L'état affiché est celui du **registre** des traces, restitué par le store :
 * `pending` ferme l'édition caméra tant que les portions répétées n'ont pas été
 * validées.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import type { MultirideStatus } from '../../stores/multiride'

const props = defineProps<{
  traceName: string
  /** Statut de la détection (`null` : aucune détection chargée). */
  status: MultirideStatus | null
  /** Nombre de segments détectés. */
  segmentCount: number
  /** Une détection ou une relecture est en cours. */
  loading: boolean
  /** Horodatage ISO de la validation, si connue. */
  validatedAt?: string | null
}>()

const emit = defineEmits<{
  back: []
  analyze: []
  'open-settings': []
}>()

const appStore = useAppStore()

/** Info-bulle du chip de validation : date de la validation. */
const validatedTitle = computed(() => {
  const date = props.validatedAt ? new Date(props.validatedAt) : null
  if (!date || Number.isNaN(date.getTime())) {
    return 'Portions répétées validées — édition caméra accessible'
  }
  return `Validé le ${date.toLocaleString('fr-FR')} — édition caméra accessible`
})
</script>
