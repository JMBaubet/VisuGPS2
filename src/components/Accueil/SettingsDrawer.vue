<script setup lang="ts">
// SettingsDrawer : drawer de paramètres.
//
// Pilotage 100 % dynamique : l'organisation (vues, catégories, actions,
// handlers spéciaux) est déclarée dans la table `[_meta]` du fichier
// `settings.default.toml` et exposée par la commande `get_settings_meta`.
// Aucune entrée n'est hardcodée dans ce composant : ajouter un paramètre
// dans le TOML le fait apparaître automatiquement dans la bonne catégorie.
//
// Sections affichées :
//   1. Actions système (ex: Modes d'exécution) — entrées non-paramètres.
//   2. Catégories système (ex: Licences, Configuration des fenêtres).
//   3. Catégories de la vue active (ex: Carte — Traces).
//
// Indicateurs visuels (sans icône additionnelle, pour préserver la flèche
// de l'accordéon) :
//   - criticité  → icône du paramètre en orange (warning) ;
//   - surcharge  → libellé du paramètre en bleu (info).
// Remontés au niveau catégorie (OR sur les enfants).
import { ref, computed, onMounted } from 'vue'
import { useAppStore } from '../../stores/app'
import { useSettingsStore } from '../../stores/settings'
import { useSettingsTree, type CategoryNode } from '../../composables/useSettingsTree'
import ParameterCard from '../parameters/ParameterCard.vue'
import SettingsEditMonitor from './SettingsEditMonitor.vue'
import SettingsCategory from './SettingsCategory.vue'

const appStore = useAppStore()
const settingsStore = useSettingsStore()

const { systemActions, systemCategories, viewCategories } = useSettingsTree()

onMounted(async () => {
  // Le meta est statique : un seul chargement suffit. Les settings sont
  // également chargés ici pour garantir la fraîcheur à l'ouverture du drawer.
  await Promise.all([
    settingsStore.loadSettings(),
    settingsStore.loadSettingsMeta(),
    appStore.loadDisplays(),
  ])
})

// --- Paramètres spéciaux (moniteurs) -------------------------------------
const principalSetting = computed(() =>
  settingsStore.settings.find(s => s.path === 'Affichage.moniteurs.principal')
)
const secondaireSetting = computed(() =>
  settingsStore.settings.find(s => s.path === 'Affichage.moniteurs.secondaire')
)
const hasMultipleDisplays = computed(() => appStore.displays.length > 1)

// --- Dialogues ------------------------------------------------------------
const paramDialog = ref(false)
const currentParamKey = ref<string | null>(null)
const showMonitorDialog = ref(false)

/** Action système (ex: openModes), identifiée par son champ `action`.
 *  Le drawer reste ouvert : on ferme via le bouton dédié, l'icône AppBar
 *  ou un clic sur la carte. */
function onSystemAction(action: string) {
  if (action === 'openModes') {
    appStore.showModeDialog = true
  }
}

function openParam(path: string) {
  currentParamKey.value = path
  paramDialog.value = true
}

function openMonitors() {
  showMonitorDialog.value = true
}

/** Clic sur une catégorie à handler spécial : on délègue l'ouverture. */
function onCategoryClick(category: CategoryNode) {
  if (category.handler === 'monitors' && hasMultipleDisplays.value) {
    openMonitors()
  }
}
</script>

<template>
  <v-navigation-drawer
    location="right"
    width="420"
    :model-value="appStore.isSettingsDrawerOpen"
    @update:model-value="appStore.isSettingsDrawerOpen = $event"
    temporary
  >
    <!-- En-tête : titre + bouton de fermeture -->
    <div class="d-flex align-center px-4 py-3">
      <v-icon icon="mdi-cog" class="mr-3"></v-icon>
      <span class="text-h6">Paramètres</span>
      <v-spacer></v-spacer>
      <v-btn
        icon="mdi-close"
        variant="text"
        density="comfortable"
        title="Fermer"
        @click="appStore.isSettingsDrawerOpen = false"
      ></v-btn>
    </div>

    <v-divider></v-divider>

    <v-list class="settings-list" density="compact" nav>
      <!-- 1. Actions système (entrées non-paramètres) -->
      <template v-if="systemActions.length">
        <v-list-item
          v-for="action in systemActions"
          :key="`action-${action.key}`"
          :prepend-icon="action.icon"
          :title="action.label"
          :value="`action-${action.key}`"
          @click="onSystemAction(action.action)"
        ></v-list-item>
        <v-divider class="my-2"></v-divider>
      </template>

      <!-- 2. Catégories système (communes à toutes les vues) -->
      <SettingsCategory
        v-for="category in systemCategories"
        :key="`sys-${category.id}`"
        :category="category"
        :can-open-handler="category.handler === 'monitors' ? hasMultipleDisplays : true"
        @open-param="openParam"
        @category-click="onCategoryClick"
      />

      <v-divider v-if="systemCategories.length" class="my-2"></v-divider>

      <!-- 3. Catégories de la vue active -->
      <SettingsCategory
        v-for="category in viewCategories"
        :key="`view-${category.id}`"
        :category="category"
        :can-open-handler="true"
        @open-param="openParam"
        @category-click="onCategoryClick"
      />
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

<style scoped>
/* Les items du drawer ne sont pas des cibles de navigation : on neutralise
   la teinte "active" (sélection persistante au clic) pour revenir au fond
   par défaut après l'animation. En Vuetify 3, la teinte est peinte sur un
   élément enfant `.v-list-item__overlay` (géré par opacité), d'où le ciblage
   de cet overlay plutôt que du list-item lui-même. Le survol (hover) reste
   inchangé. */
.settings-list :deep(.v-list-item--active > .v-list-item__overlay) {
  opacity: 0 !important;
}
</style>
