<script setup lang="ts">
/**
 * Vue d'audit d'une trace GPX (plein écran, style Accueil/EditionCamera).
 *
 * Une trace dont le statut d'audit n'est pas « clean » ne peut pas entrer en
 * édition caméra : elle est auditée ici. Le module détecte les **aller-retours**
 * (AR : rebonds, aiguilles) et les **boucles giratoires** (RP : 270°, 360° et
 * plus), puis propose des corrections — suppression de points, routage
 * OpenRouteService, faux positif — annulables par anomalie. Le GPX n'est réécrit
 * qu'à la validation finale (bouton « Appliquer », actif seulement quand plus
 * aucune anomalie n'est `pending`).
 *
 * Le `traceId` arrive par la query de la route (`/audit?traceId=…`), posée par
 * le bouton Éditer de l'accueil ou le garde-fou d'EditionCamera. À la sortie, le
 * store est **réinitialisé** (décision C.2) — les findings sont volatils.
 *
 * Sous-étape 4.1 : squelette (toolbar, liste, dialogues). La carte et le panneau
 * d'action sont intégrés aux sous-étapes 4.2 et 4.3.
 */
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useAuditStore, type AuditParams } from '../stores/audit'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import AuditToolbar from '../components/Audit/AuditToolbar.vue'
import AuditFindingsPanel from '../components/Audit/AuditFindingsPanel.vue'
import AuditMap from '../components/Audit/AuditMap.vue'
import AuditActionPanel from '../components/Audit/AuditActionPanel.vue'
import ConfirmExitDialog from '../components/Audit/dialogs/ConfirmExitDialog.vue'
import ConfirmApplyDialog from '../components/Audit/dialogs/ConfirmApplyDialog.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const auditStore = useAuditStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

const loading = ref(false)
const dialogExitOpen = ref(false)
const dialogApplyOpen = ref(false)

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find((t) => t.id === traceId.value)?.name ?? 'Trace',
)

/** Valeur d'un paramètre numérique, avec repli sur la valeur par défaut. */
function settingNumber(path: string, fallback: number): number {
  const def = settingsStore.settings.find((s) => s.path === path)
  const value = def?.value
  return typeof value === 'number' ? value : fallback
}

/** Assemble les paramètres du détecteur depuis les réglages `Audit.*`. */
function buildParams(): AuditParams {
  return {
    consolM: settingNumber('Audit.Consolidation.seuil', 0.5),
    tolDeg: settingNumber('Audit.AR.toleranceDeg', 20),
    pairM: settingNumber('Audit.AR.seuilPaireM', 50),
    maxpairs: settingNumber('Audit.AR.maxPaires', 5),
    segM: settingNumber('Audit.AR.branchesMaxM', 200),
    closeM: settingNumber('Audit.RP.seuilFermetureM', 15),
    angleDeg: settingNumber('Audit.RP.angleMinDeg', 270),
  }
}

onMounted(async () => {
  if (!traceId.value) {
    ui.showError('Aucune trace à auditer.')
    router.replace({ name: 'accueil' })
    return
  }

  loading.value = true
  try {
    await settingsStore.loadSettings()
    await auditStore.runAudit(traceId.value, buildParams())
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Erreur d'audit."
    ui.showError(msg)
    router.replace({ name: 'accueil' })
  } finally {
    loading.value = false
  }
})

onBeforeRouteLeave(() => {
  // Décision 9 : avertissement si un travail est en cours, sans boucle infinie
  // (une fois la sortie confirmée, le dialogue est refermé).
  if (auditStore.hasWorkInProgress && !dialogExitOpen.value) {
    dialogExitOpen.value = true
    return false
  }
  auditStore.reset()
  return true
})

function onFindingSelected(findingId: string) {
  // Changer d'anomalie abandonne les aperçus en cours.
  auditStore.clearDeletePreview()
  auditStore.clearRoutePreview()
  auditStore.setRouteRange(null)
  auditStore.selectFinding(findingId)
}

/** Ferme le panneau d'action (désélection) — les aperçus sont abandonnés. */
function onActionPanelClosed() {
  auditStore.clearDeletePreview()
  auditStore.clearRoutePreview()
  auditStore.setRouteRange(null)
  auditStore.selectFinding(null)
}

function onBackClicked() {
  router.push({ name: 'accueil' })
}

function onExitConfirmed() {
  dialogExitOpen.value = false
  auditStore.reset()
  router.push({ name: 'accueil' })
}

function onApplyClicked() {
  if (!auditStore.canApply) return
  dialogApplyOpen.value = true
}

async function onApplyConfirmed() {
  dialogApplyOpen.value = false
  try {
    await auditStore.validateAndRewrite()
    ui.showSuccess('Audit appliqué. Le GPX a été réécrit.')
    auditStore.reset()
    await tracesStore.loadTraces()
    router.push({ name: 'accueil' })
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la validation.'
    ui.showError(msg)
  }
}
</script>

<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <AuditToolbar
        :trace-name="traceName"
        :pending-count="auditStore.pendingCount"
        :corrected-count="auditStore.correctedCount"
        :fp-count="auditStore.fpCount"
        :can-apply="auditStore.canApply"
        @back="onBackClicked"
        @apply="onApplyClicked"
        @open-settings="appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen"
      />

      <AuditFindingsPanel
        :findings="auditStore.findings"
        :selected-id="auditStore.selectedFindingId"
        :total-distance-m="auditStore.totalDistanceM"
        @select="onFindingSelected"
      />

      <!-- Zone centrale : carte d'audit (rendu des anomalies et étiquettes). -->
      <v-main class="audit-main">
        <div class="audit-center">
          <v-progress-circular
            v-if="loading"
            :size="60"
            :width="7"
            color="primary"
            indeterminate
          />
          <AuditMap v-else />

          <AuditActionPanel
            v-if="auditStore.selectedFinding"
            :finding="auditStore.selectedFinding"
            @close="onActionPanelClosed"
          />
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupes Audit.* de la vue active) -->
      <SettingsDrawer :show-system="false" />

      <ConfirmExitDialog
        v-model="dialogExitOpen"
        :work-count="auditStore.correctedCount + auditStore.fpCount"
        @confirm="onExitConfirmed"
        @cancel="dialogExitOpen = false"
      />

      <ConfirmApplyDialog
        v-model="dialogApplyOpen"
        @confirm="onApplyConfirmed"
        @cancel="dialogApplyOpen = false"
      />
    </v-layout>
  </v-app>
</template>

<style scoped>
.audit-center {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
</style>
