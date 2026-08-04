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
      path: '/edition-camera/:traceId',
      name: 'editionCamera',
      component: EditionCamera
    },
    {
      path: '/screen-bis',
      name: 'screenBis',
      component: ScreenBis
    }
  ]
})

export default router
