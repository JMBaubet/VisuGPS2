<script setup lang="ts">
/**
 * Panneau latéral affichant la liste des circuits (traces GPX importées).
 *
 * La liste est pilotée par le store `useTracesStore` qui interroge le backend
 * via la commande Tauri `get_traces`. Le backend résout le dossier approprié
 * en fonction du mode d'exécution actif.
 */
import { ref, computed, onMounted } from 'vue'
import Circuit from './Circuit.vue'
import { useTracesStore } from '../../stores/traces'
import { useUiStore } from '../../stores/ui'
import { useSettingsStore } from '../../stores/settings'
import type { TraceMetadata } from '../../stores/traces'

const tracesStore = useTracesStore()
const ui = useUiStore()
const settingsStore = useSettingsStore()

/** Valeur d'un paramètre via son chemin (ex. 'Accueil.nbrCircuits.list'). */
function getParam(path: string): any {
  return settingsStore.getParamDef(path)?.value
}

/** Nombre maximum de circuits présentés dans la liste. */
const maxCircuits = computed(() => getParam('Accueil.nbrCircuits.list') ?? 6)

/** Traces triées par distance, limitées au paramètre nbrCircuits. */
const displayedTraces = computed(() =>
  tracesStore.sortedTracesByDistance.slice(0, maxCircuits.value)
)

/**
 * Déclenche l'import d'un fichier GPX.
 * Gère les différents cas de retour (succès, annulation, doublon, erreur).
 */
async function importerGpx() {
  try {
    const trace = await tracesStore.importerGpx()
    ui.showSuccess(`Trace « ${trace.name} » importée avec succès.`)
  } catch (error) {
    const msg = typeof error === 'string' ? error : ''

    if (msg.includes('Aucun fichier')) {
      // Annulation par l'utilisateur : ne rien afficher
      return
    }

    if (msg.includes('déjà été importée')) {
      ui.showWarning(msg)
      return
    }

    ui.showError(msg || "Une erreur est survenue lors de l'import.")
  }
}

// --- Bascule favori / affichage ---

/** Bascule le favori d'une trace et persiste via le backend. */
function onToggleFavorite(id: string) {
  const trace = tracesStore.traces.find(t => t.id === id)
  if (trace) {
    tracesStore.updateTrace(id, { favorite: !trace.favorite })
  }
}

/** Bascule l'affichage d'une trace et persiste via le backend. */
function onToggleDisplay(id: string) {
  const trace = tracesStore.traces.find(t => t.id === id)
  if (trace) {
    tracesStore.updateTrace(id, { is_displayed: !trace.is_displayed })
  }
}

// --- Suppression d'une trace ---

/** Trace en cours de suppression (pour le dialogue de confirmation). */
const traceASupprimer = ref<TraceMetadata | null>(null)
const dialogSuppression = ref(false)
const isDeleting = ref(false)

function confirmerSuppression(id: string) {
  traceASupprimer.value = tracesStore.traces.find(t => t.id === id) ?? null
  dialogSuppression.value = true
}

function annulerSuppression() {
  dialogSuppression.value = false
  traceASupprimer.value = null
}

async function validerSuppression() {
  if (!traceASupprimer.value) return
  const name = traceASupprimer.value.name
  const id = traceASupprimer.value.id
  isDeleting.value = true

  try {
    await tracesStore.supprimerTrace(id)
    ui.showSuccess(`Trace « ${name} » supprimée.`)
  } catch (error) {
    const msg = typeof error === 'string' ? error : ''
    ui.showError(msg || "Une erreur est survenue lors de la suppression.")
  } finally {
    isDeleting.value = false
    dialogSuppression.value = false
    traceASupprimer.value = null
  }
}

// Chargement initial de la liste des traces
onMounted(() => {
  tracesStore.loadTraces()
})
</script>

<template>
  <v-navigation-drawer
    permanent
    width="500"
    class="py-2"
  >
    <!-- Barre d'actions -->
    <v-btn
      icon="mdi-image-plus-outline"
      variant="flat"
      :disabled="tracesStore.loading"
      title="Importer une trace GPX"
      @click="importerGpx"
    />
    <v-btn
      icon="mdi-image-edit-outline"
      variant="flat"
      title="Éditer une trace"
    />
    <v-btn
      icon="mdi-image-search-outline"
      variant="flat"
      title="Rechercher une trace"
    />

    <!-- Indicateur de chargement -->
    <v-progress-linear
      v-if="tracesStore.loading"
      indeterminate
      color="primary"
    />

    <!-- Compteur de traces -->
    <v-list-item v-if="tracesStore.traces.length > 0" class="px-2">
      <v-list-item-subtitle>
        {{ tracesStore.traceCount }} trace{{ tracesStore.traceCount > 1 ? 's' : '' }} importée{{ tracesStore.traceCount > 1 ? 's' : '' }}
      </v-list-item-subtitle>
    </v-list-item>

    <!-- Liste des traces importées -->
    <v-list-item class="pt-4 px-0">
      <Circuit
        v-for="trace in displayedTraces"
        :key="trace.id"
        :trace="trace"
        @delete="confirmerSuppression"
        @toggle-favorite="onToggleFavorite"
        @toggle-display="onToggleDisplay"
      />
    </v-list-item>

    <!-- État vide -->
    <v-list-item v-if="!tracesStore.loading && tracesStore.traces.length === 0" class="px-4">
      <v-list-item-subtitle class="text-center text-disabled">
        Aucune trace importée.<br />
        Cliquez sur
        <v-icon size="x-small" icon="mdi-image-plus-outline" />
        pour importer un fichier GPX.
      </v-list-item-subtitle>
    </v-list-item>
  </v-navigation-drawer>

  <!-- Dialogue de confirmation de suppression -->
  <v-dialog v-model="dialogSuppression" max-width="400" persistent>
    <v-card>
      <v-card-title class="text-h6">
        Supprimer la trace ?
      </v-card-title>
      <v-card-text>
        Voulez-vous vraiment supprimer la trace
        <b>« {{ traceASupprimer?.name }} »</b> ?
        <br />Le fichier GPX sera définitivement supprimé.
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn
          variant="text"
          :disabled="isDeleting"
          @click="annulerSuppression"
        >
          Annuler
        </v-btn>
        <v-btn
          variant="flat"
          color="red-darken-3"
          :loading="isDeleting"
          @click="validerSuppression"
        >
          Supprimer
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>
