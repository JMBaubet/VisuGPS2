<script setup lang="ts">
const props = defineProps<{
  modelValue: boolean
  filename: string
  regionName?: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'confirm'): void
}>()
</script>

<template>
  <v-dialog
    :model-value="props.modelValue"
    max-width="460"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title class="text-h6">Supprimer le fichier ?</v-card-title>
      <v-card-text>
        <p class="mb-2">
          Le fichier <strong>{{ props.regionName ?? props.filename }}</strong>
          sera supprimé du support OSM.
        </p>
        <p class="text-caption text-medium-emphasis mb-0">
          Les analyses de tunnels existantes sont conservées. Les prochaines
          détections qui nécessitent cette région échoueront tant que le fichier
          n'aura pas été téléchargé à nouveau.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('update:modelValue', false)">
          Annuler
        </v-btn>
        <v-btn color="red-darken-3" variant="flat" @click="emit('confirm')">
          Supprimer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>
