// Palettes Material Design et utilitaires de couleur.
//
// Toutes les couleurs sont exprimées en hexadécimal avec canal alpha (#RRGGBBAA),
// format de stockage retenu pour les paramètres de type rgba / material_*.
//
// Sources des teintes : palette officielle Material Design 2014 (nuances 50 → 900),
// canal alpha forcé à FF (opaque) pour les swatches Material.

/** Teinte Material Design (les 19 couleurs de base). */
export type MaterialHue =
  | 'red' | 'pink' | 'purple' | 'deep-purple' | 'indigo'
  | 'blue' | 'light-blue' | 'cyan' | 'teal' | 'green'
  | 'light-green' | 'lime' | 'yellow' | 'amber' | 'orange'
  | 'deep-orange' | 'brown' | 'blue-grey' | 'grey'

/** Nuance Material Design. brown/grey/blue-grey n'exposent pas 500-A700 accent. */
export type MaterialShade =
  | '50' | '100' | '200' | '300' | '400' | '500'
  | '600' | '700' | '800' | '900'

// Palette complète 50 → 900 par teinte (hex sans alpha, puis alpha ajouté à l'usage).
const RAW_PALETTES: Record<MaterialHue, Record<MaterialShade, string>> = {
  red: { '50': 'FFEBEE', '100': 'FFCDD2', '200': 'EF9A9A', '300': 'E57373', '400': 'EF5350', '500': 'F44336', '600': 'E53935', '700': 'D32F2F', '800': 'C62828', '900': 'B71C1C' },
  pink: { '50': 'FCE4EC', '100': 'F8BBD0', '200': 'F48FB1', '300': 'F06292', '400': 'EC407A', '500': 'E91E63', '600': 'D81B60', '700': 'C2185B', '800': 'AD1457', '900': '880E4F' },
  purple: { '50': 'F3E5F5', '100': 'E1BEE7', '200': 'CE93D8', '300': 'BA68C8', '400': 'AB47BC', '500': '9C27B0', '600': '8E24AA', '700': '7B1FA2', '800': '6A1B9A', '900': '4A148C' },
  'deep-purple': { '50': 'EDE7F6', '100': 'D1C4E9', '200': 'B39DDB', '300': '9575CD', '400': '7E57C2', '500': '673AB7', '600': '5E35B1', '700': '512DA8', '800': '4527A0', '900': '311B92' },
  indigo: { '50': 'E8EAF6', '100': 'C5CAE9', '200': '9FA8DA', '300': '7986CB', '400': '5C6BC0', '500': '3F51B5', '600': '3949AB', '700': '303F9F', '800': '283593', '900': '1A237E' },
  blue: { '50': 'E3F2FD', '100': 'BBDEFB', '200': '90CAF9', '300': '64B5F6', '400': '42A5F5', '500': '2196F3', '600': '1E88E5', '700': '1976D2', '800': '1565C0', '900': '0D47A1' },
  'light-blue': { '50': 'E1F5FE', '100': 'B3E5FC', '200': '81D4FA', '300': '4FC3F7', '400': '29B6F6', '500': '03A9F4', '600': '039BE5', '700': '0288D1', '800': '0277BD', '900': '01579B' },
  cyan: { '50': 'E0F7FA', '100': 'B2EBF2', '200': '80DEEA', '300': '4DD0E1', '400': '26C6DA', '500': '00BCD4', '600': '00ACC1', '700': '0097A7', '800': '00838F', '900': '006064' },
  teal: { '50': 'E0F2F1', '100': 'B2DFDB', '200': '80CBC4', '300': '4DB6AC', '400': '26A69A', '500': '009688', '600': '00897B', '700': '00796B', '800': '00695C', '900': '004D40' },
  green: { '50': 'E8F5E9', '100': 'C8E6C9', '200': 'A5D6A7', '300': '81C784', '400': '66BB6A', '500': '4CAF50', '600': '43A047', '700': '388E3C', '800': '2E7D32', '900': '1B5E20' },
  'light-green': { '50': 'F1F8E9', '100': 'DCEDC8', '200': 'C5E1A5', '300': 'AED581', '400': '9CCC65', '500': '8BC34A', '600': '7CB342', '700': '689F38', '800': '558B2F', '900': '33691E' },
  lime: { '50': 'F9FBE7', '100': 'F0F4C3', '200': 'E6EE9C', '300': 'DCE775', '400': 'D4E157', '500': 'CDDC39', '600': 'C0CA33', '700': 'AFB42B', '800': '9E9D24', '900': '827717' },
  yellow: { '50': 'FFFDE7', '100': 'FFF9C4', '200': 'FFF59D', '300': 'FFF176', '400': 'FFEE58', '500': 'FFEB3B', '600': 'FDD835', '700': 'FBC02D', '800': 'F9A825', '900': 'F57F17' },
  amber: { '50': 'FFF8E1', '100': 'FFECB3', '200': 'FFE082', '300': 'FFD54F', '400': 'FFCA28', '500': 'FFC107', '600': 'FFB300', '700': 'FFA000', '800': 'FF8F00', '900': 'FF6F00' },
  orange: { '50': 'FFF3E0', '100': 'FFE0B2', '200': 'FFCC80', '300': 'FFB74D', '400': 'FFA726', '500': 'FF9800', '600': 'FB8C00', '700': 'F57C00', '800': 'EF6C00', '900': 'E65100' },
  'deep-orange': { '50': 'FBE9E7', '100': 'FFCCBC', '200': 'FFAB91', '300': 'FF8A65', '400': 'FF7043', '500': 'FF5722', '600': 'F4511E', '700': 'E64A19', '800': 'D84315', '900': 'BF360C' },
  brown: { '50': 'EFEBE9', '100': 'D7CCC8', '200': 'BCAAA4', '300': 'A1887F', '400': '8D6E63', '500': '795548', '600': '6D4C41', '700': '5D4037', '800': '4E342E', '900': '3E2723' },
  'blue-grey': { '50': 'ECEFF1', '100': 'CFD8DC', '200': 'B0BEC5', '300': '90A4AE', '400': '78909C', '500': '607D8B', '600': '546E7A', '700': '455A64', '800': '37474F', '900': '263238' },
  grey: { '50': 'FAFAFA', '100': 'F5F5F5', '200': 'EEEEEE', '300': 'E0E0E0', '400': 'BDBDBD', '500': '9E9E9E', '600': '757575', '700': '616161', '800': '424242', '900': '212121' },
}

