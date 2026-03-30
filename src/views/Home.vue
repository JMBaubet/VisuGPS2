<template>
  <v-container>
    <v-row>
      <v-col cols="12">
        <h1>Accueil</h1>
        <p>Bienvenue dans votre application Tauri + Vue + Vuetify + Pinia + Router</p>

        <v-card class="mt-4">
          <v-card-title>Technologies utilisées</v-card-title>
          <v-card-text>
            <v-list>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-language-typescript</v-icon>
                </template>
                <v-list-item-title>TypeScript</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-vuejs</v-icon>
                </template>
                <v-list-item-title>Vue 3</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-material-design</v-icon>
                </template>
                <v-list-item-title>Vuetify 3</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-database</v-icon>
                </template>
                <v-list-item-title>Pinia</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-routes</v-icon>
                </template>
                <v-list-item-title>Vue Router</v-list-item-title>
              </v-list-item>
              <v-list-item>
                <template v-slot:prepend>
                  <v-icon>mdi-application</v-icon>
                </template>
                <v-list-item-title>Tauri</v-list-item-title>
              </v-list-item>
            </v-list>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
</script>
