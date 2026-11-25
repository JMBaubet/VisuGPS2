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
 * Deux informations viennent du serveur hermes et ne sont pas dans le fichier :
 * - `ambiguous` (persisté) marque un passage disputé par deux communes, la
 *   frontière OSM étant imprécise à cet endroit ;
 * - le **hors couverture** (état de session) explique un résultat vide : la
 *   trace sort du dataset servi, ce n'est pas « aucune commune ».
 *
 * **Garde-fou de couverture** : le résumé compare les kilomètres couverts par
 * des passages à la longueur totale de la trace, et le backend pose le statut
 * `partial` au registre quand plus de 5 % du trajet n'est dans aucune commune
 * (trace hors couverture, ou artefact serveur incomplet). L'alerte suit ce
 * statut : elle survit au redémarrage, contrairement à la seule comparaison
 * des totaux.
 *
 * Le bouton **Recalculer** relance la détection de cette seule trace
 * (`communesStore.request(traceId, { force: true })`) ; le fichier précédent
 * reste en place jusqu'au succès. En **mode debug** (`appStore.isDebug`), un
 * pied de fenêtre permet de supprimer la détection — fichier `communes.json` et
 * drapeau du registre — pour la rejouer.
 */
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../../stores/app'
import { useCommunesStore } from '../../stores/communes'
import type { CommunePassage } from '../../stores/communes'
import { useTracesStore } from '../../stores/traces'

const appStore = useAppStore()
const communesStore = useCommunesStore()
const tracesStore = useTracesStore()

/** Passages lus dans `communes.json` (`null` tant que la lecture n'a pas abouti). */
const passages = ref<CommunePassage[] | null>(null)
/** Vrai pendant la lecture du fichier. */
const loading = ref(false)
/** Vrai pendant la suppression de la détection (mode debug). */
const suppressionEnCours = ref(false)
/** Vrai quand la suppression attend confirmation. */
const confirmationOuverte = ref(false)

/** Ouverture de la fenêtre : une trace est désignée. */
const isOpen = computed(() => appStore.communesTraceId !== null)

/** Trace consultée — nom affiché et cible de la réinitialisation. */
const trace = computed(
  () => tracesStore.traces.find(t => t.id === appStore.communesTraceId) ?? null,
)

/** Nombre de communes distinctes traversées. */
const communeCount = computed(() => new Set((passages.value ?? []).map(p => p.commune)).size)

/** Kilométrage cumulé dans les communes — peut être inférieur à la longueur
 *  totale de la trace si une portion passe hors de toute commune connue. */
const totalKm = computed(() => (passages.value ?? []).reduce((sum, p) => sum + p.longueur_km, 0))

/**
 * Longueur de la trace (`stats.distance_m`, avec dénivelé), affichée à côté des
 * kilomètres couverts et servant de référence au garde-fou. `0` tant que la
 * trace n'est pas dans la liste.
 */
const traceTotalKm = computed(() => (trace.value?.stats.distance_m ?? 0) / 1000)

/** Kilomètres de la trace qui ne sont dans aucune commune détectée. */
const uncoveredKm = computed(() =>
  traceTotalKm.value > 0 ? Math.max(0, traceTotalKm.value - totalKm.value) : 0,
)

/**
 * La détection est marquée **partielle** au registre par le backend (plus de
 * 5 % de la trace hors de toute commune) : c'est la source de vérité de
 * l'alerte, y compris après un redémarrage.
 */
const coverageIssue = computed(() => trace.value?.communes_status === 'partial')

/** Une détection est en cours (ou en file) pour la trace consultée. */
const isPending = computed(() => {
  const traceId = appStore.communesTraceId
  return traceId !== null && communesStore.isPending(traceId)
})

/** Dataset dont la dernière détection est sortie, ou `null`. */
const outsideDataset = computed(() => {
  const traceId = appStore.communesTraceId
  return traceId === null ? null : communesStore.outsideCoverageDataset(traceId)
})

/** Relit la description d'une trace. */
async function charger(traceId: string): Promise<void> {
  loading.value = true
  try {
    passages.value = await communesStore.loadPassages(traceId)
  } finally {
    loading.value = false
  }
}

/** Ferme la fenêtre. */
function fermer() {
  appStore.closeCommunes()
}

/** Ferme la fenêtre quand le dialogue se referme (Échap, clic hors cadre). */
function onDialogUpdate(value: boolean) {
  if (!value) fermer()
}

/** Relance la détection de cette trace, sans effacer la description actuelle. */
function recalculer() {
  const traceId = appStore.communesTraceId
  if (traceId === null) return
  communesStore.request(traceId, { force: true })
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

/** Lecture de la description à l'ouverture, et à chaque changement de trace. */
watch(
  () => appStore.communesTraceId,
  async traceId => {
    confirmationOuverte.value = false
    if (traceId === null) return
    await charger(traceId)
  },
)

/**
 * Fin d'un recalcul : la description a été réécrite sur disque (ou le recalcul a
 * été refusé, et le fichier est intact). Dans les deux cas, on relit.
 */
watch(isPending, async pending => {
  const traceId = appStore.communesTraceId
  if (pending || traceId === null) return
  await charger(traceId)
})
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

        <!-- Résultat vide : « hors couverture » n'est pas « aucune commune ». -->
        <v-alert
          v-else-if="passages.length === 0"
          :type="outsideDataset || coverageIssue ? 'warning' : 'info'"
          variant="tonal"
          density="comfortable"
        >
          <template v-if="outsideDataset">
            Aucune commune détectée : la trace sort de la couverture du dataset
            <b>{{ outsideDataset }}</b>. Vérifiez le jeu de données servi dans la vue Hermes,
            puis relancez le recalcul.
          </template>
          <template v-else-if="coverageIssue">
            Aucune commune détectée : la trace n'est couverte par aucune commune du
            dataset servi. Vérifiez le jeu de données, puis relancez le recalcul.
          </template>
          <template v-else>
            Aucune commune traversée détectée pour ce circuit.
          </template>
        </v-alert>

        <template v-else>
          <p class="text-body-2 mb-3">
            {{ passages.length }} passage{{ passages.length > 1 ? 's' : '' }} dans
            {{ communeCount }} commune{{ communeCount > 1 ? 's' : '' }} —
            {{ totalKm.toFixed(2) }}<template v-if="traceTotalKm > 0"> / {{ traceTotalKm.toFixed(2) }}</template>
            km couverts.
          </p>

          <v-alert
            v-if="coverageIssue"
            type="warning"
            variant="tonal"
            density="comfortable"
            class="mb-3"
          >
            {{ uncoveredKm.toFixed(1) }} km de la trace ne sont dans aucune commune du
            dataset. La trace sort peut-être de sa couverture, ou le dataset n'indexe pas
            certaines communes.
          </v-alert>

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
                  <v-chip
                    v-if="passage.ambiguous"
                    class="ml-1"
                    size="x-small"
                    color="orange"
                    variant="tonal"
                    title="Frontière OSM imprécise : deux communes se disputent ce passage sur plus d'un mètre."
                  >
                    frontière incertaine
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
        <v-btn
          variant="text"
          prepend-icon="mdi-refresh"
          :loading="isPending"
          :disabled="trace === null"
          @click="recalculer"
        >
          Recalculer
        </v-btn>
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