/** Ajoute le canal alpha (opaque) à une teinte hex 6 digits -> #RRGGBBAA. */
function withAlpha(hex6: string, alpha = 'FF'): string {
  return `#${hex6.toUpperCase()}${alpha}`
}

/**
 * Palettes complètes (50 → 900) par teinte, au format #RRGGBBAA.
 * Utilisée par le sélecteur Material étendu.
 */
export const MATERIAL_FULL_PALETTES: Record<MaterialHue, Record<MaterialShade, string>> =
  (Object.keys(RAW_PALETTES) as MaterialHue[]).reduce((acc, hue) => {
    acc[hue] = (Object.keys(RAW_PALETTES[hue]) as MaterialShade[]).reduce((sub, shade) => {
      sub[shade] = withAlpha(RAW_PALETTES[hue][shade])
      return sub
    }, {} as Record<MaterialShade, string>)
    return acc
  }, {} as Record<MaterialHue, Record<MaterialShade, string>>)

/**
 * Swatches primaires : la nuance 500 de chaque teinte (les 19 couleurs de base).
 * Utilisée par le sélecteur Material primaire (v-color-picker mode swatches).
 */
export const MATERIAL_PRIMARY_SWATCHES: { hue: MaterialHue; hex: string }[] =
  (Object.keys(MATERIAL_FULL_PALETTES) as MaterialHue[]).map((hue) => ({
    hue,
    hex: MATERIAL_FULL_PALETTES[hue]['500'],
  }))

// --- Helpers ---------------------------------------------------------------

/** Valide qu'une chaîne est une couleur hexadécimale avec alpha : #RRGGBBAA. */
export function isValidHexAlpha(s: string | null | undefined): s is string {
  return !!s && /^#[0-9A-Fa-f]{8}$/.test(s)
}

/** Convertit #RRGGBBAA en "rgba(r, g, b, a)" lisible (a normalisé 0..1). */
export function hexToRgbaString(hex: string): string {
  if (!isValidHexAlpha(hex)) return ''
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  const a = parseInt(hex.slice(7, 9), 16) / 255
  return `rgba(${r}, ${g}, ${b}, ${a.toFixed(3)})`
}

/** Distance quadratique entre deux couleurs (comparaison R,G,B). */
function colorDistance(aHex: string, bHex: string): number {
  const ar = parseInt(aHex.slice(1, 3), 16)
  const ag = parseInt(aHex.slice(3, 5), 16)
  const ab = parseInt(aHex.slice(5, 7), 16)
  const br = parseInt(bHex.slice(1, 3), 16)
  const bg = parseInt(bHex.slice(3, 5), 16)
  const bb = parseInt(bHex.slice(5, 7), 16)
  return (ar - br) ** 2 + (ag - bg) ** 2 + (ab - bb) ** 2
}

/**
 * Retourne la couleur Material étendue la plus proche de la couleur donnée,
 * en comparant uniquement les canaux R,G,B (l'alpha est conservé tel quel sur la
 * couleur d'origine). Garantit qu'une couleur hors charte soit rapprochée d'une
 * teinte autorisée.
 */
export function nearestMaterialColor(hex: string): string {
  if (!isValidHexAlpha(hex)) return MATERIAL_FULL_PALETTES.indigo['500']
  let best = MATERIAL_FULL_PALETTES.indigo['500']
  let bestDist = Infinity
  for (const hue of Object.keys(MATERIAL_FULL_PALETTES) as MaterialHue[]) {
    for (const shade of Object.keys(MATERIAL_FULL_PALETTES[hue]) as MaterialShade[]) {
      const candidate = MATERIAL_FULL_PALETTES[hue][shade]
      const dist = colorDistance(hex, candidate)
      if (dist < bestDist) {
        bestDist = dist
        best = candidate
      }
    }
  }
  // Conserve l'alpha de la couleur d'origine sur la teinte Material la plus proche.
  const originalAlpha = hex.slice(7, 9).toUpperCase()
  return withAlpha(best.slice(1, 7), originalAlpha)
}

/** true si la couleur fait partie de la palette Material étendue (à l'alpha près). */
export function isMaterialColor(hex: string): boolean {
  if (!isValidHexAlpha(hex)) return false
  const rgb = hex.slice(1, 7).toUpperCase()
  for (const hue of Object.keys(MATERIAL_FULL_PALETTES) as MaterialHue[]) {
    for (const shade of Object.keys(MATERIAL_FULL_PALETTES[hue]) as MaterialShade[]) {
      if (MATERIAL_FULL_PALETTES[hue][shade].slice(1, 7) === rgb) return true
    }
  }
  return false
}

/** true si la couleur est une nuance 500 d'une teinte Material de base. */
export function isMaterialPrimaryColor(hex: string): boolean {
  if (!isValidHexAlpha(hex)) return false
  const rgb = hex.slice(1, 7).toUpperCase()
  return MATERIAL_PRIMARY_SWATCHES.some((s) => s.hex.slice(1, 7) === rgb)
}
