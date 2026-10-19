<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Recalculer la détection ?</v-card-title>
      <v-card-text>
        Les modifications de montées non enregistrées seront perdues. La
        détection sera rejouée avec les paramètres actuels.
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="warning" @click="emit('confirm')">Recalculer</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation d'un recalcul explicite lorsque des montées ont été éditées :
 * la nouvelle détection repart de l'automatique et écrase ces éditions.
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
