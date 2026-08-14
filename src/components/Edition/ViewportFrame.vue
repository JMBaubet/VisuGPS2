<template>
  <div v-if="editionStore.showViewportFrame" ref="overlay" class="viewport-overlay">
    <!--
      Rectangle aux dimensions du ratio sélectionné (16:9 / 4:3), centré.
      Sa box-shadow gigantesque (spread 100vmax) crée le masque sombre autour
      de la zone de rendu, sans avoir à gérer quatre bandeaux.
      pointer-events: none pour ne pas bloquer la carte en dessous.
    -->
    <div
      class="viewport-rect"
      :class="{ 'mode-validation': editionStore.validationMode }"
      :style="rectStyle"
    >
      <span class="viewport-label">{{ viewportLabel }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Cadre ViewPort (overlay CSS pur) — ratio 16:9 ou 4:3 selon la sélection.
 *
 * Représente la zone de rendu finale de l'export vidéo (spec §4.2) :
 *   - rectangle au ratio sélectionné (`editionStore.viewportAspect`), le plus
 *     grand possible, centré
 *   - calque semi-transparent noir (~70 %) à l'extérieur du rectangle
 *   - trait blanc fin (2 px) sur les bords
 *
 * Purement informatif : ce composant ne modifie pas le rendu MapBox. Il
 * s'affiche par-dessus la carte (z-index supérieur) et laisse passer les
 * événements de pointeur (pointer-events: none) afin de ne pas bloquer la
 * manipulation de la carte.
 *
 * Le rectangle est calculé en JS à partir des dimensions réelles du
 * conteneur (ResizeObserver) : on détermine le plus grand rectangle du ratio
 * sélectionné tenant dans la zone disponible, ce qui gère correctement les
 * ratios d'écran très larges ou très étroits.
 */
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { useEditionStore } from '../../stores/edition'
import { VIEWPORTS_BY_ASPECT } from '../../algorithms/keyframeGenerator'

const editionStore = useEditionStore()

const overlay = ref<HTMLDivElement | null>(null)
const containerWidth = ref(0)
const containerHeight = ref(0)

let resizeObserver: ResizeObserver | null = null

/** Ratio d'écran sélectionné (16:9 ou 4:3) — pilote le cadre affiché. */
const targetRatio = computed(() => {
  const vp = VIEWPORTS_BY_ASPECT[editionStore.viewportAspect]
  return vp.width / vp.height
})

/** Libellé du cadre (ratio + dimensions de référence). */
const viewportLabel = computed(() => {
  const vp = VIEWPORTS_BY_ASPECT[editionStore.viewportAspect]
  return `ViewPort · ${editionStore.viewportAspect} · ${vp.width}×${vp.height}`
})

/**
 * Dimensions du rectangle (ratio sélectionné) le plus grand tenant dans le
 * conteneur. Si le conteneur est plus « large » que le ratio, on est limité par
 * la hauteur ; sinon par la largeur.
 */
const rectStyle = computed(() => {
  const w = containerWidth.value
  const h = containerHeight.value
  if (!w || !h) return { width: '0px', height: '0px' }

  const ratio = targetRatio.value
  const containerRatio = w / h

  let rectW: number
  let rectH: number
  if (containerRatio > ratio) {
    // Limité par la hauteur.
    rectH = h
    rectW = rectH * ratio
  } else {
    // Limité par la largeur.
    rectW = w
    rectH = rectW / ratio
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

/* Mode validation : cadre bleu (et libellé assorti). */
.viewport-rect.mode-validation {
  border-color: #2196F3;
}
.viewport-rect.mode-validation .viewport-label {
  color: #2196F3;
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
