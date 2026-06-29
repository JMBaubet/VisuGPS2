<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { SettingDefinition } from '../../stores/settings'
import { useAppStore } from '../../stores/app'
import { renderMarkdown } from '../../utils/markdown'

const props = defineProps<{
  principalSetting: SettingDefinition
  secondaireSetting: SettingDefinition
}>()

const emit = defineEmits<{
  (e: 'update', payload: { path: string; value: string }): void
  (e: 'reset', path: string): void
}>()

const appStore = useAppStore()
const valPrincipal = ref<string>(props.principalSetting.value)
const valSecondaire = ref<string>(props.secondaireSetting.value)
const refreshing = ref(false)

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

async function refreshDisplays() {
  refreshing.value = true
  try {
    await appStore.loadDisplays()
  } catch (err) {
    console.error(err)
  } finally {
    refreshing.value = false
  }
}

const monitorOptions = computed(() => {
  const options = [
    { value: 'origin', title: 'Écran principal (origin)' },
    { value: 'other', title: 'Autre écran (other)' },
    { value: 'builtin', title: 'Écran intégré (builtin)' },
    { value: 'external', title: 'Écran externe (external)' }
  ]

  // Ajouter les moniteurs détectés physiquement
  const physicalMonitors = appStore.displays
    .map(d => d.name)
    .filter((name): name is string => !!name)
  
  // Supprimer les doublons de noms
  const uniquePhysical = [...new Set(physicalMonitors)]

  if (uniquePhysical.length > 0) {
    // Dans Vuetify v-select, on peut passer des objets avec props ou utiliser des sous-headers
    // Pour simplifier, on ajoute les options directement
    uniquePhysical.forEach(name => {
      options.push({ value: name, title: name })
    })
  }

  return options
})

const hasChanges = computed(() => {
  return valPrincipal.value !== props.principalSetting.value ||
         valSecondaire.value !== props.secondaireSetting.value
})

function onSave() {
  if (valPrincipal.value !== props.principalSetting.value) {
    emit('update', { path: props.principalSetting.path, value: valPrincipal.value })
  }
  if (valSecondaire.value !== props.secondaireSetting.value) {
    emit('update', { path: props.secondaireSetting.path, value: valSecondaire.value })
  }
}

function onResetPrincipal() {
  if (props.principalSetting.is_overridden) {
    emit('reset', props.principalSetting.path)
  } else {
    valPrincipal.value = props.principalSetting.default
  }
}

function onResetSecondaire() {
  if (props.secondaireSetting.is_overridden) {
    emit('reset', props.secondaireSetting.path)
  } else {
    valSecondaire.value = props.secondaireSetting.default
  }
}
</script>

<template>
  <v-card class="mx-auto" width="600" elevation="2">
    <v-card-item>
      <div class="d-flex align-center justify-space-between w-100">
        <v-card-title class="text-h6 font-weight-bold text-primary">
          Configuration des fenêtres
        </v-card-title>
        <v-btn
          icon="mdi-refresh"
          variant="text"
          :loading="refreshing"
          title="Rafraîchir la liste des écrans"
          @click="refreshDisplays"
        ></v-btn>
      </div>
    </v-card-item>

    <v-card-text>
      <!-- Fenêtre Principale -->
      <div class="mb-6">
        <div class="d-flex align-center justify-space-between mb-1">
          <span class="font-weight-bold text-subtitle-1">{{ principalSetting.description }}</span>
          <v-btn
            v-if="principalSetting.is_overridden || valPrincipal !== principalSetting.default"
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
        <div class="mt-2 markdown-doc" v-html="renderMarkdown(principalSetting.doc)"></div>
      </div>

      <v-divider class="my-4"></v-divider>

      <!-- Fenêtre Secondaire -->
      <div>
        <div class="d-flex align-center justify-space-between mb-1">
          <span class="font-weight-bold text-subtitle-1">{{ secondaireSetting.description }}</span>
          <v-btn
            v-if="secondaireSetting.is_overridden || valSecondaire !== secondaireSetting.default"
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
        <div class="mt-2 markdown-doc" v-html="renderMarkdown(secondaireSetting.doc)"></div>
      </div>
    </v-card-text>

    <v-card-actions class="justify-end">
      <v-btn
        color="primary"
        variant="elevated"
        :disabled="!hasChanges"
        @click="onSave"
      >
        Enregistrer
      </v-btn>
    </v-card-actions>
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
</style>
