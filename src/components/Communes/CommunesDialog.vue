<script setup lang="ts">
/**
 * Fenêtre de consultation des communes traversées par une trace.
 *
 * Ouverte depuis la carte du circuit (icône verte) : elle restitue le contenu
 * de `traces/{trace_id}/communes.json` — un tableau des passages dans l'ordre
 * chronologique, chacun portant son numéro quand la commune est traversée
 * plusieurs fois. La trace consultée est désignée par
 * `appStore.communesTraceId`, comme les autres fenêtres globales de
 * l'application ; le composant est monté une seule fois, sur l'accueil.
 *
 * En **mode debug** (`appStore.isDebug`), un pied de fenêtre permet de
 * supprimer la détection d'un circuit — fichier `communes.json` et drapeau du
 * registre — pour la rejouer.
 */
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../../stores/app'
import { useCommunesStore } from '../../stores/communes'
import type { CommunePassage } from '../../stores/communes'
import { useTracesStore } from '../../stores/traces'

const appStore = useAppStore()
const communesStore = useCommunesStore()
const tracesStore = useTracesStore()

/** Ouverture de la fenêtre : une trace est désignée. */
const isOpen = computed(() => appStore.communesTraceId !== null)

/** Trace consultée — nom affiché et cible de la réinitialisation. */
const trace = computed(
  () => tracesStore.traces.find(t => t.id === appStore.communesTraceId) ?? null,
)

/** Passages lus dans `communes.json` (`null` tant que la lecture n'a pas abouti). */
const passages = ref<CommunePassage[] | null>(null)
/** Vrai pendant la lecture du fichier. */
const loading = ref(false)
/** Vrai pendant la suppression de la détection (mode debug). */
const suppressionEnCours = ref(false)
/** Vrai quand la suppression attend confirmation. */
const confirmationOuverte = ref(false)

/** Nombre de communes distinctes traversées. */
const communeCount = computed(() => new Set((passages.value ?? []).map(p => p.commune)).size)

/** Kilométrage cumulé dans les communes — peut être inférieur à la longueur
 *  totale de la trace si une portion passe hors de toute commune connue. */
const totalKm = computed(() => (passages.value ?? []).reduce((sum, p) => sum + p.longueur_km, 0))

/** Lecture de la description à l'ouverture, et à chaque changement de trace. */
watch(
  () => appStore.communesTraceId,
  async (traceId) => {
    confirmationOuverte.value = false
    if (traceId === null) return
    loading.value = true
    passages.value = await communesStore.loadPassages(traceId)
    loading.value = false
  },
)

/** Ferme la fenêtre. */
function fermer() {
  appStore.closeCommunes()
}

/** Ferme la fenêtre quand le dialogue se referme (Échap, clic hors cadre). */
function onDialogUpdate(value: boolean) {
  if (!value) fermer()
}

/** Supprime la détection de cette trace (diagnostic, mode debug). */
async function supprimerDetection() {
  const traceId = appStore.communesTraceId
  if (traceId === null) return
  suppressionEnCours.value = true
  try {
    await communesStore.resetDetection(traceId)
    confirmationOuverte.value = false
    fermer()
  } finally {
    suppressionEnCours.value = false
  }
}
</script>

<template>
  <v-dialog
    :model-value="isOpen"
    max-width="760"
    @update:model-value="onDialogUpdate"
  >
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon icon="mdi-city" class="mr-2" />
        <span class="text-h6">Communes traversées</span>
        <v-spacer></v-spacer>
        <span class="text-body-2 text-medium-emphasis text-truncate">{{ trace?.name ?? '' }}</span>
      </v-card-title>

      <v-divider></v-divider>

      <v-card-text>
        <div v-if="loading" class="d-flex justify-center py-6">
          <v-progress-circular indeterminate size="28" />
        </div>

        <!-- Rien à restituer : fichier absent ou illisible. -->
        <v-alert
          v-else-if="passages === null"
          type="info"
          variant="tonal"
          density="comfortable"
        >
          Description des communes illisible pour ce circuit.
        </v-alert>

        <!-- Détection aboutie sans aucune commune : c'est un résultat. -->
        <v-alert
          v-else-if="passages.length === 0"
          type="info"
          variant="tonal"
          density="comfortable"
        >
          Aucune commune traversée détectée pour ce circuit.
        </v-alert>

        <template v-else>
          <p class="text-body-2 mb-3">
            {{ passages.length }} passage{{ passages.length > 1 ? 's' : '' }} dans
            {{ communeCount }} commune{{ communeCount > 1 ? 's' : '' }} —
            {{ totalKm.toFixed(2) }} km couverts.
          </p>

          <v-table density="compact">
            <thead>
              <tr>
                <th class="text-right">Km entrée</th>
                <th>Commune</th>
                <th class="text-right">Km sortie</th>
                <th class="text-right">Longueur</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(passage, index) in passages" :key="index">
                <td class="text-right">{{ passage.km_entree.toFixed(3) }}</td>
                <td>
                  {{ passage.commune }}
                  <v-chip
                    v-if="passage.nb_passages_total > 1"
                    class="ml-1"
                    size="x-small"
                    color="blue"
                    variant="tonal"
                  >
                    passage {{ passage.passage }}/{{ passage.nb_passages_total }}
                  </v-chip>
                </td>
                <td class="text-right">{{ passage.km_sortie.toFixed(3) }}</td>
                <td class="text-right">{{ passage.longueur_km.toFixed(3) }} km</td>
              </tr>
            </tbody>
          </v-table>
        </template>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions>
        <!-- Diagnostic : réservé au mode debug. -->
        <v-btn
          v-if="appStore.isDebug && passages !== null"
          variant="text"
          color="red-darken-3"
          prepend-icon="mdi-delete-outline"
          @click="confirmationOuverte = true"
        >
          Supprimer la détection
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="fermer">
          Fermer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>

  <!-- Confirmation de suppression de la détection (diagnostic, mode debug) -->
  <v-dialog v-model="confirmationOuverte" max-width="420" persistent>
    <v-card>
      <v-card-title class="text-h6">
        Supprimer la détection ?
      </v-card-title>
      <v-card-text>
        Le fichier <code>communes.json</code> de
        <b>« {{ trace?.name ?? 'ce circuit' }} »</b> sera supprimé et sa détection
        repassera « à faire ».
        <br />Elle sera rejouée au prochain lancement, ou au clic sur l'icône
        <v-icon size="x-small" icon="mdi-city" /> de la carte du circuit.
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn
          variant="text"
          :disabled="suppressionEnCours"
          @click="confirmationOuverte = false"
        >
          Annuler
        </v-btn>
        <v-btn
          variant="flat"
          color="red-darken-3"
          :loading="suppressionEnCours"
          @click="supprimerDetection"
        >
          Supprimer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>
