<template>
  <v-navigation-drawer
    v-model="isOpen"
    location="right"
    width="460"
    temporary
    floating
  >
    <v-list density="compact" nav>
      <v-list-subheader class="text-uppercase font-weight-bold">
        Paramètres d'édition
      </v-list-subheader>

      <!-- Une carte ParameterCard par paramètre, groupée par section.
           On réutilise le composant générique existant : définition, draft,
           persistance et validation sont gérés par le store settings, comme
           dans le SettingsDrawer d'accueil. -->
      <template v-for="section in sections" :key="section.title">
        <v-list-subheader class="text-caption font-weight-bold mt-3">
          <v-icon start :icon="section.icon" size="small" />
          {{ section.title }}
        </v-list-subheader>

        <v-list-item v-for="key in section.keys" :key="key" class="px-0">
          <ParameterCard :param-key="key" />
        </v-list-item>
      </template>
    </v-list>
  </v-navigation-drawer>

  <!-- Bouton d'ouverture (à placer dans la toolbar par la vue parent) -->
  <v-btn
    icon="mdi-tune-variant"
    variant="text"
    title="Paramètres d'édition"
    @click="isOpen = true"
  />
</template>

<script setup lang="ts">
/**
 * Panneau de paramètres dédié à l'atelier d'édition caméra (Mode 2).
 *
 * Contrairement au `SettingsDrawer` générique (qui lit les catégories depuis
 * `[_meta]`), ce composant expose directement les paramètres `EditionCamera.*`
 * groupés par section thématique, via le composant `ParameterCard` réutilisé
 * tel quel. Les paramètres sont définis dans `settings.default.toml` mais
 * délibérément absents de `[_meta.views.editionCamera].groups` : ils
 * n'apparaissent donc QUE ici, jamais dans le drawer global.
 *
 * Le binding `v-model="isOpen"` permet à la vue parent de piloter l'ouverture
 * (ou d'utiliser le bouton intégré).
 */
import { ref } from 'vue'
import ParameterCard from '../parameters/ParameterCard.vue'

/** État d'ouverture du drawer. */
const isOpen = ref(false)

/** Sections thématiques et leurs paramètres (paths `EditionCamera.*`). */
const sections = [
  {
    title: 'Résolution de pré-calcul',
    icon: 'mdi-resize',
    keys: [
      'EditionCamera.Viewport.largeurReference',
      'EditionCamera.Viewport.hauteurReference',
    ],
  },
  {
    title: 'Échantillonnage',
    icon: 'mdi-timer-outline',
    keys: ['EditionCamera.PreCalcul.sampleRate'],
  },
  {
    title: 'Caméra par défaut',
    icon: 'mdi-camera',
    keys: [
      'EditionCamera.Cam.defaultZoom',
      'EditionCamera.Cam.defaultPitch',
      'EditionCamera.Cam.defaultBearing',
    ],
  },
  {
    title: 'Zone morte',
    icon: 'mdi-image-filter-center-focus',
    keys: [
      'EditionCamera.ZoneMorte.deadZoneX',
      'EditionCamera.ZoneMorte.deadZoneY',
      'EditionCamera.ZoneMorte.anticipationTime',
      'EditionCamera.ZoneMorte.bearingSmoothThreshold',
    ],
  },
  {
    title: 'Vols (FlyTo)',
    icon: 'mdi-airplane-takeoff',
    keys: [
      'EditionCamera.FlyTo.minFlyDuration',
      'EditionCamera.FlyTo.maxFlyDuration',
      'EditionCamera.FlyTo.speedFactor',
      'EditionCamera.FlyTo.zoomAltitudeFactor',
    ],
  },
  {
    title: 'Traceur',
    icon: 'mdi-map-marker',
    keys: [
      'EditionCamera.Traceur.markerColor',
      'EditionCamera.Traceur.markerSize',
      'EditionCamera.Traceur.markerAltitudeOffset',
      'EditionCamera.Traceur.showTrail',
    ],
  },
]
</script>
