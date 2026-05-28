<script setup lang="ts">
import ModeExecutionBtnToggle from '../ModeExecutionBtnToggle.vue'

const props = defineProps<{
  theme: string
}>()

const emit = defineEmits<{
  (e: 'update:theme', val: string): void
  (e: 'open-settings'): void
}>()

function changeMode() {
  emit('update:theme', props.theme === 'light' ? 'dark' : 'light')
}
</script>

<template>
  <v-app-bar>
    <!-- Informations sur le mode d'exécution-->
    <ModeExecutionBtnToggle />
    <v-spacer></v-spacer>
    
    <v-icon
      class="ma-2"
      color="green"
      icon="mdi-web-check"
    ></v-icon>
  
    <v-btn
      :icon="props.theme === 'light' ? 'mdi-weather-sunny' : 'mdi-weather-night'"
      slim
      @click="changeMode"
    ></v-btn>
    <v-btn
      icon="mdi-cog-outline"
      slim
      @click="emit('open-settings')"
    ></v-btn>
    <v-btn
      color="red"
      icon="mdi-exit-to-app"
    ></v-btn>
  </v-app-bar>
</template>
