<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>Passages multiples non validés</v-card-title>
      <v-card-text>
        <p>
          Les portions répétées détectées n'ont pas été validées :
          <b>l'édition caméra restera inaccessible</b> tant qu'elles ne le
          seront pas.
          <template v-if="segmentCount > 0">
            <b>{{ segmentCount }}</b> segment(s) restent à valider.
          </template>
        </p>
        <p class="mt-2">
          La détection et ses ajustements sont enregistrés : ils seront
          restitués à la prochaine ouverture de la vue.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Rester</v-btn>
        <v-btn color="warning" @click="emit('confirm')">Quitter</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation avant de quitter la vue des passages multiples sans les avoir
 * validés.
 *
 * Le composant est purement présentationnel : la décision de l'ouvrir appartient
 * à `Multiride.vue` (`onBeforeRouteLeave` et le bouton Retour), qui ne l'ouvre
 * que si la barrière est encore levée. Le travail est **archivé** au fil des
 * ajustements : quitter ne perd rien.
 */
defineProps<{
  modelValue: boolean
  /** Nombre de segments détectés restant à valider. */
  segmentCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
