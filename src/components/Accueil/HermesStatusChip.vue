<template>
  <v-tooltip location="bottom end" :open-delay="300" max-width="420">
    <template #activator="{ props }">
      <v-chip
        v-bind="props"
        :color="display.color"
        :prepend-icon="display.icon"
        variant="tonal"
        size="small"
        class="hermes-status-chip ma-1"
        @click="openView"
      >
        <span class="hermes-status-chip__label">{{ display.label }}</span>
      </v-chip>
    </template>
    <div class="hermes-status-tooltip">
      <div v-for="(line, index) in display.tooltip" :key="index">
        {{ line }}
      </div>
    </div>
  </v-tooltip>
</template>

<script setup lang="ts">
/**
 * Chip d'état du serveur `hermes` dans la toolbar de l'Accueil.
 *
 * Il affiche l'état de connexion et l'identifiant du dataset actif. Un clic
 * ouvre la vue `/hermes`, où l'ensemble des données est géré. Le rafraîchissement
 * a lieu au montage et au retour de focus de la fenêtre — jamais par un timer
 * permanent, qui empêcherait la veille du serveur.
 */
import { computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useHermesStore } from '../../stores/hermes'

const router = useRouter()
const hermesStore = useHermesStore()

const display = computed(() => hermesStore.statusDisplay)

function openView(): void {
  void router.push({ name: 'hermes' })
}

function refresh(): void {
  void hermesStore.refreshServer()
}

onMounted(() => {
  refresh()
  window.addEventListener('focus', refresh)
})

onUnmounted(() => {
  window.removeEventListener('focus', refresh)
})
</script>

<style scoped>
.hermes-status-chip {
  max-width: 280px;
}

.hermes-status-chip__label {
  display: inline-block;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}

.hermes-status-tooltip {
  font-size: 0.8rem;
  line-height: 1.4;
}
</style>
