<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useAppStore } from '../../stores/app'
import { useSettingsStore } from '../../stores/settings'
import SettingsEditEntier from './SettingsEditEntier.vue'
import SettingsEditSecret from './SettingsEditSecret.vue'

const appStore = useAppStore()
const settingsStore = useSettingsStore()

defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

onMounted(async () => {
  await settingsStore.loadSettings()
})

const nbrCircuitsSetting = computed(() => 
  settingsStore.settings.find(s => s.path === 'Accueil.nbrCircuits.list')
)

const mapBoxSetting = computed(() => 
  settingsStore.settings.find(s => s.path === 'Systeme.Key.mapBox')
)

const showNbrCircuitsDialog = ref(false)
const showMapBoxDialog = ref(false)

function openModes() {
  emit('update:modelValue', false)
  appStore.showModeDialog = true
}

function openNbrCircuits() {
  emit('update:modelValue', false)
  showNbrCircuitsDialog.value = true
}

function openMapBox() {
  emit('update:modelValue', false)
  showMapBoxDialog.value = true
}

async function handleUpdate(path: string, value: any) {
  try {
    await settingsStore.updateSetting(path, value)
    showNbrCircuitsDialog.value = false
    showMapBoxDialog.value = false
  } catch (err) {
    console.error(err)
  }
}

async function handleReset(path: string) {
  try {
    await settingsStore.resetSetting(path)
    showNbrCircuitsDialog.value = false
    showMapBoxDialog.value = false
  } catch (err) {
    console.error(err)
  }
}
</script>

<template>
  <v-navigation-drawer
    location="right" 
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
    temporary
  >
    <v-list-item
        prepend-icon="mdi-cog"
        title="Paramètres"
    ></v-list-item>

    <v-divider></v-divider>

    <v-list density="compact" nav>
        <v-list-item
        prepend-icon="mdi-database-cog-outline"
        title="Modes d'exécution"
        value="mode"
        @click="openModes"
        ></v-list-item>
        
        <v-list-item
        prepend-icon="mdi-map-legend"
        title="Nbre de circuits affichés"
        value="nbre-circuits"
        @click="openNbrCircuits"
        ></v-list-item>
        
        <v-list-item
        prepend-icon="mdi-key-chain"
        title="Licences"
        value="licences"
        @click="openMapBox"
        ></v-list-item>
    </v-list>
  </v-navigation-drawer>

  <!-- Dialogues d'édition des paramètres -->
  <v-dialog v-model="showNbrCircuitsDialog" max-width="500">
    <SettingsEditEntier
      v-if="nbrCircuitsSetting"
      :setting="nbrCircuitsSetting"
      @update="payload => handleUpdate(payload.path, payload.value)"
      @reset="handleReset"
    />
  </v-dialog>

  <v-dialog v-model="showMapBoxDialog" max-width="500">
    <SettingsEditSecret
      v-if="mapBoxSetting"
      :setting="mapBoxSetting"
      @update="payload => handleUpdate(payload.path, payload.value)"
      @reset="handleReset"
    />
  </v-dialog>
</template>
