<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <v-app-bar density="compact" color="surface" elevation="1">
        <v-btn
          icon="mdi-arrow-left"
          variant="text"
          title="Retour à l'accueil"
          @click="goHome"
        />

        <v-app-bar-title>
          <v-icon icon="mdi-server-network" size="18" class="mr-2" />
          Hermes
        </v-app-bar-title>

        <template #append>
          <v-btn
            icon="mdi-refresh"
            variant="text"
            title="Actualiser"
            :loading="hermesStore.loading"
            @click="refresh"
          />
          <v-btn
            icon="mdi-cog-outline"
            variant="text"
            :color="appStore.isSettingsDrawerOpen ? 'primary' : ''"
            title="Paramètres Hermes"
            @click="toggleSettings"
          />
        </template>
      </v-app-bar>

      <v-main>
        <div class="hermes-placeholder">
          <v-card max-width="640" width="100%" variant="tonal">
            <v-card-title class="d-flex align-center">
              <v-icon
                :icon="hermesStore.statusDisplay.icon"
                :color="hermesStore.statusDisplay.color"
                class="mr-2"
              />
              État du serveur
              <v-spacer />
              <v-chip
                :color="hermesStore.statusDisplay.color"
                variant="tonal"
                size="small"
              >
                {{ hermesStore.statusDisplay.label }}
              </v-chip>
            </v-card-title>

            <v-card-text>
              <div class="hermes-line">
                <span class="hermes-line__label">URL</span>
                <span>{{ hermesStore.baseUrl }}</span>
              </div>
              <div class="hermes-line">
                <span class="hermes-line__label">Dataset actif</span>
                <span>{{ hermesStore.activeDataset?.id ?? 'Aucun' }}</span>
              </div>
              <div class="hermes-line">
                <span class="hermes-line__label">Dernier ping</span>
                <span>{{ lastPingLabel }}</span>
              </div>

              <v-alert
                v-if="hermesStore.lastError"
                type="error"
                variant="tonal"
                density="compact"
                class="mt-3"
                :text="hermesStore.lastError"
              />

              <div v-if="hermesStore.wakeProgress" class="mt-3 text-body-2">
                Réveil en cours — tentative
                {{ hermesStore.wakeProgress.attempts }},
                {{ hermesStore.wakeProgress.elapsedSec }} s.
              </div>

              <div class="d-flex ga-2 mt-4">
                <v-btn
                  v-if="hermesStore.server !== 'online'"
                  color="primary"
                  variant="flat"
                  prepend-icon="mdi-power"
                  :loading="hermesStore.server === 'waking'"
                  :disabled="hermesStore.server === 'waking'"
                  @click="wake"
                >
                  Réveiller le serveur
                </v-btn>
                <v-btn
                  variant="tonal"
                  prepend-icon="mdi-refresh"
                  :loading="hermesStore.loading"
                  @click="refresh"
                >
                  Tester la connexion
                </v-btn>
              </div>
            </v-card-text>
          </v-card>

          <p class="text-body-2 text-medium-emphasis mt-6">
            La carte des régions et la gestion des datasets, polygones et
            instances ORS seront ajoutées à la sous-étape suivante.
          </p>
        </div>
      </v-main>

      <SettingsDrawer :show-system="false" />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue Hermes — squelette de la sous-étape 1.3.
 *
 * Elle monte la route, le drawer de paramètres limité aux groupes
 * `Hermes.Connexion` / `Hermes.Reveil`, et vérifie la connexion au serveur.
 * La carte Mapbox et les panneaux datasets/régions/polygones arrivent en 1.4.
 */
import { computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useHermesStore } from '../stores/hermes'
import { useUiStore } from '../stores/ui'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'
import { formatDateTime } from '../utils/format'

const router = useRouter()
const appStore = useAppStore()
const hermesStore = useHermesStore()
const ui = useUiStore()

const lastPingLabel = computed(() => formatDateTime(hermesStore.lastPingAt))

function goHome(): void {
  void router.push({ name: 'accueil' })
}

function toggleSettings(): void {
  appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen
}

async function refresh(): Promise<void> {
  await hermesStore.refreshServer()
}

async function wake(): Promise<void> {
  const ok = await hermesStore.wake()
  if (ok) {
    ui.showSuccess('Serveur hermes réveillé.')
  } else {
    ui.showWarning("Le serveur hermes n'a pas répondu dans le délai imparti.")
  }
}

onMounted(() => {
  void hermesStore.refreshServer()
})

onUnmounted(() => {
  hermesStore.cancelPolling()
})
</script>

<style scoped>
.hermes-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  padding: 32px;
  text-align: center;
}

.hermes-line {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 0;
  font-size: 0.9rem;
}

.hermes-line__label {
  color: rgba(var(--v-theme-on-surface), 0.65);
}
</style>
