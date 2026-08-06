<template>
  <div v-if="editionStore.showViewportFrame" ref="overlay" class="viewport-overlay">
    <!--
      Rectangle 16:9 centré. Sa box-shadow gigantesque (spread 100vmax) crée
      le masque sombre autour de la zone de rendu, sans avoir à gérer quatre
      bandeaux. pointer-events: none pour ne pas bloquer la carte en dessous.
    -->
    <div class="viewport-rect" :style="rectStyle">
      <span class="viewport-label">ViewPort · 16:9 · 1920×1080</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Cadre ViewPort 16:9 (overlay CSS pur).
 *
 * Représente la zone de rendu finale de l'export vidéo (spec §4.2) :
 *   - rectangle au ratio 16/9, le plus grand possible, centré
 *   - calque semi-transparent noir (~70 %) à l'extérieur du rectangle
 *   - trait blanc fin (2 px) sur les bords
 *
 * Purement informatif : ce composant ne modifie pas le rendu MapBox. Il
 * s'affiche par-dessus la carte (z-index supérieur) et laisse passer les
 * événements de pointeur (pointer-events: none) afin de ne pas bloquer la
 * manipulation de la carte.
 *
 * Le rectangle est calculé en JS à partir des dimensions réelles du
 * conteneur (ResizeObserver) : on détermine le plus grand rectangle de
 * ratio 16/9 tenant dans la zone disponible, ce qui gère correctement les
 * ratios d'écran très larges ou très étroits.
 */
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { useEditionStore } from '../../stores/edition'

const editionStore = useEditionStore()

const overlay = ref<HTMLDivElement | null>(null)
const containerWidth = ref(0)
const containerHeight = ref(0)

let resizeObserver: ResizeObserver | null = null

/**
 * Dimensions du rectangle 16:9 le plus grand tenant dans le conteneur.
 * Si le conteneur est plus « large » que 16:9, on est limité par la
 * hauteur ; sinon par la largeur.
 */
const rectStyle = computed(() => {
  const w = containerWidth.value
  const h = containerHeight.value
  if (!w || !h) return { width: '0px', height: '0px' }

  const targetRatio = 16 / 9
  const containerRatio = w / h

  let rectW: number
  let rectH: number
  if (containerRatio > targetRatio) {
    // Limité par la hauteur.
    rectH = h
    rectW = rectH * targetRatio
  } else {
    // Limité par la largeur.
    rectW = w
    rectH = rectW / targetRatio
  }

  return {
    width: `${rectW}px`,
    height: `${rectH}px`,
  }
})

function measure() {
  if (overlay.value) {
    containerWidth.value = overlay.value.clientWidth
    containerHeight.value = overlay.value.clientHeight
  }
}

// Mesurer dès l'apparition (v-if) du composant.
watch(() => editionStore.showViewportFrame, async (visible) => {
  if (visible) {
    await nextTick()
    measure()
  }
})

onMounted(() => {
  measure()
  if (overlay.value) {
    resizeObserver = new ResizeObserver(() => measure())
    resizeObserver.observe(overlay.value)
  }
})

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
})
</script>

<style scoped>
/* Calque plein écran par-dessus la carte, laisse passer les clics. */
.viewport-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  pointer-events: none;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

/*
 * Rectangle 16:9 centré.
 * La box-shadow colossale (spread 100vmax, noir à 70 % d'opacité) crée le
 * masque autour du rectangle : elle déborde sur tout l'écran tout en
 * laissant l'intérieur du rectangle parfaitement transparent.
 */
.viewport-rect {
  position: relative;
  border: 2px solid #FFFFFF;
  background: transparent;
  box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.7);
}

/* Étiquette discrète en haut à gauche du rectangle. */
.viewport-label {
  position: absolute;
  top: 6px;
  left: 8px;
  font-size: 11px;
  line-height: 1;
  color: #FFFFFF;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
  font-family: monospace;
  opacity: 0.85;
}
</style>
