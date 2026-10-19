<template>
  <v-dialog
    :model-value="modelValue"
    max-width="520"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Créer une montée</v-card-title>
      <v-card-text>
        <p>Deux clics sur le profil altimétrique suffisent :</p>
        <ol class="alt-create-steps">
          <li>
            Cliquez pour poser le <b>point de vallée</b>, en dehors d'une
            montée existante.
          </li>
          <li>
            Cliquez une seconde fois pour poser le <b>point de sommet</b>
            (50 m minimum après la vallée).
          </li>
          <li>
            La montée s'ajoute aux modifications ; utilisez « Sauvegarder »
            pour l'enregistrer.
          </li>
        </ol>
        <p class="text-medium-emphasis mt-3">
          La création peut être annulée à tout moment depuis le widget.
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
 * Aide de création manuelle d'une montée : deux clics sur le profil, vallée
 * puis sommet. Le dialogue est purement présentationnel ; la vue bascule
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
