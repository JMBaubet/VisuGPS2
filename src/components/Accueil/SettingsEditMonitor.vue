<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { useSettingsStore, SettingDefinition } from '../../stores/settings'
import { useAppStore } from '../../stores/app'
import { renderMarkdown } from '../../utils/markdown'

const props = defineProps<{
  principalSetting: SettingDefinition
  secondaireSetting: SettingDefinition
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const settingsStore = useSettingsStore()

const appStore = useAppStore()
const valPrincipal = ref<string>(props.principalSetting.value)
const valSecondaire = ref<string>(props.secondaireSetting.value)
const refreshing = ref(false)

// États pour les dialogues de confirmation
const showCloseConfirmDialog = ref(false)
const showResetPrincipalDialog = ref(false)
const showResetSecondaireDialog = ref(false)

watch(() => props.principalSetting.value, (newVal) => {
  valPrincipal.value = newVal
})

watch(() => props.secondaireSetting.value, (newVal) => {
  valSecondaire.value = newVal
})

onMounted(async () => {
  if (appStore.displays.length === 0) {
    await refreshDisplays()
  }
})


const monitorOptions = computed(() => {
  const options = [
    { value: 'origin', title: 'Écran principal (origin)' },
    { value: 'other', title: 'Autre écran (other)' },
    { value: 'builtin', title: 'Écran intégré (builtin)' },
    { value: 'external', title: 'Écran externe (external)' }
  ]

  const isWindows = navigator.userAgent.includes('Windows')

  if (!isWindows) {
    const physicalMonitors = appStore.displays
      .map(d => d.name)
      .filter((name): name is string => !!name)
    
    const uniquePhysical = [...new Set(physicalMonitors)]

    if (uniquePhysical.length > 0) {
      uniquePhysical.forEach(name => {
        options.push({ value: name, title: name })
      })
    }
  }

  return options
})

const hasChanges = computed(() => {
  const principalChanged = valPrincipal.value !== props.principalSetting.value
  const secondaireChanged = valSecondaire.value !== props.secondaireSetting.value
  // Détecte aussi les surcharges inutiles (valeur == défaut mais is_overridden)
  const principalStaleOverride = valPrincipal.value === props.principalSetting.default && props.principalSetting.is_overridden
  const secondaireStaleOverride = valSecondaire.value === props.secondaireSetting.default && props.secondaireSetting.is_overridden
  return principalChanged || secondaireChanged || principalStaleOverride || secondaireStaleOverride
})

async function onSave() {
  const principalNeedsSave = valPrincipal.value !== props.principalSetting.value ||
    (valPrincipal.value === props.principalSetting.default && props.principalSetting.is_overridden)

  const secondaireNeedsSave = valSecondaire.value !== props.secondaireSetting.value ||
    (valSecondaire.value === props.secondaireSetting.default && props.secondaireSetting.is_overridden)

  if (principalNeedsSave) {
    await settingsStore.updateSetting(props.principalSetting.path, valPrincipal.value)
  }
  if (secondaireNeedsSave) {
    await settingsStore.updateSetting(props.secondaireSetting.path, valSecondaire.value)
  }
}

// --- Actions de réinitialisation ------------------------------------------

function onResetPrincipal() {
  const isCritical = props.principalSetting.critical || false
  if (isCritical) {
    showResetPrincipalDialog.value = true
  } else {
    void doResetPrincipal()
  }
}

function onResetSecondaire() {
  const isCritical = props.secondaireSetting.critical || false
  if (isCritical) {
    showResetSecondaireDialog.value = true
  } else {
    void doResetSecondaire()
  }
}

async function doResetPrincipal() {
  showResetPrincipalDialog.value = false
  if (props.principalSetting.is_overridden) {
    await settingsStore.resetSetting(props.principalSetting.path)
  } else {
    valPrincipal.value = props.principalSetting.default
  }
}

async function doResetSecondaire() {
  showResetSecondaireDialog.value = false
  if (props.secondaireSetting.is_overridden) {
    await settingsStore.resetSetting(props.secondaireSetting.path)
  } else {
    valSecondaire.value = props.secondaireSetting.default
  }
}

// --- Fermeture avec confirmation ------------------------------------------

function onClose() {
  if (hasChanges.value) {
    showCloseConfirmDialog.value = true
  } else {
    // Réinitialiser les valeurs locales aux valeurs persistées avant de fermer
    valPrincipal.value = props.principalSetting.value
    valSecondaire.value = props.secondaireSetting.value
    emit('close')
  }
}

function confirmClose() {
  showCloseConfirmDialog.value = false
  // Réinitialiser les valeurs locales aux valeurs persistées
  valPrincipal.value = props.principalSetting.value
  valSecondaire.value = props.secondaireSetting.value
  emit('close')
}

// --- États pour l'affichage des boutons ----------------------------------

const isPrincipalModified = computed(() => 
  valPrincipal.value !== props.principalSetting.value
)

const isSecondaireModified = computed(() => 
  valSecondaire.value !== props.secondaireSetting.value
)

const isPrincipalDefault = computed(() =>
  valPrincipal.value === props.principalSetting.default
)

const isSecondaireDefault = computed(() =>
  valSecondaire.value === props.secondaireSetting.default
)

const isPrincipalCritical = computed(() => 
  props.principalSetting.critical || false
)

const isSecondaireCritical = computed(() => 
  props.secondaireSetting.critical || false
)

// Raccourci pour la fermeture avec pertes
const closeBtnColor = computed(() => hasChanges.value ? 'warning' : undefined)
</script>

<template>
  <v-card class="mx-auto" width="600" elevation="2">
    <v-card-item>
      <div class="d-flex align-center justify-space-between w-100">
        <v-card-title class="text-h6 font-weight-bold text-primary d-flex align-center">
          Configuration des fenêtres
          <v-tooltip v-if="principalSetting.critical || secondaireSetting.critical" location="top">
            <template #activator="{ props: tooltipProps }">
              <v-icon
                v-bind="tooltipProps"
                color="warning"
                size="small"
                class="ml-2"
              >
                mdi-alert-circle
              </v-icon>
            </template>
            Un ou plusieurs paramètres sont critiques
          </v-tooltip>
        </v-card-title>
        <div class="d-flex align-center">
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            :color="closeBtnColor"
            title="Fermer"
            @click="onClose"
          ></v-btn>
        </div>
      </div>
    </v-card-item>

    <v-card-text>
      <!-- Fenêtre Principale -->
      <div class="mb-6">
        <div class="d-flex align-center justify-space-between mb-1">
          <span class="font-weight-bold text-subtitle-1 d-flex align-center">
            {{ principalSetting.description }}
            <v-tooltip v-if="isPrincipalCritical" location="top">
              <template #activator="{ props: tooltipProps }">
                <v-icon
                  v-bind="tooltipProps"
                  color="warning"
                  size="small"
                  class="ml-1"
                >
                  mdi-alert-circle
                </v-icon>
              </template>
              Critique
            </v-tooltip>
          </span>
          <v-btn
            v-if="!isPrincipalDefault"
            icon="mdi-undo"
            variant="text"
            color="warning"
            density="comfortable"
            title="Restaurer la valeur par défaut"
            @click="onResetPrincipal"
          ></v-btn>
        </div>
        <v-select
          v-model="valPrincipal"
          :items="monitorOptions"
          item-title="title"
          item-value="value"
          variant="outlined"
          density="comfortable"
          hide-details
        ></v-select>
        <div class="mt-2 markdown-doc" v-html="renderMarkdown(principalSetting.documentation)"></div>
      </div>

      <v-divider class="my-4"></v-divider>

      <!-- Fenêtre Secondaire -->
      <div>
        <div class="d-flex align-center justify-space-between mb-1">
          <span class="font-weight-bold text-subtitle-1 d-flex align-center">
            {{ secondaireSetting.description }}
            <v-tooltip v-if="isSecondaireCritical" location="top">
              <template #activator="{ props: tooltipProps }">
                <v-icon
                  v-bind="tooltipProps"
                  color="warning"
                  size="small"
                  class="ml-1"
                >
                  mdi-alert-circle
                </v-icon>
              </template>
              Critique
            </v-tooltip>
          </span>
          <v-btn
            v-if="!isSecondaireDefault"
            icon="mdi-undo"
            variant="text"
            color="warning"
            density="comfortable"
            title="Restaurer la valeur par défaut"
            @click="onResetSecondaire"
          ></v-btn>
        </div>
        <v-select
          v-model="valSecondaire"
          :items="monitorOptions"
          item-title="title"
          item-value="value"
          variant="outlined"
          density="comfortable"
          hide-details
        ></v-select>
        <div class="mt-2 markdown-doc" v-html="renderMarkdown(secondaireSetting.documentation)"></div>
      </div>
    </v-card-text>

    <v-card-actions class="justify-end">
      <v-btn 
        :color="closeBtnColor" 
        variant="text"
        @click="onClose"
      >
        Fermer
      </v-btn>
      <v-btn
        v-if="hasChanges"
        color="primary"
        variant="elevated"
        @click="onSave"
      >
        Enregistrer
      </v-btn>
    </v-card-actions>

    <!-- Dialogue : réinitialisation du paramètre principal (critique) -->
    <v-dialog v-model="showResetPrincipalDialog" max-width="420">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert-decagram</v-icon>
          Réinitialisation critique
        </v-card-title>
        <v-card-text>
          Le paramètre <strong>{{ principalSetting.description }}</strong> est identifié comme <strong>critique</strong>.
          Voulez-vous vraiment rétablir sa valeur par défaut ?
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showResetPrincipalDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="doResetPrincipal">Réinitialiser</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Dialogue : réinitialisation du paramètre secondaire (critique) -->
    <v-dialog v-model="showResetSecondaireDialog" max-width="420">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert-decagram</v-icon>
          Réinitialisation critique
        </v-card-title>
        <v-card-text>
          Le paramètre <strong>{{ secondaireSetting.description }}</strong> est identifié comme <strong>critique</strong>.
          Voulez-vous vraiment rétablir sa valeur par défaut ?
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showResetSecondaireDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="doResetSecondaire">Réinitialiser</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Dialogue : fermeture avec modifications non enregistrées -->
    <v-dialog v-model="showCloseConfirmDialog" max-width="420">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert</v-icon>
          Modifications non enregistrées
        </v-card-title>
        <v-card-text>
          Des modifications non enregistrées seront perdues.
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showCloseConfirmDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="confirmClose">
            Fermer sans enregistrer
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<style scoped>
.markdown-doc {
  font-size: 0.85rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
  line-height: 1.4;
  padding: 8px 12px;
  background-color: rgba(var(--v-theme-on-surface), 0.03);
  border-radius: 4px;
  border-left: 3px solid rgba(var(--v-theme-primary), 0.6);
}
.markdown-doc :deep(strong) {
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.9);
}
.markdown-doc :deep(code) {
  background-color: rgba(var(--v-theme-on-surface), 0.1);
  padding: 2px 4px;
  border-radius: 3px;
  font-family: monospace;
}
.markdown-doc :deep(ul) {
  margin-left: 20px;
  margin-top: 4px;
  margin-bottom: 4px;
}
.markdown-doc :deep(a) {
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
}
.markdown-doc :deep(a:hover) {
  text-decoration: underline;
}
</style>