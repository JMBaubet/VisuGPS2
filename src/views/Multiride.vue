<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <MultirideToolbar
        :trace-name="traceName"
        :status="multirideStore.status"
        :segment-count="multirideStore.segmentCount"
        :loading="multirideStore.loading"
        :validated-at="archive?.updatedAt ?? null"
        :pending-count="multirideStore.pendingSegmentCount"
        :treated-count="multirideStore.treatedSegmentCount"
        :fp-count="multirideStore.falsePositiveSegmentCount"
        :adjustments-locked="multirideStore.hasAdjustments"
        @back="onBackClicked"
        @validate="onValidateClicked"
        @open-settings="appStore.isSettingsDrawerOpen = !appStore.isSettingsDrawerOpen"
      />

      <MultirideSegmentsPanel
        :passages="multirideStore.passages"
        :segment-numbers="multirideStore.segmentNumbers"
        :selected-segment="multirideStore.selectedSegment"
        :trace-length-km="archive?.traceLengthKm ?? 0"
        :repeated-km="multirideStore.repeatedKm"
        :analysis-duration-ms="multirideStore.analysisDurationMs"
        :params="archive?.params ?? null"
        @select="multirideStore.selectSegment"
      />

      <!-- Zone centrale : la carte de restitution. Elle n'est montée qu'une fois
           la trace chargée — le découpage des emprunts a besoin de ses points. -->
      <v-main class="multiride-main">
        <div class="multiride-center">
          <MultirideMap
            v-if="tracePoints.length >= 2"
            :passages="multirideStore.passages"
            :trace-points="tracePoints"
            :selected-segment="multirideStore.selectedSegment"
            @select-segment="multirideStore.selectSegment"
          />
          <v-progress-circular
            v-else
            :size="60"
            :width="7"
            color="primary"
            indeterminate
          />

          <!-- Fenêtre d'action du segment sélectionné : les gestes du module,
               comme le panneau d'action de la vue Audit. -->
          <MultirideActionPanel
            v-if="multirideStore.selectedSegment !== null"
            :segment="multirideStore.selectedSegment"
            @close="multirideStore.selectSegment(null)"
          />
        </div>
      </v-main>

      <!-- Panneau Paramètres (groupes Multiride.* de la vue active) -->
      <SettingsDrawer :show-system="false" />

      <ConfirmExitDialog
        v-model="dialogExitOpen"
        :segment-count="multirideStore.segmentCount"
        @confirm="onExitConfirmed"
        @cancel="dialogExitOpen = false"
      />
    </v-layout>
  </v-app>
</template>

<script setup lang="ts">
/**
 * Vue de détection des passages multiples (route `/multiride`, plein écran
 * style Accueil/Audit).
 *
 * Une trace valide peut contenir des portions parcourues plusieurs fois :
 * aller-retour sur un tronçon, reconnaissance repassant sur une section, boucle
 * locale. Cette vue les détecte — ou restitue une détection déjà écrite — et
 * les fait valider : **tant qu'un passage reste à valider, l'édition caméra est
 * inaccessible** (`multiride_status = "pending"`), au même titre qu'une trace
 * non auditée. La détection suit donc l'audit et précède l'édition caméra.
 *
 * La restitution est répartie entre le panneau latéral (synthèse, ruban,
 * emprunts) et la carte, qui porte la trace et les portions répétées. Les deux
 * sont synchronisés par le store : la sélection d'un segment y est publiée, le
 * panneau la met en avant et la carte cadre son étendue. Le clic sur un segment
 * — dans la liste ou sur la carte — ouvre en outre la **fenêtre d'action** du
 * segment, où se prennent les trois gestes du module : fusion, faux positif,
 * annulation de l'ajustement en cours.
 *
 * Le `traceId` arrive par la query de la route (`/multiride?traceId=…`), posée
 * par le bouton Éditer de l'accueil ou par le garde-fou d'EditionCamera. À la
 * sortie, le store est réinitialisé — le fichier de description, lui, survit.
 */
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useMultirideStore, type MultirideParams } from '../stores/multiride'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore, type TracePoint } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import MultirideToolbar from '../components/Multiride/MultirideToolbar.vue'
import MultirideMap from '../components/Multiride/MultirideMap.vue'
import MultirideSegmentsPanel from '../components/Multiride/MultirideSegmentsPanel.vue'
import MultirideActionPanel from '../components/Multiride/MultirideActionPanel.vue'
import ConfirmExitDialog from '../components/Multiride/dialogs/ConfirmExitDialog.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const multirideStore = useMultirideStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

