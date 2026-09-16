<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title>
        {{
          pendingCount > 0
            ? 'Anomalies en cours de traitement'
            : 'Fichier GPX non créé'
        }}
      </v-card-title>
      <v-card-text>
        <p v-if="pendingCount > 0">
          <b>{{ pendingCount }}</b> anomalie(s) restent à traiter : le fichier
          GPX ne pourra pas être créé tant qu'elles n'auront pas toutes été
          corrigées ou marquées faux positif.
        </p>
        <p v-else>
          Les <b>{{ treatedCount }}</b> anomalie(s) détectées ont toutes été
          traitées, mais <b>le fichier GPX n'a pas été créé</b> : utilisez
          « Appliquer » pour l'écrire.
        </p>
        <p class="mt-2">
          L'audit en cours est archivé : il sera restitué à la prochaine
          ouverture de la vue Audit.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('cancel')">Rester</v-btn>
        <v-btn
          :color="pendingCount > 0 ? 'warning' : 'primary'"
          @click="emit('confirm')"
        >
          Quitter
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Confirmation avant de quitter la vue Audit avec un travail en cours.
 *
 * Le composant est purement présentationnel : la décision de l'ouvrir
 * appartient à `Audit.vue` (`onBeforeRouteLeave`), qui ne l'ouvre jamais en
 * consultation. Deux messages selon l'état du travail :
 * - des anomalies restent à traiter → le fichier GPX ne peut pas être créé ;
 * - toutes sont traitées mais le GPX n'a pas été créé (bouton « Appliquer »
 *   oublié).
 *
 * Dans les deux cas le travail est **archivé** au fil des traitements : quitter
 * ne perd rien.
 */
defineProps<{
  modelValue: boolean
  /** Anomalies restant à traiter. */
  pendingCount: number
  /** Anomalies traitées (corrigées ou marquées faux positif). */
  treatedCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  confirm: []
  cancel: []
}>()
</script>
