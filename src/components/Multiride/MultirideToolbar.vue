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
      <!-- Avancement : segments examinés sur le total, comme le compteur
           d'anomalies de la vue Audit. -->
      <MultirideProgressChip
        :pending="pendingCount"
        :treated="treatedCount"
        :fp="fpCount"
        class="mr-3"
      />

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

      <!-- Sortie : la validation lève la barrière et ramène à l'accueil, où la
           carte du circuit ouvre l'édition caméra. -->
      <v-btn
        v-if="status === 'pending'"
        color="success"
        prepend-icon="mdi-check"
        :disabled="loading"
        title="Valider les passages multiples et revenir à l'accueil"
        @click="emit('validate')"
      >
        Valider
      </v-btn>

      <!-- Le titre est porté par l'enveloppe : un bouton désactivé ne reçoit
           pas le survol, son info-bulle ne s'afficherait donc jamais — or
           c'est elle qui dit pourquoi le panneau est fermé. -->
      <div :title="settingsNote">
        <v-btn
          icon="mdi-cog-outline"
          variant="text"
          :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
          :disabled="settingsLocked"
          @click="emit('open-settings')"
        />
      </div>
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
/**
 * Barre supérieure de la vue Passages multiples : retour, titre, avancement,
 * état de la détection, validation et panneau Paramètres (flip-flop).
 *
 * L'état affiché est celui du **registre** des traces, restitué par le store :
 * `pending` ferme l'édition caméra tant que les portions répétées n'ont pas été
 * validées. La validation lève la barrière et **ramène à l'accueil** ;
 * l'édition caméra s'ouvre depuis la carte du circuit, plus depuis cette vue.
 *
 * Il n'y a **pas de relance manuelle** : la détection est rejouée d'elle-même
 * quand un paramètre est enregistré, et le bouton des paramètres est grisé tant
 * qu'un ajustement est en place — une relance les écraserait — ou qu'une
 * détection est en cours.
 */
import { computed } from 'vue'
import { useAppStore } from '../../stores/app'
import MultirideProgressChip from './MultirideProgressChip.vue'
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
  /** Segments sans geste — à examiner. */
  pendingCount: number
  /** Segments examinés : approuvés, écartés ou fusionnés. */
  treatedCount: number
  /** Segments écartés (faux positifs). */
  fpCount: number
  /** Un ajustement est en place : les paramètres ne sont pas modifiables. */
  adjustmentsLocked: boolean
}>()

const emit = defineEmits<{
  back: []
  validate: []
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

/** Paramètres inaccessibles : une relance est en cours, ou un ajustement existe. */
const settingsLocked = computed(() => props.loading || props.adjustmentsLocked)

/** Motif du verrouillage — ou l'action du bouton, quand il est ouvert. */
const settingsNote = computed(() => {
  if (props.loading) {
    return 'Détection en cours : les paramètres ne sont pas modifiables'
  }
  if (props.adjustmentsLocked) {
    return 'Des ajustements sont en place : annulez-les pour modifier les paramètres'
  }
  return appStore.isSettingsDrawerOpen
    ? 'Fermer les paramètres'
    : 'Paramètres de la vue Passages multiples'
})
</script>
