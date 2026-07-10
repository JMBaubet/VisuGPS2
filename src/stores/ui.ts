/**
 * Store Pinia des notifications (snackbar mutualisé).
 *
 * Centralise l'état d'affichage des snackbars pour éviter de dupliquer
 * la logique dans chaque composant. Le composant v-snackbar doit être
 * rendu une seule fois, idéalement au niveau d'App.vue ou Accueil.vue.
 *
 * Pattern Setup Store (cf. app.ts, settings.ts).
 */

import { defineStore } from 'pinia'
import { reactive } from 'vue'

/** Couleurs possibles du snackbar Vuetify. */
type SnackbarColor = 'success' | 'error' | 'warning' | 'info'

export const useUiStore = defineStore('ui', () => {
  // État réactif du snackbar
  const snackbar = reactive({
    show: false,
    message: '',
    color: 'success' as SnackbarColor,
  })

  /** Affiche un snackbar de succès (vert). */
  function showSuccess(message: string) {
    snackbar.message = message
    snackbar.color = 'success'
    snackbar.show = true
  }

  /** Affiche un snackbar d'erreur (rouge). */
  function showError(message: string) {
    snackbar.message = message
    snackbar.color = 'error'
    snackbar.show = true
  }

  /** Affiche un snackbar d'avertissement (orange). */
  function showWarning(message: string) {
    snackbar.message = message
    snackbar.color = 'warning'
    snackbar.show = true
  }

  /** Affiche un snackbar d'information (bleu). */
  function showInfo(message: string) {
    snackbar.message = message
    snackbar.color = 'info'
    snackbar.show = true
  }

  /** Ferme le snackbar. */
  function hideSnackbar() {
    snackbar.show = false
  }

  // Exposition publique
  return {
    snackbar,
    showSuccess,
    showError,
    showWarning,
    showInfo,
    hideSnackbar,
  }
})
