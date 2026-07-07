<script setup lang="ts">
// Material Design complet avec toutes les déclinaisons Vuetify 3
// Pour chaque couleur : lighten-5 à 1, base, darken-1 à 4, accent-1 à 4
// Organisé par teinte avec affichage du nom de la couleur sélectionnée
import { computed, ref } from 'vue'
import type { SettingDefinition } from '../../stores/settings'
import {
  isValidHexAlpha,
  nearestMaterialColor,
} from '../../utils/materialColors'

const props = defineProps<{
  def: SettingDefinition
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

// État pour masquer/afficher les noms des teintes
const showHueLabels = ref(false)

// Définition des couleurs Material Design avec leurs déclinaisons
// Format: [lighten-5, lighten-4, lighten-3, lighten-2, lighten-1, base, darken-1, darken-2, darken-3, darken-4, accent-1, accent-2, accent-3, accent-4]
const MATERIAL_VARIATIONS: Record<string, { name: string; variations: string[] }> = {
  'red': {
    name: 'Rouge',
    variations: [
      '#FFEBEE', '#FFCDD2', '#EF9A9A', '#E57373', '#EF5350', '#F44336', 
      '#E53935', '#D32F2F', '#C62828', '#B71C1C',
      '#FF8A80', '#FF5252', '#FF1744', '#D50000'
    ]
  },
  'pink': {
    name: 'Rose',
    variations: [
      '#FCE4EC', '#F8BBD0', '#F48FB1', '#F06292', '#EC407A', '#E91E63',
      '#D81B60', '#C2185B', '#AD1457', '#880E4F',
      '#FF80AB', '#FF4081', '#F50057', '#C51162'
    ]
  },
  'purple': {
    name: 'Violet',
    variations: [
      '#F3E5F5', '#E1BEE7', '#CE93D8', '#BA68C8', '#AB47BC', '#9C27B0',
      '#8E24AA', '#7B1FA2', '#6A1B9A', '#4A148C',
      '#EA80FC', '#E040FB', '#D500F9', '#AA00FF'
    ]
  },
  'deep-purple': {
    name: 'Violet profond',
    variations: [
      '#EDE7F6', '#D1C4E9', '#B39DDB', '#9575CD', '#7E57C2', '#673AB7',
      '#5E35B1', '#512DA8', '#4527A0', '#311B92',
      '#B388FF', '#7C4DFF', '#651FFF', '#6200EA'
    ]
  },
  'indigo': {
    name: 'Indigo',
    variations: [
      '#E8EAF6', '#C5CAE9', '#9FA8DA', '#7986CB', '#5C6BC0', '#3F51B5',
      '#3949AB', '#303F9F', '#283593', '#1A237E',
      '#8C9EFF', '#536DFE', '#3D5AFE', '#304FFE'
    ]
  },
  'blue': {
    name: 'Bleu',
    variations: [
      '#E3F2FD', '#BBDEFB', '#90CAF9', '#64B5F6', '#42A5F5', '#2196F3',
      '#1E88E5', '#1976D2', '#1565C0', '#0D47A1',
      '#82B1FF', '#448AFF', '#2979FF', '#2962FF'
    ]
  },
  'light-blue': {
    name: 'Bleu clair',
    variations: [
      '#E1F5FE', '#B3E5FC', '#81D4FA', '#4FC3F7', '#29B6F6', '#03A9F4',
      '#039BE5', '#0288D1', '#0277BD', '#01579B',
      '#80D8FF', '#40C4FF', '#00B0FF', '#0091EA'
    ]
  },
  'cyan': {
    name: 'Cyan',
    variations: [
      '#E0F7FA', '#B2EBF2', '#80DEEA', '#4DD0E1', '#26C6DA', '#00BCD4',
      '#00ACC1', '#0097A7', '#00838F', '#006064',
      '#84FFFF', '#18FFFF', '#00E5FF', '#00B8D4'
    ]
  },
  'teal': {
    name: 'Teal',
    variations: [
      '#E0F2F1', '#B2DFDB', '#80CBC4', '#4DB6AC', '#26A69A', '#009688',
      '#00897B', '#00796B', '#00695C', '#004D40',
      '#A7FFEB', '#64FFDA', '#1DE9B6', '#00BFA5'
    ]
  },
  'green': {
    name: 'Vert',
    variations: [
      '#E8F5E9', '#C8E6C9', '#A5D6A7', '#81C784', '#66BB6A', '#4CAF50',
      '#43A047', '#388E3C', '#2E7D32', '#1B5E20',
      '#B9F6CA', '#69F0AE', '#00E676', '#00C853'
    ]
  },
  'light-green': {
    name: 'Vert clair',
    variations: [
      '#F1F8E9', '#DCEDC8', '#C5E1A5', '#AED581', '#9CCC65', '#8BC34A',
      '#7CB342', '#689F38', '#558B2F', '#33691E',
      '#CCFF90', '#B2FF59', '#76FF03', '#64DD17'
    ]
  },
  'lime': {
    name: 'Lime',
    variations: [
      '#F9FBE7', '#F0F4C3', '#E6EE9C', '#DCE775', '#D4E157', '#CDDC39',
      '#C0CA33', '#AFB42B', '#9E9D24', '#827717',
      '#F4FF81', '#EEFF41', '#C6FF00', '#AEEA00'
    ]
  },
  'yellow': {
    name: 'Jaune',
    variations: [
      '#FFFDE7', '#FFF9C4', '#FFF59D', '#FFF176', '#FFEE58', '#FFEB3B',
      '#FDD835', '#FBC02D', '#F9A825', '#F57F17',
      '#FFFF8D', '#FFFF00', '#FFEA00', '#FFD600'
    ]
  },
  'amber': {
    name: 'Ambre',
    variations: [
      '#FFF8E1', '#FFECB3', '#FFE082', '#FFD54F', '#FFCA28', '#FFC107',
      '#FFB300', '#FFA000', '#FF8F00', '#FF6F00',
      '#FFE57F', '#FFD740', '#FFC400', '#FFAB00'
    ]
  },
  'orange': {
    name: 'Orange',
    variations: [
      '#FFF3E0', '#FFE0B2', '#FFCC80', '#FFB74D', '#FFA726', '#FF9800',
      '#FB8C00', '#F57C00', '#EF6C00', '#E65100',
      '#FFD180', '#FFAB40', '#FF9100', '#FF6D00'
    ]
  },
  'deep-orange': {
    name: 'Orange profond',
    variations: [
      '#FBE9E7', '#FFCCBC', '#FFAB91', '#FF8A65', '#FF7043', '#FF5722',
      '#F4511E', '#E64A19', '#D84315', '#BF360C',
      '#FF9E80', '#FF6E40', '#FF3D00', '#DD2C00'
    ]
  },
  'brown': {
    name: 'Marron',
    variations: [
      '#EFEBE9', '#D7CCC8', '#BCAAA4', '#A1887F', '#8D6E63', '#795548',
      '#6D4C41', '#5D4037', '#4E342E', '#3E2723'
    ]
  },
  'grey': {
    name: 'Gris',
    variations: [
      '#FAFAFA', '#F5F5F5', '#EEEEEE', '#E0E0E0', '#BDBDBD', '#9E9E9E',
      '#757575', '#616161', '#424242', '#212121'
    ]
  },
  'blue-grey': {
    name: 'Bleu gris',
    variations: [
      '#ECEFF1', '#CFD8DC', '#B0BEC5', '#90A4AE', '#78909C', '#607D8B',
      '#546E7A', '#455A64', '#37474F', '#263238'
    ]
  }
}

// Extra: Blanc et Noir
const EXTRA_SWATCHES = [
  { hex: '#FFFFFF', name: 'Blanc' },
  { hex: '#000000', name: 'Noir' }
]

// Map des noms de couleurs (pour l'affichage)
const COLOR_NAMES: Record<string, string> = {
  '#FFFFFF': 'Blanc',
  '#000000': 'Noir'
}

// Ajouter les noms des couleurs de base
Object.entries(MATERIAL_VARIATIONS).forEach(([key, value]) => {
  COLOR_NAMES[value.variations[5]] = value.name
})

// Récupérer toutes les couleurs avec leurs métadonnées
const allSwatches = computed(() => {
  const result: Array<{ hex: string; name: string; hue: string; variation: string }> = []
  
  Object.entries(MATERIAL_VARIATIONS).forEach(([hue, data]) => {
    const variations = data.variations
    const variationNames = [
      'lighten-5', 'lighten-4', 'lighten-3', 'lighten-2', 'lighten-1',
      'base',
      'darken-1', 'darken-2', 'darken-3', 'darken-4'
    ]
    
    // Ajouter les lighten, base et darken
    variations.slice(0, 10).forEach((hex, index) => {
      const label = index < 5 ? variationNames[index] : 
                    index === 5 ? 'base' : 
                    variationNames[index]
      result.push({
        hex: hex.toUpperCase(),
        name: `${data.name} ${label}`,
        hue,
        variation: label
      })
    })
    
    // Ajouter les accent (si présentes)
    if (variations.length > 10) {
      variations.slice(10).forEach((hex, index) => {
        result.push({
          hex: hex.toUpperCase(),
          name: `${data.name} accent-${index + 1}`,
          hue,
          variation: `accent-${index + 1}`
        })
      })
    }
  })
  
  // Ajouter blanc et noir
  EXTRA_SWATCHES.forEach(swatch => {
    result.push({
      hex: swatch.hex,
      name: swatch.name,
      hue: 'extra',
      variation: 'base'
    })
  })
  
  return result
})

// Organisation en colonnes par teinte
const hues = computed(() => {
  const hueMap: Record<string, typeof allSwatches.value> = {}
  allSwatches.value.forEach(swatch => {
    if (!hueMap[swatch.hue]) {
      hueMap[swatch.hue] = []
    }
    hueMap[swatch.hue].push(swatch)
  })
  return Object.entries(hueMap)
})

const safeValue = computed(() =>
  isValidHexAlpha(props.modelValue)
    ? props.modelValue
    : '#FFFFFFFF'
)

const defaultColor = computed(() => {
  const def = props.def.default
  return isValidHexAlpha(def) ? def : '#FFFFFFFF'
})

const currentColorName = computed(() => {
  const hex = safeValue.value.toUpperCase().slice(0, 7)
  // Trouver le nom complet
  const match = allSwatches.value.find(s => s.hex === hex)
  return match ? match.name : hex
})

const defaultColorName = computed(() => {
  const hex = defaultColor.value.toUpperCase().slice(0, 7)
  const match = allSwatches.value.find(s => s.hex === hex)
  return match ? match.name : hex
})

function selectColor(hex: string) {
  // Conserver l'alpha si présent
  const alpha = props.modelValue.slice(7) || 'FF'
  emit('update:modelValue', `${hex}${alpha}`)
}

function isSelected(hex: string) {
  return safeValue.value.toUpperCase().slice(0, 7) === hex.toUpperCase()
}

function isDefault(hex: string) {
  return defaultColor.value.toUpperCase().slice(0, 7) === hex.toUpperCase()
}

function getSwatchStyle(swatch: { hex: string; hue: string }) {
  const style: Record<string, string> = {
    backgroundColor: swatch.hex
  }
  
  // Ajouter une bordure de la couleur primaire pour les dérivées
  if (swatch.hue !== 'extra') {
    const baseColor = MATERIAL_VARIATIONS[swatch.hue]?.variations[5] || swatch.hex
    style.borderColor = baseColor
    style.borderWidth = '2px'
    style.borderStyle = 'solid'
  }
  
  return style
}

function getHueName(hue: string) {
  if (hue === 'extra') return 'Autres'
  return MATERIAL_VARIATIONS[hue]?.name || hue
}
</script>

<template>
  <div>
    <!-- Grille de couleurs par teinte -->
    <div class="palette-container">
      <div
        v-for="[hue, swatches] in hues"
        :key="hue"
        class="hue-section"
      >
        <div
          v-if="showHueLabels"
          class="hue-label text-caption text-medium-emphasis"
        >
          {{ getHueName(hue) }}
        </div>
        <div class="color-row">
          <div
            v-for="swatch in swatches"
            :key="swatch.hex"
            class="color-cell"
            :class="{ 
              selected: isSelected(swatch.hex),
              isDefault: isDefault(swatch.hex)
            }"
            @click="selectColor(swatch.hex)"
            :title="swatch.name"
          >
            <div
              class="color-swatch"
              :style="getSwatchStyle(swatch)"
            >
              <!-- Indicateur de sélection -->
              <div v-if="isSelected(swatch.hex)" class="selection-indicator">
                <v-icon
                  v-if="swatch.hex.toLowerCase() === '#ffffff'"
                  icon="mdi-check"
                  size="16"
                  color="black"
                />
                <v-icon
                  v-else
                  icon="mdi-check"
                  size="16"
                  color="white"
                />
              </div>
              
              <!-- Indicateur de valeur par défaut -->
              <div v-if="isDefault(swatch.hex) && !isSelected(swatch.hex)" class="default-indicator">
                <v-icon
                  v-if="swatch.hex.toLowerCase() === '#ffffff'"
                  icon="mdi-circle-outline"
                  size="12"
                  color="black"
                />
                <v-icon
                  v-else
                  icon="mdi-circle-outline"
                  size="12"
                  color="white"
                />
              </div>
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
      
      <div class="d-flex align-center">
        <!-- Checkbox pour masquer/afficher les noms des teintes -->
        <v-checkbox
          v-model="showHueLabels"
          label="Afficher les noms"
          density="compact"
          hide-details
          class="mr-3"
        />
        
        <div class="d-flex align-center default-info">
          <span class="text-caption text-medium-emphasis mr-2">Défaut :</span>
          <div class="default-preview" :style="{ backgroundColor: defaultColor }"></div>
          <span class="text-caption text-medium-emphasis ml-2">{{ defaultColorName }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.palette-container {
  max-height: 400px;
  overflow-y: auto;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.1);
  border-radius: 6px;
  padding: 12px;
}

.hue-section {
  margin-bottom: 12px;
}

.hue-section:last-child {
  margin-bottom: 0;
}

.hue-label {
  font-weight: 500;
  margin-bottom: 4px;
  padding: 0 4px;
}

.color-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.color-cell {
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  transition: all 0.15s ease;
  position: relative;
  flex: 0 0 auto;
}

.color-cell:hover {
  transform: scale(1.05);
  z-index: 1;
}

.color-swatch {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  transition: all 0.15s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.color-cell:hover .color-swatch {
  border-color: rgba(var(--v-theme-on-surface), 0.2);
}

/* Sélection avec indicateur de check */
.color-cell.selected .color-swatch {
  border-color: rgba(var(--v-theme-on-surface), 0.08);
}

.color-cell.isDefault:not(.selected) .color-swatch {
  border-color: rgba(var(--v-theme-on-surface), 0.08);
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

.default-indicator {
  position: absolute;
  bottom: 1px;
  right: 1px;
  opacity: 0.8;
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
  .color-swatch {
    width: 24px;
    height: 24px;
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