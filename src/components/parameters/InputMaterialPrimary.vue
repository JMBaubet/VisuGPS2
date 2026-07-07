<script setup lang="ts">
// Material primaire : affichage en grille de couleurs Material Design 500
// avec ajout du blanc et du noir. Sélection restreinte à ces couleurs.
import { computed } from 'vue'
import type { SettingDefinition } from '../../stores/settings'
import {
  MATERIAL_PRIMARY_SWATCHES,
  isValidHexAlpha,
} from '../../utils/materialColors'

const props = defineProps<{
  def: SettingDefinition
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

// Ajout du blanc et du noir aux swatches
const EXTRA_SWATCHES = [
  { hex: '#FFFFFFFF', name: 'Blanc' },
  { hex: '#000000FF', name: 'Noir' },
]

// Map des couleurs avec leurs noms (sans canal alpha)
const COLOR_NAMES: Record<string, string> = {
  '#F44336': 'Rouge',
  '#E91E63': 'Rose',
  '#9C27B0': 'Violet',
  '#673AB7': 'Violet profond',
  '#3F51B5': 'Indigo',
  '#2196F3': 'Bleu',
  '#03A9F4': 'Bleu clair',
  '#00BCD4': 'Cyan',
  '#009688': 'Teal',
  '#4CAF50': 'Vert',
  '#8BC34A': 'Vert clair',
  '#CDDC39': 'Lime',
  '#FFEB3B': 'Jaune',
  '#FFC107': 'Ambre',
  '#FF9800': 'Orange',
  '#FF5722': 'Orange profond',
  '#795548': 'Marron',
  '#9E9E9E': 'Gris',
  '#607D8B': 'Bleu gris',
  '#FFFFFF': 'Blanc',
  '#000000': 'Noir',
}

// Fonction pour normaliser un code hex (supprimer le canal alpha)
function normalizeHex(hex: string): string {
  const upper = hex.toUpperCase()
  // Si le code a un canal alpha (8 caractères), on le retire
  if (upper.startsWith('#') && upper.length === 9) {
    return upper.substring(0, 7)
  }
  return upper
}

const allSwatches = computed(() => [
  ...MATERIAL_PRIMARY_SWATCHES,
  ...EXTRA_SWATCHES,
])

// Organisation en grille de 6 colonnes
const swatchesGrid = computed(() => {
  const grid: { hex: string; name?: string }[][] = []
  const rowSize = 6
  
  for (let i = 0; i < allSwatches.value.length; i += rowSize) {
    grid.push(allSwatches.value.slice(i, i + rowSize))
  }
  
  return grid
})

const safeValue = computed(() =>
  isValidHexAlpha(props.modelValue)
    ? props.modelValue
    : MATERIAL_PRIMARY_SWATCHES[0].hex
)

const defaultColor = computed(() => {
  const def = props.def.default
  return isValidHexAlpha(def) ? def : MATERIAL_PRIMARY_SWATCHES[0].hex
})

const currentColorName = computed(() => {
  const normalized = normalizeHex(safeValue.value)
  return COLOR_NAMES[normalized] || safeValue.value
})

const defaultColorName = computed(() => {
  const normalized = normalizeHex(defaultColor.value)
  return COLOR_NAMES[normalized] || defaultColor.value
})



function selectColor(hex: string) {
  emit('update:modelValue', hex)
}

function isSelected(hex: string) {
  return safeValue.value.toUpperCase() === hex.toUpperCase()
}

function isDefault(hex: string) {
  return defaultColor.value.toUpperCase() === hex.toUpperCase()
}

function getColorDisplayName(hex: string) {
  const normalized = normalizeHex(hex)
  return COLOR_NAMES[normalized] || hex
}
</script>

<template>
  <div>
    <!-- Grille de couleurs -->
    <div class="color-grid">
      <div
        v-for="row in swatchesGrid"
        :key="row.map(c => c.hex).join('-')"
        class="color-row"
      >
        <div
          v-for="swatch in row"
          :key="swatch.hex"
          class="color-cell"
          :class="{ 
            selected: isSelected(swatch.hex),
            isDefault: isDefault(swatch.hex)
          }"
          @click="selectColor(swatch.hex)"
          :title="getColorDisplayName(swatch.hex)"
        >
          <div
            class="color-swatch"
            :style="{ backgroundColor: swatch.hex }"
          >
            <!-- Indicateur de sélection -->
            <div v-if="isSelected(swatch.hex)" class="selection-indicator">
              <v-icon
                v-if="swatch.hex.toLowerCase() === '#ffffffff'"
                icon="mdi-check"
                size="18"
                color="black"
              />
              <v-icon
                v-else
                icon="mdi-check"
                size="18"
                color="white"
              />
            </div>
            
            <!-- Indicateur de valeur par défaut -->
            <div v-if="isDefault(swatch.hex) && !isSelected(swatch.hex)" class="default-indicator">
              <v-icon
                v-if="swatch.hex.toLowerCase() === '#ffffffff'"
                icon="mdi-circle-outline"
                size="14"
                color="black"
              />
              <v-icon
                v-else
                icon="mdi-circle-outline"
                size="14"
                color="white"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Aperçu et informations -->
    <div class="d-flex align-center mt-3 info-container">
      <div class="d-flex align-center">
        <div class="color-preview" :style="{ backgroundColor: safeValue }"></div>
        <div class="ml-3">
          <div class="text-body-2 font-weight-medium">{{ currentColorName }}</div>
        </div>
      </div>
      
      <v-spacer></v-spacer>
      
      <div class="d-flex align-center default-info">
        <span class="text-caption text-medium-emphasis mr-2">Défaut :</span>
        <div class="default-preview" :style="{ backgroundColor: defaultColor }"></div>
        <span class="text-caption text-medium-emphasis ml-2">{{ defaultColorName }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.color-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.color-row {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 6px;
}

.color-cell {
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  transition: all 0.15s ease;
  position: relative;
}

.color-cell:hover {
  transform: scale(1.02);
  z-index: 1;
}

/* Suppression complète de l'encadrement de sélection */
.color-cell.selected {
  box-shadow: none;
}

.color-cell.isDefault:not(.selected) {
  box-shadow: 0 0 0 2px rgba(var(--v-theme-on-surface), 0.0);
  border-radius: 4px;
}

.color-swatch {
  width: 100%;
  aspect-ratio: 1;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  transition: all 0.15s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  min-height: 32px;
  max-height: 48px;
}

.color-cell:hover .color-swatch {
  border-color: rgba(var(--v-theme-on-surface), 0.2);
}

.selection-indicator {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0);
}

/* Suppression de la bordure de sélection */
.color-cell.selected .color-swatch {
  border-color: rgba(var(--v-theme-on-surface), 0.08);
}

.default-indicator {
  position: absolute;
  bottom: 2px;
  right: 2px;
  opacity: 0.7;
}

.color-preview {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.15);
  flex-shrink: 0;
}

.default-preview {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.15);
  flex-shrink: 0;
}

.info-container {
  flex-wrap: wrap;
  gap: 8px;
}

.default-info {
  background: rgba(var(--v-theme-on-surface), 0.04);
  padding: 4px 12px 4px 8px;
  border-radius: 4px;
}

/* Responsive */
@media (max-width: 600px) {
  .color-row {
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }
  
  .color-swatch {
    min-height: 28px;
    max-height: 36px;
  }
  
  .info-container {
    flex-direction: column;
    align-items: stretch !important;
  }
  
  .default-info {
    justify-content: center;
  }
}
</style>