<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Appliquer les corrections ?</v-card-title>
      <v-card-text>
        <p>
          Le GPX va être <b>réécrit</b> avec les corrections appliquées.
          L'original sera sauvegardé en <code>.gpx.orig</code> (une seule fois,
          jamais écrasé).
        </p>
        <p class="mt-2">
          Cette action est <b>irréversible</b>. La trace sera ensuite marquée
          comme auditée et prête pour l'édition caméra.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="success" @click="emit('confirm')">
          Appliquer et fermer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation avant réécriture du GPX — point de non-retour de l'audit
 * (décision 8 : la validation est ferme et non annulable).
 */
defineProps<{ modelValue: boolean }>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
