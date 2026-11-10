<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <HermesToolbar
        :loading="hermesStore.loading"
        :server-state="hermesStore.server"
        @back="goHome"
        @refresh="refresh"
        @open-settings="toggleSettings"
      />

      <HermesPanel
        :selected-region-id="selectedRegionId"
        :selected-polygon-id="selectedPolygonId"
        @select-region="onSelectRegion"
        @select-polygon="onSelectPolygon"
      />

      <v-main>
        <HermesMap
          :regions="hermesStore.regions"
          :polygons="hermesStore.polygons"
          :selected-region-id="selectedRegionId"
          :selected-polygon-id="selectedPolygonId"
          @select-region="onSelectRegion"
          @select-polygon="onSelectPolygon"
        />
      </v-main>

      <SettingsDrawer :show-system="false" />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Hermes : état du serveur, datasets, régions et polygones custom.
 *
 * Le panneau gauche (carte Serveur + onglets) et la carte Mapbox partagent la
 * même sélection : cliquer une région ou un polygone dans la liste ou sur la
 * carte cadre la carte sur l'objet.
 *
 * Les étapes suivantes ajoutent le dessin/CRUD des polygones (1.5), puis le
 * CRUD des datasets et la gestion ORS (1.6).
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useHermesStore } from '../stores/hermes'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import HermesMap from '../components/Hermes/HermesMap.vue'
import HermesPanel from '../components/Hermes/HermesPanel.vue'
import HermesToolbar from '../components/Hermes/HermesToolbar.vue'

const router = useRouter()
const appStore = useAppStore()
const hermesStore = useHermesStore()

/** Région sélectionnée (contour ou bbox), ou `null`. */
const selectedRegionId = ref<string | null>(null)
/** Polygone custom sélectionné, ou `null`. */
const selectedPolygonId = ref<string | null>(null)

function goHome(): void {
  void router.push({ name: 'accueil' })
}

function toggleSettings(): void {
  appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen
}

async function refresh(): Promise<void> {
  await Promise.all([
    hermesStore.refreshServer(),
    hermesStore.loadRegions(),
    hermesStore.loadPolygons(),
  ])
}

function onSelectRegion(id: string | null): void {
  selectedRegionId.value = id
  if (id) selectedPolygonId.value = null
}

function onSelectPolygon(id: string | null): void {
  selectedPolygonId.value = id
  if (id) selectedRegionId.value = null
}

onMounted(() => {
  void refresh()
})

onUnmounted(() => {
  hermesStore.cancelPolling()
})
</script>
