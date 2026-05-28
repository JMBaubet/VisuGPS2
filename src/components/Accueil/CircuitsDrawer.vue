<script setup lang="ts">
import Circuit from "./Circuit.vue";

defineProps<{
  modelValue?: boolean
}>()


</script>

<template>
  <v-navigation-drawer
    permanent
    width="500"
    class="py-2"
  >
    <!-- Navigation Drawer (Sidebar) - 500px -->
    <v-btn
      icon="mdi-image-plus-outline"
      variant="flat"
    ></v-btn>
    <v-btn
      icon="mdi-image-edit-outline"
      variant="flat"
    ></v-btn>
    <v-btn
      icon="mdi-image-search-outline"
      variant="flat"
    ></v-btn>
  
    <v-list-item class="pt-4 px-0">
      <Circuit 
        backgroundColor="bg-yellow"
        nom="2024 Santa susanna 122 col turo de home"
        :communes="100"
        url="https://www.openrunner.com/route-details/18239499"
      />
      <Circuit
        backgroundColor="bg-green"
        nom="2024 Santa susanna Sélection"
        :communes="99" 
      />
      <Circuit
        backgroundColor="bg-green"
        nom="2026 la gaillarde tanneron"
      />
      <Circuit
        nom="2026 altea confrigue 125-22943082"
      />
      <Circuit
        nom="45 km avec boucle"
      />
      <Circuit
        nom="2026 calpe Pedreguer"
      />
      <Circuit
        nom="2024 santa susanna 70 bord de mer sud"
      />
    </v-list-item>
  </v-navigation-drawer>
</template>
