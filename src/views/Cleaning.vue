<script setup lang="ts">
/**
 * Vue de nettoyage d'une trace GPX (plein écran, style Accueil/EditionCamera).
 *
 * Une trace qui contient des anomalies (points hors trace, ronds-points,
 * aller-retours) n'est pas **valide** et ne peut pas entrer en édition caméra.
 * Le nettoyage est organisé en **3 étapes séquentielles** (widget « boîte à
 * états » dans la toolbar) : cette vue présente chaque anomalie de la phase
 * courante sur une carte MapBox et permet de corriger **segment par segment**
 * (suppression/déplacement de points), avec validation manuelle obligatoire
 * avant de passer au suivant.
 *
 * Sauvegardes **partielles** à tout moment (bouton Enregistrer) : le GPX reste
 * intact. La **validation d'étape** (bouton « Valider l'étape N », actif quand
 * tous les cas de la phase sont validés) applique les corrections, réécrit le
 * GPX (entrée de l'étape suivante) et avance la phase.
 */
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useCleaningStore } from '../stores/cleaning'
import { useUiStore } from '../stores/ui'
import CleaningMap from '../components/Cleaning/CleaningMap.vue'
import CleaningToolbar from '../components/Cleaning/CleaningToolbar.vue'
import CleaningCasesPanel from '../components/Cleaning/CleaningCasesPanel.vue'
import CleaningPointTable from '../components/Cleaning/CleaningPointTable.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

const router = useRouter()
const appStore = useAppStore()
const cleaning = useCleaningStore()
const ui = useUiStore()

/** État des boutons Enregistrer / Valider l'étape du toolbar. */
const saving = ref(false)
const validating = ref(false)

/** État initial sérialisé (pour détecter des modifications non sauvegardées). */
let initialStateJson = ''
/** true si des corrections locales diffèrent de la dernière sauvegarde. */
const dirty = ref(false)

// --- Dialog de retour sans sauvegarde ---
const backDialog = ref(false)
const backPending = ref(false)

onMounted(async () => {
  // Garde-fou : aucune trace sélectionnée → retour à l'accueil.
  if (!cleaning.selectedTraceId) {
    router.replace({ name: 'accueil' })
    return
  }
  try {
    await cleaning.load()
    initialStateJson = JSON.stringify(cleaning.state ?? null)
    dirty.value = false
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Chargement impossible.'
    ui.showError(msg)
  }
})

// Détection de modifications locales (approximative : comparaison JSON).
function refreshDirty() {
  dirty.value = JSON.stringify(cleaning.state ?? null) !== initialStateJson
}

// --- Actions ---

function onBack() {
  refreshDirty()
  if (dirty.value) {
    backDialog.value = true
  } else {
    router.push({ name: 'accueil' })
  }
}

function quitNow() {
  backDialog.value = false
  router.push({ name: 'accueil' })
}

async function saveAndQuit() {
  if (backPending.value) return
  backPending.value = true
  try {
    await save()
    backDialog.value = false
    router.push({ name: 'accueil' })
  } finally {
    backPending.value = false
  }
}

async function save() {
  try {
    saving.value = true
    await cleaning.save()
    initialStateJson = JSON.stringify(cleaning.state ?? null)
    dirty.value = false
    ui.showSuccess('Progression de nettoyage enregistrée (le GPX original est conservé).')
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Échec de l'enregistrement."
    ui.showError(msg)
  } finally {
    saving.value = false
  }
}

async function onReset() {
  const ok = window.confirm(
    'Abandonner toutes les corrections en cours et relancer la détection ?',
  )
  if (!ok) return
  try {
    await cleaning.reset()
    await cleaning.load()
    initialStateJson = JSON.stringify(cleaning.state ?? null)
    dirty.value = false
    ui.showInfo('Corrections abandonnées, détection relancée.')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Réinitialisation impossible.'
    ui.showError(msg)
  }
}

async function onValidate() {
  try {
    validating.value = true
    const ok = await cleaning.validatePhase()
    if (ok) {
      initialStateJson = JSON.stringify(cleaning.state ?? null)
      dirty.value = false
      ui.showSuccess('Étape validée : le GPX nettoyé est enregistré, étape suivante chargée.')
    } else {
      ui.showWarning("Validation impossible : tous les cas de cette étape doivent être validés.")
    }
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Validation de l'étape impossible."
    ui.showError(msg)
  } finally {
    validating.value = false
  }
}
</script>

<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <CleaningToolbar
        :saving="saving"
        :validating="validating"
        @back="onBack"
        @save="save"
        @reset="onReset"
        @validate="onValidate"
      />

      <!-- Panneau latéral : liste des cas + tableau des points -->
      <v-navigation-drawer permanent width="560" class="py-1">
        <div class="cleaning-drawer">
          <CleaningCasesPanel />
          <v-divider />
          <CleaningPointTable />
        </div>
      </v-navigation-drawer>

      <!-- Carte centrale -->
      <v-main fill-height class="cleaning-main">
        <div class="cleaning-map-wrapper">
          <CleaningMap />
          <div v-if="cleaning.loading" class="cleaning-loading">
            <div class="cleaning-loading-inner">
              <v-progress-circular :size="60" :width="7" color="primary" indeterminate />
              <span class="cleaning-loading-text">Analyse de la trace…</span>
            </div>
          </div>
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupe Nettoyage.Cap de la vue active) -->
      <SettingsDrawer :show-system="false" />

      <!-- Dialog : quitter sans sauvegarder -->
      <v-dialog v-model="backDialog" max-width="420" persistent>
        <v-card>
          <v-card-title class="text-h6">
            Quitter le nettoyage ?
          </v-card-title>
          <v-card-text>
            Des corrections n'ont pas été enregistrées. Le GPX original n'est
            pas modifié. Que voulez-vous faire ?
          </v-card-text>
          <v-card-actions>
            <v-spacer></v-spacer>
            <v-btn variant="text" @click="backDialog = false">
              Annuler
            </v-btn>
            <v-btn variant="text" color="red" @click="quitNow">
              Quitter sans enregistrer
            </v-btn>
            <v-btn
              color="green"
              variant="flat"
              :loading="backPending"
              @click="saveAndQuit"
            >
              Enregistrer et quitter
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>
    </v-layout>
  </v-app>
</template>

<style scoped>
.cleaning-drawer {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

/* La zone carte occupe toute la place restante ; repère d'ancrage. */
.cleaning-main {
  display: flex;
  flex-direction: column;
}

.cleaning-map-wrapper {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
}

.cleaning-loading {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  pointer-events: none;
}

.cleaning-loading-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.cleaning-loading-text {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.92);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
}
</style>
