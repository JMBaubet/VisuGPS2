<template>
  <div v-if="editionStore.hasKeyframes" class="distance-hud">
    <span class="distance-current">{{ currentKm }}</span>
    <span class="distance-sep">/</span>
    <span class="distance-total">{{ totalKm }}</span>
    <span class="distance-unit">km</span>
  </div>
</template>

<script setup lang="ts">
/**
 * HUD Distance — affiche la distance parcourue / distance totale.
 *
 * Overlay en haut au centre, fond semi-transparent sombre (~80 %). La distance
 * parcourue est en orange (#FF9800), la distance totale en blanc atténué.
 *
 * Masqué tant qu'aucun jeu de keyframes n'est chargé.
 */
import { computed } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

const currentKm = computed(() => editionStore.currentDistanceKm.toFixed(2))
const totalKm = computed(() => editionStore.totalDistanceKm.toFixed(2))
</script>

<style scoped>
.distance-hud {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 6;
  padding: 6px 14px;
  background: rgba(0, 0, 0, 0.8);
  color: #ffffff;
  border-radius: 6px;
  font-size: 14px;
  line-height: 1.4;
  font-family: monospace;
  pointer-events: none;
  user-select: none;
  white-space: nowrap;
}

.distance-current {
  color: #ff9800;
  font-weight: 700;
}

.distance-sep {
  opacity: 0.5;
  margin: 0 4px;
}

.distance-total {
  opacity: 0.7;
  font-weight: 600;
}

.distance-unit {
  opacity: 0.55;
  margin-left: 4px;
}
</style>
