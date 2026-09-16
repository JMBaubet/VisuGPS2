<template>
  <v-app :theme="appStore.theme" class="h-screen w-screen">
    <v-layout>
      <MultirideToolbar
        :trace-name="traceName"
        :status="multirideStore.status"
        :segment-count="multirideStore.segmentCount"
        :loading="multirideStore.loading || analyzing"
        :validated-at="archive?.updatedAt ?? null"
        :dirty="dirty"
        @back="onBackClicked"
        @analyze="onAnalyzeClicked"
        @validate="onValidateClicked"
        @edit="goToEdition"
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
        :has-adjustments="multirideStore.hasAdjustments"
        @select="multirideStore.selectSegment"
        @merge="onMergeClicked"
        @toggle-fp="onToggleFpClicked"
        @reset="onResetClicked"
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
 * panneau la met en avant et la carte cadre son étendue.
 *
 * Le `traceId` arrive par la query de la route (`/multiride?traceId=…`), posée
 * par le bouton Éditer de l'accueil ou par le garde-fou d'EditionCamera. À la
 * sortie, le store est réinitialisé — le fichier de description, lui, survit.
 *
 * Les ajustements (fusion, faux positif) et la validation sont portés par la
 * sous-étape suivante : cette vue expose la détection et sa restitution.
 */
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useMultirideStore, type MultirideParams } from '../stores/multiride'
import { useEditionStore } from '../stores/edition'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore, type TracePoint } from '../stores/traces'
import { useUiStore } from '../stores/ui'
import MultirideToolbar from '../components/Multiride/MultirideToolbar.vue'
import MultirideMap from '../components/Multiride/MultirideMap.vue'
import MultirideSegmentsPanel from '../components/Multiride/MultirideSegmentsPanel.vue'
import ConfirmExitDialog from '../components/Multiride/dialogs/ConfirmExitDialog.vue'
import SettingsDrawer from '../components/Accueil/SettingsDrawer.vue'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const multirideStore = useMultirideStore()
const editionStore = useEditionStore()
const settingsStore = useSettingsStore()
const tracesStore = useTracesStore()
const ui = useUiStore()

/** Une détection est en cours de lancement depuis cette vue. */
const analyzing = ref(false)
/** Confirmation de sortie lorsque la barrière est encore levée. */
const dialogExitOpen = ref(false)
/** Points de la trace, pour le tracé de fond et le découpage des emprunts. */
const tracePoints = ref<TracePoint[]>([])

const traceId = computed(() => (route.query.traceId as string | null) ?? null)
const traceName = computed(
  () => tracesStore.traces.find((t) => t.id === traceId.value)?.name ?? 'Trace',
)

/** L'état de la détection courante (`null` avant la première restitution). */
const archive = computed(() => multirideStore.archive)

/**
 * Les paramètres de détection ont changé depuis la détection affichée : une
 * relance produirait un autre résultat, et perdrait les ajustements en cours.
 * C'est ce que signale la pastille de la barre — la relance reste manuelle.
 */
const dirty = computed(() => {
  const applied = archive.value?.params
  if (!applied) return false
  const current = buildParams()
  return (
    current.toleranceM !== applied.toleranceM ||
    current.longueurMinM !== applied.longueurMinM ||
    current.pasEchantillonnageM !== applied.pasEchantillonnageM ||
    current.fusionReferencesM !== applied.fusionReferencesM
  )
})

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

/** Charge les points de la trace — support du tracé et du découpage des emprunts. */
async function loadTracePoints(): Promise<void> {
  if (!traceId.value) return
  tracePoints.value = await tracesStore.getTracePoints(traceId.value)
}

/** Lance la détection sur la trace courante, puis recharge ses points. */
async function runDetection(): Promise<void> {
  if (!traceId.value) return
  await multirideStore.runDetection(traceId.value, buildParams())
  await loadTracePoints()
}

/** Fusionne un segment avec le précédent, puis recharge les points si besoin. */
async function onMergeClicked(segment: number): Promise<void> {
  try {
    await multirideStore.mergeSegment(segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la fusion.'
    ui.showError(msg)
  }
}

/** Marque ou démarque un segment en faux positif. */
async function onToggleFpClicked(segment: number): Promise<void> {
  try {
    await multirideStore.toggleFp(segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec du marquage.'
    ui.showError(msg)
  }
}

/** Rétablit la détection d'origine (la détection est rejouée). */
async function onResetClicked(): Promise<void> {
  try {
    await multirideStore.resetAdjustments()
    ui.showSuccess('Détection rétablie.')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la réinitialisation.'
    ui.showError(msg)
  }
}

/**
 * Valide les passages détectés, puis poursuit vers l'édition caméra : c'est
 * l'intention de l'utilisateur qui a ouvert cette vue pour éditer.
 */
async function onValidateClicked(): Promise<void> {
  try {
    await multirideStore.validate()
    await tracesStore.loadTraces()
    ui.showSuccess('Passages multiples validés.')
    await goToEdition()
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la validation.'
    ui.showError(msg)
  }
}

/** Poursuit vers l'édition caméra — la barrière est levée ou n'a pas lieu d'être. */
async function goToEdition(): Promise<void> {
  if (!traceId.value) return
  editionStore.selectTrace(traceId.value)
  await router.push({ name: 'editionCamera' })
}

/** Relance la détection à la demande (paramètres `Multiride.*` courants). */
async function onAnalyzeClicked(): Promise<void> {
  analyzing.value = true
  try {
    await runDetection()
    ui.showSuccess('Détection des passages multiples terminée.')
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la détection.'
    ui.showError(msg)
  } finally {
    analyzing.value = false
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
    if (!restored) await runDetection()
    else await loadTracePoints()
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
