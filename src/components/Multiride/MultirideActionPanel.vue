<template>
  <v-card class="mrl-action-panel" variant="elevated" elevation="8">
    <v-card-title class="d-flex justify-space-between align-center text-body-2">
      <span>{{ title }}</span>
      <v-btn
        icon="mdi-close"
        size="small"
        variant="text"
        title="Fermer"
        @click="emit('close')"
      />
    </v-card-title>

    <v-card-actions class="flex-column align-stretch">
      <!-- Le titre est porté par l'enveloppe et non par le bouton : un bouton
           désactivé ne reçoit pas le survol, son info-bulle ne s'afficherait
           donc jamais — or c'est elle qui dit pourquoi l'action est refusée. -->
      <div :title="approveNote">
        <v-btn
          block
          color="success"
          prepend-icon="mdi-check"
          :disabled="approveBlocked"
          @click="approve()"
        >
          {{ approved ? 'Segment approuvé' : 'Valider le segment' }}
        </v-btn>
      </div>

      <div :title="mergeNote">
        <v-btn
          block
          color="primary"
          prepend-icon="mdi-arrow-collapse-up"
          :disabled="mergeBlocked"
          @click="merge()"
        >
          {{ mergeLabel }}
        </v-btn>
      </div>

      <div :title="fpNote">
        <v-btn
          block
          color="info"
          :prepend-icon="falsePositive ? 'mdi-close-circle-outline' : 'mdi-cancel'"
          :disabled="fpBlocked"
          @click="toggleFp()"
        >
          {{ falsePositive ? 'Retirer le faux positif' : 'Faux positif' }}
        </v-btn>
      </div>

      <!-- L'annulation n'est offerte que si elle est possible : un segment
           fusionné par une version antérieure n'a pas d'enregistrement, et la
           commande le refuserait. -->
      <v-btn
        v-if="undoable"
        block
        color="warning"
        prepend-icon="mdi-undo"
        @click="undo()"
      >
        {{ undoLabel }}
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Fenêtre d'action d'un segment (mêmes vues et mêmes gestes que la vue Audit,
 * où le clic sur une anomalie ouvre `AuditActionPanel`).
 *
 * Elle porte les trois gestes du module, dans l'ordre de fréquence :
 * **approuver** le segment dans son état courant, le **fusionner** avec le
 * précédent, le marquer **faux positif** — puis **annuler** le geste en cours.
 *
 * Deux ordres de gestes, et deux règles : un **verdict** (approuvé, écarté)
 * porte sur le segment tel qu'il est, et les deux verdicts s'excluent ; une
 * **fusion**, elle, réorganise la détection sans la juger — un segment fusionné
 * s'approuve donc comme un autre, mais ne peut plus être écarté. Les gestes
 * impossibles sont grisés, leur info-bulle en donne le motif : premier segment,
 * faux positif d'un des deux camps, segment déjà fusionné pour l'écartement, ou
 * déjà approuvé. L'annulation retire d'abord le verdict, puis la fusion.
 *
 * Comme son homologue de l'audit, la fenêtre est **branchée au store** : elle
 * lit le segment sélectionné et agit elle-même, le parent n'ayant qu'à la
 * monter et à refermer. Les gestes restent ouverts après coup — un verdict se
 * défait, et la fenêtre propose justement de le défaire.
 */
import { computed } from 'vue'
import { useMultirideStore } from '../../stores/multiride'
import { useUiStore } from '../../stores/ui'
import { formatSegmentTitle } from './multirideFormat'

const props = defineProps<{
  /** Numéro du segment géré, tel que publié par la liste ou la carte. */
  segment: number
}>()

const emit = defineEmits<{
  close: []
}>()

const multirideStore = useMultirideStore()
const ui = useUiStore()

/** Emprunts du segment, du plus ancien au plus récent. */
const passages = computed(() => multirideStore.segmentPassages(props.segment))

/** Emprunts de son précédent — le camp que la fusion viendrait rejoindre. */
const previousPassages = computed(() =>
  multirideStore.previousSegmentPassages(props.segment),
)

/** En-tête : la longueur et le début de la référence du segment. */
const title = computed(() =>
  formatSegmentTitle(props.segment, multirideStore.referenceOf(props.segment)),
)

/** Le segment est écarté : marqué faux positif par l'utilisateur. */
const falsePositive = computed(() => passages.value.some((p) => p.fauxPositif))

/** Le segment a été réuni à son précédent par une fusion manuelle. */
const merged = computed(() => passages.value.some((p) => p.fusionne))

