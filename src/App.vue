<template>
  <v-app :theme="appStore.theme">
    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { useAppStore } from './stores/app'

const appStore = useAppStore()
</script>
