import { createRouter, createWebHistory } from 'vue-router'
import Accueil from '../views/Accueil.vue'
import Visualisation from '../views/Visualisation.vue'
import EditionCamera from '../views/EditionCamera.vue'

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
    }
  ]
})

export default router
