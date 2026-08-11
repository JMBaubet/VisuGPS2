/**
 * Composable singleton partageant l'instance Mapbox de la vue d'édition.
 *
 * `EditionMap.vue` écrit `mapRef.value = map` après l'initialisation de la
 * carte ; `CameraEditor.vue` (widgets d'édition des keyframes) la lit pour
 * piloter la caméra en temps réel (setPitch/setZoom/setBearing/panBy...).
 *
 * Un singleton module permet à deux composants **frères** (EditionMap et
 * CameraEditor, tous deux enfants du wrapper carte) de partager l'instance
 * sans passer par provide/inject ni par le store.
 */
import { ref } from 'vue'
import type { Map } from 'mapbox-gl'

/** Instance Mapbox de la vue d'édition (null tant que la carte n'est pas créée). */
const mapRef = ref<Map | null>(null)

/** Partage l'instance Mapbox entre EditionMap et CameraEditor. */
export function useEditionMap() {
  return { map: mapRef }
}
