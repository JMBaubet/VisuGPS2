<template>
  <v-dialog
    :model-value="modelValue"
    max-width="520"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Créer un tunnel</v-card-title>
      <v-card-text>
        <p>Deux clics sur le profil altimétrique suffisent :</p>
        <ol class="alt-create-steps">
          <li>
            Cliquez pour poser le <b>point d'entrée</b>, en dehors d'un tunnel
            existant.
          </li>
          <li>
            Cliquez une seconde fois pour poser le <b>point de sortie</b>
            (10 m minimum après l'entrée).
          </li>
          <li>
            Le tunnel s'ajoute aux modifications ; utilisez « Sauvegarder »
            pour l'enregistrer et le valider.
          </li>
        </ol>
        <p class="text-medium-emphasis mt-3">
          La création peut être annulée à tout moment depuis le widget, avec
          la touche Échap.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Annuler</v-btn>
        <v-btn color="primary" @click="emit('start')">Commencer</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Aide de création manuelle d'un tunnel : deux clics sur le profil, entrée
 * puis sortie. Le dialogue est purement présentationnel ; la vue bascule
 * ensuite le widget en mode création.
 */
defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  start: []
  cancel: []
}>()
</script>

<style scoped>
.alt-create-steps {
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
