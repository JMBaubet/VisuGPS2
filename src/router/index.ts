import { createRouter, createWebHistory } from 'vue-router'
import Accueil from '../views/Accueil.vue'
import Visualisation from '../views/Visualisation.vue'
import EditionCamera from '../views/EditionCamera.vue'
import ScreenBis from '../views/ScreenBis.vue'
import Cleaning from '../views/Cleaning.vue'

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
      path: '/nettoyage',
      name: 'nettoyage',
      component: Cleaning
    },
    {
      path: '/screen-bis',
      name: 'screenBis',
      component: ScreenBis
    }
  ]
})

export default router