/** Confirmation de sortie lorsque la barrière est encore levée. */
const dialogExitOpen = ref(false)
/** Points de la trace, pour le tracé de fond et le découpage des emprunts. */
const tracePoints = ref<TracePoint[]>([])
/**
 * Paramètres ayant produit la détection affichée, en une chaîne comparable.
 * `null` tant qu'aucune détection n'est chargée : c'est ce qui distingue
 * l'ouverture de la vue — où les réglages arrivent après le premier rendu —
 * d'un enregistrement de paramètre.
 */
const appliedParamsKey = ref<string | null>(null)

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find((t) => t.id === traceId.value)?.name ?? 'Trace',
)

/** L'état de la détection courante (`null` avant la première restitution). */
const archive = computed(() => multirideStore.archive)

/** Paramètres réglés dans le drawer, en une chaîne comparable à l'appliquée. */
const settingsParamsKey = computed(() => paramsKeyOf(buildParams()))

/** Valeur d'un paramètre numérique, avec repli sur la valeur par défaut. */
function settingNumber(path: string, fallback: number): number {
  const def = settingsStore.settings.find((s) => s.path === path)
  const value = def?.value
  return typeof value === 'number' ? value : fallback
}

/** Assemble les paramètres du détecteur depuis les réglages `Multiride.*`. */
function buildParams(): MultirideParams {
  return {
    toleranceM: settingNumber('Multiride.Detection.tolerance', 10),
    longueurMinM: settingNumber('Multiride.Detection.longueurMin', 100),
    pasEchantillonnageM: settingNumber('Multiride.Detection.pasEchantillonnage', 4),
    fusionReferencesM: settingNumber('Multiride.Detection.fusionReferences', 100),
  }
}

/**
 * Les quatre paramètres en une chaîne comparable — les valeurs seules, dans
 * l'ordre de [`MultirideParams`] : deux jeux de paramètres sont identiques
 * quand leurs chaînes le sont.
 */
function paramsKeyOf(params: MultirideParams | null | undefined): string | null {
  if (!params) return null
  return [
    params.toleranceM,
    params.longueurMinM,
    params.pasEchantillonnageM,
    params.fusionReferencesM,
  ].join('|')
}

/** Charge les points de la trace — support du tracé et du découpage des emprunts. */
async function loadTracePoints(): Promise<void> {
  if (!traceId.value) return
  tracePoints.value = await tracesStore.getTracePoints(traceId.value)
}

/**
 * Lance la détection sur la trace courante, puis recharge ses points.
 *
 * Les paramètres appliqués sont relevés sur l'état produit — et non sur les
 * réglages : c'est ce que la détection a réellement joué qui fait référence.
 */
async function runDetection(): Promise<void> {
  if (!traceId.value) return
  await multirideStore.runDetection(traceId.value, buildParams())
  appliedParamsKey.value = paramsKeyOf(multirideStore.archive?.params)
  await loadTracePoints()
}

/**
 * Rejoue la détection avec les paramètres qui viennent d'être enregistrés.
 *
 * C'est la **seule** relance : le bouton dédié a disparu. Une relance écrase la
 * détection précédente et ses ajustements, raison pour laquelle les paramètres
 * ne sont pas modifiables tant qu'un ajustement existe — le drawer est fermé et
 * son bouton grisé, l'utilisateur ne peut donc pas déclencher ce cas.
 */
async function onParametersSaved(): Promise<void> {
  try {
    await runDetection()
    ui.showSuccess('Paramètres enregistrés : détection relancée.')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Détection impossible.'
    ui.showError(`${msg} Modifiez un paramètre pour réessayer.`)
  }
}

/**
 * Valide les passages détectés, puis revient à l'accueil : l'édition caméra
 * s'ouvre depuis la carte du circuit, plus depuis cette vue.
 */
