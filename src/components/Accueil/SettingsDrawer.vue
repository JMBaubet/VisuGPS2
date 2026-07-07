<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useAppStore } from '../../stores/app'
import { useSettingsStore } from '../../stores/settings'
import ParameterCard from '../parameters/ParameterCard.vue'
import SettingsEditMonitor from './SettingsEditMonitor.vue'

const appStore = useAppStore()
const settingsStore = useSettingsStore()

defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

onMounted(async () => {
  await Promise.all([
    settingsStore.loadSettings(),
    appStore.loadDisplays()
  ])
})

// --- Paramètres réels ----------------------------------------------------
const nbrCircuitsPath = 'Accueil.nbrCircuits.list'
const mapBoxPath = 'Systeme.Key.mapBox'
const principalSetting = computed(() =>
  settingsStore.settings.find(s => s.path === 'Affichage.moniteurs.principal')
)
const secondaireSetting = computed(() =>
  settingsStore.settings.find(s => s.path === 'Affichage.moniteurs.secondaire')
)

const hasMultipleDisplays = computed(() => appStore.displays.length > 1)

// --- Dialogue générique ParameterCard -----------------------------------
const paramDialog = ref(false)
const currentParamKey = ref<string | null>(null)

// --- Dialogue spécifique moniteurs (carte double) ------------------------
const showMonitorDialog = ref(false)

function openModes() {
  emit('update:modelValue', false)
  appStore.showModeDialog = true
}

function openParam(path: string) {
  emit('update:modelValue', false)
  currentParamKey.value = path
  paramDialog.value = true
}

function openMonitors() {
  emit('update:modelValue', false)
  showMonitorDialog.value = true
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
        @click="openParam(nbrCircuitsPath)"
        ></v-list-item>

        <v-list-item
        prepend-icon="mdi-key-chain"
        title="Licences"
        value="licences"
        @click="openParam(mapBoxPath)"
        ></v-list-item>

        <v-list-item
        v-if="hasMultipleDisplays"
        prepend-icon="mdi-projector"
        title="Configuration des fenêtres"
        value="moniteurs"
        @click="openMonitors"
        ></v-list-item>

        <v-divider class="my-2"></v-divider>

        <!-- Paramètres d'exemple (démonstration ParameterCard) -->
        <v-list-subheader>Exemples par type</v-list-subheader>
        <v-list-item prepend-icon="mdi-toggle-switch-outline" title="Booléen" value="ex-bool" @click="openParam('Exemples.bool')"></v-list-item>
        <v-list-item prepend-icon="mdi-decimal" title="Décimal" value="ex-float" @click="openParam('Exemples.float')"></v-list-item>
        <v-list-item prepend-icon="mdi-numeric" title="Entier" value="ex-int" @click="openParam('Exemples.int')"></v-list-item>
        <v-list-item prepend-icon="mdi-form-textbox-password" title="Secret" value="ex-secret" @click="openParam('Exemples.secret')"></v-list-item>
        <v-list-item prepend-icon="mdi-format-list-bulleted" title="Liste" value="ex-list" @click="openParam('Exemples.list')"></v-list-item>
        <v-list-item prepend-icon="mdi-palette" title="Couleur RGBA" value="ex-rgba" @click="openParam('Exemples.rgba')"></v-list-item>
        <v-list-item prepend-icon="mdi-palette-outline" title="Material primaire" value="ex-mp" @click="openParam('Exemples.materialPrimary')"></v-list-item>
        <v-list-item prepend-icon="mdi-palette-swatch" title="Material étendu" value="ex-me" @click="openParam('Exemples.materialExtended')"></v-list-item>
    </v-list>
  </v-navigation-drawer>

  <!-- Dialogue générique ParameterCard -->
  <v-dialog v-model="paramDialog" max-width="620">
    <ParameterCard
      v-if="currentParamKey"
      :param-key="currentParamKey"
      @close="paramDialog = false"
    />
  </v-dialog>

  <!-- Dialogue spécifique moniteurs (carte double principal/secondaire) -->
  <v-dialog v-model="showMonitorDialog" max-width="600">
    <SettingsEditMonitor
      v-if="showMonitorDialog && principalSetting && secondaireSetting"
      :principal-setting="principalSetting"
      :secondaire-setting="secondaireSetting"
      @close="showMonitorDialog = false"
    />
  </v-dialog>
</template>
