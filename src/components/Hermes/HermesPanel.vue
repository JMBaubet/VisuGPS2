<template>
  <v-navigation-drawer
    permanent
    width="440"
    location="left"
    class="hermes-panel"
  >
    <div class="hermes-panel__inner">
      <HermesServerCard />

      <v-tabs v-model="tab" density="compact" grow class="hermes-panel__tabs">
        <v-tab value="datasets" prepend-icon="mdi-database">
          Datasets
        </v-tab>
        <v-tab value="regions" prepend-icon="mdi-map-outline">
          Régions
        </v-tab>
        <v-tab value="polygons" prepend-icon="mdi-shape-polygon-plus">
          Polygones
        </v-tab>
      </v-tabs>

      <v-window v-model="tab" class="hermes-panel__window">
        <v-window-item value="datasets">
          <HermesDatasetsSection
            @create-dataset="emit('create-dataset')"
            @delete-dataset="emit('delete-dataset', $event)"
          />
        </v-window-item>
        <v-window-item value="regions">
          <HermesRegionsSection
            :selected-region-id="selectedRegionId"
            @select-region="emit('select-region', $event)"
          />
        </v-window-item>
        <v-window-item value="polygons">
          <HermesPolygonsSection
            :selected-polygon-id="selectedPolygonId"
            :busy="polygonBusy"
            @select-polygon="emit('select-polygon', $event)"
            @create-polygon="emit('create-polygon')"
            @edit-polygon="emit('edit-polygon', $event)"
            @delete-polygon="emit('delete-polygon', $event)"
          />
        </v-window-item>
      </v-window>
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Panneau gauche de la vue Hermes : carte Serveur persistante + onglets
 * Datasets / Régions / Polygones.
 *
 * La sélection est contrôlée par la vue, qui la partage avec la carte.
 */
import { ref } from 'vue'
import HermesServerCard from './HermesServerCard.vue'
import HermesDatasetsSection from './HermesDatasetsSection.vue'
import HermesRegionsSection from './HermesRegionsSection.vue'
import HermesPolygonsSection from './HermesPolygonsSection.vue'
import type {
  HermesDataset,
  HermesPolygon,
} from '../../stores/hermes'

defineProps<{
  selectedRegionId: string | null
  selectedPolygonId: string | null
  polygonBusy: boolean
}>()

const emit = defineEmits<{
  'select-region': [id: string | null]
  'select-polygon': [id: string | null]
  'create-polygon': []
  'edit-polygon': [polygon: HermesPolygon]
  'delete-polygon': [polygon: HermesPolygon]
  'create-dataset': []
  'delete-dataset': [dataset: HermesDataset]
}>()

const tab = ref<'datasets' | 'regions' | 'polygons'>('datasets')
</script>

<style scoped>
.hermes-panel :deep(.v-navigation-drawer__content) {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.hermes-panel__inner {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}

.hermes-panel__tabs {
  flex: 0 0 auto;
}

.hermes-panel__window {
  flex: 1 1 auto;
  min-height: 0;
}

.hermes-panel :deep(.v-window__container),
.hermes-panel :deep(.v-window-item) {
  height: 100%;
}
</style>
