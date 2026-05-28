<script setup lang="ts">
import { useAppStore } from '../../stores/app'

const appStore = useAppStore()

defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

function openModes() {
  emit('update:modelValue', false)
  appStore.showModeDialog = true
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
        ></v-list-item>
        <v-list-item
        prepend-icon="mdi-key-chain"
        title="Licences"
        value="licences"
        ></v-list-item>
    </v-list>
  </v-navigation-drawer>
</template>
