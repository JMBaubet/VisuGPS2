<template>
  <v-dialog
    :model-value="modelValue"
    max-width="500"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Relancer la détection des tunnels ?</v-card-title>
      <v-card-text>
        <p v-if="dirty">
          Les modifications de tunnels non enregistrées seront perdues.
        </p>
        <p v-if="existingCount > 0">
          La détection écrasera les {{ existingCount }} tunnel{{
            existingCount > 1 ? 's' : ''
          }}
          actuel{{ existingCount > 1 ? 's' : '' }}, y compris les fusions et
          les tunnels créés manuellement.
        </p>
        <p class="mb-0">
          La détection sera rejouée avec les paramètres actuels.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="warning" @click="emit('confirm')">Relancer</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation d'une relance explicite de la détection des tunnels : elle
 * écrase le fichier courant, fusions et tunnels manuels compris.
 */
defineProps<{
  modelValue: boolean
  existingCount: number
  dirty: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
