<template>
  <v-card class="mx-auto" width="600">
    <v-card-title class="headline d-flex justify-space-between align-center">
      Gestion des modes d'exécution
      <v-spacer></v-spacer>

      <!-- Activation du Debug -->
      <div>
        <v-btn
          :icon="appStore.isDebug ? 'mdi-bug-play' : 'mdi-bug-pause'"
          :color="appStore.isDebug ? 'orange' : 'green'"
          variant="text"
          @click="appStore.toggleDebug()"
          :title="appStore.isDebug ? 'Désactiver le mode debug' : 'Activer le mode debug'"
        ></v-btn>
      </div>

      <!-- Menu (Décoratif/Out of scope) -->
      <div>
        <v-btn icon="mdi-menu" variant="text" title="Menu"></v-btn>
      </div>
    </v-card-title>

    <!-- Liste des modes d'exécution disponibles -->
    <v-card-text class="pt-4">
      <v-list>
        <v-list-item v-for="mode in appStore.modes" :key="mode.nom" class="border-bottom py-2">
          <template v-slot:title>
            <span :class="mode.nom === 'OPE' ? 'text-green font-weight-bold' : 'text-blue font-weight-bold'">
              {{ mode.nom !== "OPE" ? mode.nom.substr(5) : "OPE" }}
            </span>
            <div
              class="text-caption text-grey-darken-1"
              style="white-space: normal; font-size: 0.85rem"
            >
              {{ mode.descrition }}
            </div>
            <div class="text-caption text-grey-lighten-1" style="font-size: 0.75rem">
              Créé le : {{ mode.création }}
              <span v-if="mode.révision"> | Modifié le : {{ mode.révision }}</span>
            </div>
          </template>

          <template v-slot:append>
            <!-- Label Actif du mode Prod en vert si OPE sinon en bleu -->
            <v-chip
              v-if="mode.nom === appStore.activeModeProd"
              :color="mode.nom === 'OPE' ? 'success' : 'primary'"
              variant="elevated"
              size="small"
              class="ml-2"
            >
              Actif
            </v-chip>

            <!-- Label Actif du mode Dev : visible uniquement en mode DEV -->
            <v-chip
              v-if="appStore.isDev && mode.nom === appStore.activeModeDev"
              color="orange"
              variant="elevated"
              size="small"
              class="ml-2"
            >
              Actif Dev
            </v-chip>

            <!-- Delete Button (Visible si EVAL, non-actif) -->
            <v-btn
              v-if="mode.nom !== 'OPE' && (mode.nom !== appStore.activeModeDev && mode.nom !== appStore.activeModeProd)"
              icon="mdi-delete"
              variant="text"
              color="error"
              @click="handleDelete(mode.nom)"
              class="ml-2"
              title="Supprimer ce mode"
            ></v-btn>

            <!-- Edit Button (Visible si EVAL) -->
            <v-btn
              v-if="mode.nom !== 'OPE' && (mode.nom !== appStore.activeModeDev && mode.nom !== appStore.activeModeProd)"
              icon="mdi-pencil"
              variant="text"
              color="primary"
              @click="startEdit(mode)"
              class="ml-2"
              title="Modifier ce mode"
            ></v-btn>

            <!-- Select Button (Visible si non-actif) -->
            <v-btn
              v-if="mode.nom !== appStore.activeMode"
              icon="mdi-check"
              variant="text"
              color="success"
              @click="handleSelect(mode.nom)"
              class="ml-2"
              title="Activer ce mode"
            ></v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card-text>

    <v-card-actions>
      <v-btn
        @click="startCreate"
        color="blue"
        variant="flat"
        text="Créer un nouveau mode"
      ></v-btn>
      <v-spacer></v-spacer>
      <v-btn @click="appStore.showModeDialog = false" variant="outlined" text="Fermer"></v-btn>
    </v-card-actions>

    <!-- Formulaire de création/modification des modes -->
    <v-expand-transition>
      <div v-if="showModifications">
        <v-divider></v-divider>
        <v-card class="pa-4 bg-grey-lighten-4" flat>
          <v-card-title class="headline px-0">
            {{ newMode ? "Créer un nouveau mode d'exécution" : "Modifier le mode d'exécution" }}
          </v-card-title>

          <v-card-text class="pb-0 px-0">
            <v-text-field
              label="Nom du mode"
              v-model="editModeName"
              placeholder="Version_x.y.z"
              class="mb-4"
              variant="outlined"
              density="compact"
            ></v-text-field>

            <v-text-field
              label="Description"
              v-model="editModeDescription"
              placeholder="Description de la version d'évaluation"
              variant="outlined"
              density="compact"
            ></v-text-field>

            <v-alert
              v-if="errorMessage"
              type="error"
              density="compact"
              variant="tonal"
              class="mt-2"
            >
              {{ errorMessage }}
            </v-alert>
          </v-card-text>

          <v-card-actions class="px-0 pt-4">
            <v-spacer></v-spacer>
            <v-btn color="orange" variant="text" @click="cancelModifications">Annuler</v-btn>
            <v-btn color="primary" variant="flat" @click="saveMode">Enregistrer</v-btn>
          </v-card-actions>
        </v-card>
      </div>
    </v-expand-transition>

    <!-- Dialogue de confirmation Vuetify -->
    <v-dialog v-model="confirmDialog" max-width="450" persistent>
      <v-card>
        <v-card-title class="headline bg-primary"  >{{ confirmTitle }}</v-card-title>
        <v-card-text style="white-space: pre-line;">{{ confirmMessage }}</v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="onConfirmCancel">Annuler</v-btn>
          <v-btn color="primary" variant="flat" @click="onConfirmOk">Confirmer</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Snackbar pour les erreurs -->
    <v-snackbar v-model="snackbar" color="error" :timeout="4000">
      {{ snackbarMessage }}
    </v-snackbar>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useAppStore, type ModeInfo } from '../../stores/app'

