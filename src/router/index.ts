import { createRouter, createWebHistory } from 'vue-router'
import Accueil from '../views/Accueil.vue'
import Visualisation from '../views/Visualisation.vue'
import EditionCamera from '../views/EditionCamera.vue'
import ScreenBis from '../views/ScreenBis.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'accueil',
      component: Accueil
    },
    {
      path: '/visualisation',
      name: 'visualisation',
      component: Visualisation
    },
    {
      path: '/edition-camera',
      name: 'editionCamera',
      component: EditionCamera
    },
    {
      // Lazy loading : Mapbox GL ne doit pas alourdir le bundle principal.
      path: '/audit',
      name: 'audit',
      component: () => import('../views/Audit.vue')
    },
    {
      // Détection des passages multiples — même raison de lazy loading.
      path: '/multiride',
      name: 'multiride',
      component: () => import('../views/Multiride.vue')
    },
    {
      // Analyse altimétrique — lazy loading pour ne pas embarquer Mapbox dans
      // le bundle principal.
      path: '/altimetrie',
      name: 'altimetrie',
      component: () => import('../views/Altimetrie.vue')
    },
    {
      path: '/screen-bis',
      name: 'screenBis',
      component: ScreenBis
    }
  ]
})

export default router