/** Le segment est approuvé tel quel : un vrai passage multiple. */
const approved = computed(() => passages.value.some((p) => p.valide))

/** Le précédent est écarté : il ne peut rien absorber. */
const previousFalsePositive = computed(() =>
  previousPassages.value.some((p) => p.fauxPositif),
)

/**
 * Approbation refusée sur un segment écarté — un segment exclu de l'export ne
 * s'approuve pas —, et sans objet sur un segment déjà approuvé.
 *
 * Un segment **fusionné** s'approuve, en revanche : la fusion réorganise la
 * détection, elle ne dit rien de sa justesse, et le segment fusionné est un
 * segment neuf, à juger comme les autres.
 */
const approveBlocked = computed(() => approved.value || falsePositive.value)

/** Motif du refus — ou ce que le geste affirme, quand il est possible. */
const approveNote = computed(() => {
  if (approved.value) {
    return "Ce segment est déjà approuvé — « Annuler l'approbation » revient dessus"
  }
  if (falsePositive.value) {
    return 'Ce segment est un faux positif : un segment écarté ne peut pas être approuvé'
  }
  if (merged.value) {
    return 'Approuver le résultat de la fusion : la fusion réorganise la détection, elle ne juge pas sa justesse'
  }
  return 'Un vrai passage multiple, rien à changer : le segment est approuvé et compte comme examiné'
})

/** Fusion refusée : premier segment, ou faux positif d'un des deux camps. */
const mergeBlocked = computed(
  () => props.segment < 2 || previousFalsePositive.value || falsePositive.value,
)

/** Motif du refus — ou ce que le geste produit, quand il est possible. */
const mergeNote = computed(() => {
  if (props.segment < 2) {
    return "Le segment 1 n'a pas de précédent : il ne peut pas être fusionné."
  }
  if (previousFalsePositive.value) {
    return `Le segment ${props.segment - 1} est un faux positif : un faux positif ne peut pas être fusionné.`
  }
  if (falsePositive.value) {
    return 'Ce segment est un faux positif : un faux positif ne peut pas être fusionné.'
  }
  return `Réunir le segment ${props.segment} au précédent : les emprunts sont recousus deux à deux quand ils vont dans le même sens et qu'un kilomètre au plus les sépare`
})

/** Libellé du bouton de fusion, juste même quand il n'y a pas de précédent. */
const mergeLabel = computed(() =>
  props.segment > 1
    ? `Fusionner avec S${props.segment - 1}`
    : 'Fusionner avec le précédent',
)

/** Marquage refusé sur un segment fusionné : un segment, un ajustement. */
const fpBlocked = computed(() => merged.value)

const fpNote = computed(() =>
  merged.value
    ? 'Ce segment a été fusionné : annulez la fusion pour le marquer faux positif.'
    : 'Détecté à tort : exclure ce segment du résultat et des kilomètres répétés',
)

/**
 * Un état reste à défaire : une approbation, le marqueur d'un segment écarté,
 * ou l'enregistrement des emprunts d'avant une fusion.
 */
const undoable = computed(
  () =>
    approved.value ||
    falsePositive.value ||
    passages.value.some((p) => p.avantFusion != null),
)

const undoLabel = computed(() => {
  if (approved.value) return "Annuler l'approbation"
  return falsePositive.value ? 'Retirer le faux positif' : 'Annuler la fusion'
})

/** Approuve le segment — un vrai passage multiple, rien à changer. */
async function approve(): Promise<void> {
  try {
    await multirideStore.validateSegment(props.segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Échec de l'approbation."
    ui.showError(msg)
  }
}

/** Réunit le segment à son précédent — le store suit la fusion. */
async function merge(): Promise<void> {
  try {
    await multirideStore.mergeSegment(props.segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec de la fusion.'
    ui.showError(msg)
  }
}

/** Marque ou démarque le segment en faux positif. */
async function toggleFp(): Promise<void> {
  try {
    await multirideStore.toggleFp(props.segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : 'Échec du marquage.'
    ui.showError(msg)
  }
}

/** Défait l'ajustement du segment : marqueur retiré, ou fusion annulée. */
async function undo(): Promise<void> {
  try {
    await multirideStore.undoSegment(props.segment)
  } catch (error) {
    const msg = typeof error === 'string' ? error : "Échec de l'annulation."
    ui.showError(msg)
  }
}
</script>

<style scoped>
.mrl-action-panel {
  position: absolute;
  top: 16px;
  left: 16px;
  width: 320px;
  z-index: 10;
}
</style>