const appStore = useAppStore()

const showModifications = ref(false)
const newMode = ref(false)

const editModeName = ref('')
const editModeDescription = ref('')
const oldModeName = ref('')
const errorMessage = ref('')

// Dialogue de confirmation Vuetify
const confirmDialog = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
let confirmResolve: ((value: boolean) => void) | null = null

function showConfirm(title: string, message: string): Promise<boolean> {
  confirmTitle.value = title
  confirmMessage.value = message
  confirmDialog.value = true
  return new Promise((resolve) => {
    confirmResolve = resolve
  })
}

function onConfirmOk() {
  confirmDialog.value = false
  confirmResolve?.(true)
  confirmResolve = null
}

function onConfirmCancel() {
  confirmDialog.value = false
  confirmResolve?.(false)
  confirmResolve = null
}

// Snackbar pour les erreurs
const snackbar = ref(false)
const snackbarMessage = ref('')

function showError(message: string) {
  snackbarMessage.value = message
  snackbar.value = true
}

function startCreate() {
  newMode.value = true
  editModeName.value = ''
  editModeDescription.value = ''
  errorMessage.value = ''
  showModifications.value = true
}

function startEdit(mode: ModeInfo) {
  newMode.value = false
  editModeName.value = mode.nom.substring(5) 
  editModeDescription.value = mode.descrition
  oldModeName.value = mode.nom
  errorMessage.value = ''
  showModifications.value = true
}

function cancelModifications() {
  showModifications.value = false
  errorMessage.value = ''
}

async function saveMode() {
  errorMessage.value = ''
  const trimmedName = 'EVAL_' + editModeName.value.trim()
  const trimmedDesc = editModeDescription.value.trim()

  if (!trimmedName.startsWith('EVAL_')) {
    errorMessage.value = "Le nom du mode doit obligatoirement commencer par 'EVAL_'"
    return
  }

  if (trimmedName.length <= 5) {
    errorMessage.value = "Le nom du mode après 'EVAL_' ne doit pas être vide"
    return
  }

  try {
    if (newMode.value) {
      await appStore.createMode(trimmedName, trimmedDesc)
    } else {
      await appStore.updateMode(oldModeName.value, trimmedName, trimmedDesc)
    }
    showModifications.value = false
  } catch (error: any) {
    errorMessage.value = error?.toString() || "Une erreur est survenue lors de l'enregistrement"
  }
}

async function handleDelete(nom: string) {
  const confirmed = await showConfirm(`Suppression du mode ${nom.substring(5)}`, `Êtes-vous sûr de vouloir supprimer ce mode  ?`)
  if (confirmed) {
    try {
      await appStore.deleteMode(nom)
    } catch (error: any) {
      showError(error?.toString() || "Erreur de suppression")
    }
  }
}

async function handleSelect(nom: string) {
  const confirmed = await showConfirm(`Activation du mode ${nom.substring(5)}`, `L'application doit redémarrer pour basculer vers le nouveau mode.\n\nÊtes vous sûr de vouloir continuer ?`)
  if (confirmed) {
    try {
      await appStore.selectMode(nom)
    } catch (error: any) {
      showError(error?.toString() || "Erreur lors de la sélection du mode")
    }
  }
}
</script>

<style scoped>
.border-bottom {
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
}
</style>
