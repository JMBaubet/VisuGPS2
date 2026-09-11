<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Travail en cours</v-card-title>
      <v-card-text>
        <p>
          <b>{{ workCount }}</b> anomalie(s) ont été traitées.
        </p>
        <p class="mt-2">
          Si vous quittez maintenant, ces corrections seront perdues (l'audit
          n'est pas sauvegardé automatiquement).
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="error" @click="emit('confirm')">
          Quitter et perdre les corrections
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation avant de quitter la vue Audit lorsqu'un travail est en cours
 * (décision 9). Le composant est purement présentationnel : la décision de
 * l'ouvrir appartient à `Audit.vue` (`onBeforeRouteLeave`).
 */
defineProps<{
  modelValue: boolean
  workCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
