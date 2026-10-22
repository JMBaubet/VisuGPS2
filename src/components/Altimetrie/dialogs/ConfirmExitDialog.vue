<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Modifications non enregistrées</v-card-title>
      <v-card-text>
        Une modification des montées ou des tunnels est en cours — création,
        déplacement, séparation, fusion ou suppression — et n'est pas
        enregistrée. Quitter sans sauvegarder ?
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="warning" @click="emit('confirm')">
          Quitter sans sauvegarder
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation avant de quitter la vue Altimétrie avec des modifications non
 * enregistrées. Composant purement présentationnel : la décision d'ouvrir le
 * dialogue appartient à `Altimetrie.vue` (`onBeforeRouteLeave`).
 */
defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