async function onValidateClicked(): Promise<void> {
  try {
    await multirideStore.validate()
    await tracesStore.loadTraces()
    ui.showSuccess('Passages multiples validés.')
    // Le garde-fou de sortie ne s'y oppose plus : la barrière est levée.
    await router.push({ name: 'accueil' })
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la validation.'
    ui.showError(msg)
  }
}

function onBackClicked() {
  // Quitter sans valider laisse l'édition caméra inaccessible : on le dit.
  if (multirideStore.needsValidation) {
    dialogExitOpen.value = true
    return
  }
  router.push({ name: 'accueil' })
}

function onExitConfirmed() {
  dialogExitOpen.value = false
  multirideStore.reset()
  router.push({ name: 'accueil' })
}

onMounted(async () => {
  // Garde-fou : sans trace en query, la vue n'a rien à analyser.
  if (!traceId.value) {
    ui.showError('Aucune trace à analyser.')
    router.replace({ name: 'accueil' })
    return
  }

  try {
    await settingsStore.loadSettings()
    await tracesStore.loadTraces()
    // La détection déjà écrite fait foi — ajustements compris ; elle n'est
    // rejouée que si le fichier est absent ou inexploitable.
    const restored = await multirideStore.restore(traceId.value)
    if (!restored) {
      await runDetection()
    } else {
      // Une détection restituée n'est pas relancée, même si ses paramètres
      // diffèrent des réglages courants : ouvrir la vue ne doit pas écraser
      // des ajustements. Ce sont les paramètres de la détection qui font
      // référence jusqu'au prochain enregistrement.
      appliedParamsKey.value = paramsKeyOf(multirideStore.archive?.params)
      await loadTracePoints()
    }
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Erreur de détection.'
    ui.showError(msg)
    router.replace({ name: 'accueil' })
  }
})

onBeforeRouteLeave(() => {
  // Le fichier de description porte l'état : quitter la vue ne perd rien, une
  // reprise restitue la détection et ses ajustements. Seule une sortie avec la
  // barrière encore levée demande confirmation — sans boucle infinie, une fois
  // la sortie confirmée le dialogue est refermé.
  if (multirideStore.needsValidation && !dialogExitOpen.value) {
    dialogExitOpen.value = true
    return false
  }
  multirideStore.reset()
  return true
})

/**
 * Relance de la détection à l'enregistrement d'un paramètre.
 *
 * Le déclencheur est l'**enregistrement**, et non la frappe : les curseurs du
 * drawer n'alimentent qu'un brouillon local, rien n'est persisté avant le
 * bouton d'enregistrement. Comparer les valeurs suffit donc, sans temporisation
 * — le rechargement des réglages qui suit un enregistrement ne déclenche rien
 * tant que les valeurs sont celles appliquées.
 *
 * Deux garde-fous. Une détection déjà en cours, pour ne pas empiler deux
 * analyses. Et un ajustement en place : une relance l'écraserait — théoriquement
 * superflu, le bouton des paramètres étant grisé dès qu'un ajustement existe,
 * mais il protège d'une modification venue d'ailleurs.
 */
watch(settingsParamsKey, (key) => {
  // Aucune détection de référence : c'est l'ouverture de la vue, pas un
  // enregistrement — les réglages y arrivent après le premier rendu.
  if (appliedParamsKey.value === null) return
  if (key === appliedParamsKey.value) return
  if (multirideStore.loading || multirideStore.hasAdjustments) return
  void onParametersSaved()
})

/**
 * Les paramètres cessent d'être modifiables dès qu'une détection est en cours
 * ou qu'un ajustement est en place — les deux conditions qui grisent le bouton
 * de la barre. Le drawer se referme donc s'il était ouvert : sans cela, un
 * second enregistrement pendant l'analyse serait ignoré, la détection en cours
 * portant les paramètres qu'elle a reçus, et l'affichage ne correspondrait plus
 * aux réglages enregistrés.
 */
watch(
  () => multirideStore.loading || multirideStore.hasAdjustments,
  (locked) => {
    if (locked) appStore.isSettingsDrawerOpen = false
  },
)
</script>

<style scoped>
.multiride-center {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}
</style>
