<script setup lang="ts">
// ParameterCard : carte générique d'édition d'un paramètre.
// Reçoit une `paramKey` (path) et s'appuie sur le store settings pour la
// définition, la valeur persistée et les drafts locaux.
//
// Structure :
//   1. En-tête (titre, boutons Réinitialiser / Fermer)
//   2. Zone de saisie adaptative au type
//   3. Documentation Markdown
//   4. Barre d'actions (Fermer / Enregistrer)
import { ref, computed, onMounted } from 'vue'
import { useSettingsStore, type SettingDefinition } from '../../stores/settings'
import { renderMarkdown } from '../../utils/markdown'
import InputBool from './InputBool.vue'
import InputInt from './InputInt.vue'
import InputFloat from './InputFloat.vue'
import InputSecret from './InputSecret.vue'
import InputList from './InputList.vue'
import InputRgba from './InputRgba.vue'
import InputMaterialPrimary from './InputMaterialPrimary.vue'
import InputMaterialExtended from './InputMaterialExtended.vue'

const props = defineProps<{
  paramKey: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const store = useSettingsStore()

onMounted(() => {
  store.initDraft(props.paramKey)
})

const def = computed<SettingDefinition | undefined>(() => store.getParamDef(props.paramKey))

// Accès au draft via un computed get/set : le v-model des sous-composants
// reste réactif tout en écrivant dans le draft du store.
const draftValue = computed<any>({
  get: () => store.getDraft(props.paramKey),
  set: (v: any) => {
    store.drafts[props.paramKey] = v
  },
})

const isModified = computed(() => store.isModified(props.paramKey))
const isDefault = computed(() => store.isDefault(props.paramKey))
const isCritical = computed(() => !!def.value?.critical)

// Affichage des boutons
const showReset = computed(() => !isDefault.value)
const closeBtnColor = computed(() => (isModified.value ? 'warning' : undefined))

// --- Dialogues de confirmation -------------------------------------------
const showResetDialog = ref(false) // réinitialisation d'un paramètre critique
const showCloseConfirmDialog = ref(false) // fermeture avec pertes

// --- Actions -------------------------------------------------------------

function onResetClick() {
  // Pour un paramètre critique, on exige une confirmation explicite.
  if (isCritical.value) {
    showResetDialog.value = true
  } else {
    void doReset()
  }
}

async function doReset() {
  showResetDialog.value = false
  await store.resetParam(props.paramKey)
}

function onCloseClick() {
  if (isModified.value) {
    showCloseConfirmDialog.value = true
  } else {
    emit('close')
  }
}

function confirmCloseWithLoss() {
  showCloseConfirmDialog.value = false
  store.closeWithoutSaving(props.paramKey)
  emit('close')
}

async function onSave() {
  await store.saveParam(props.paramKey)
}

const docHtml = computed(() => renderMarkdown(def.value?.documentation ?? ''))
</script>

<template>
  <v-card
    v-if="def"
    class="mx-auto parameter-card"
    :class="{ 'parameter-card--critical': isCritical }"
    width="600"
    elevation="2"
  >
    <!-- 1. En-tête -->
    <v-card-item>
      <div class="d-flex align-center justify-space-between w-100">
        <v-card-title class="text-h6 font-weight-bold text-primary d-flex align-center pa-0">
          <span>{{ def.description }}</span>
          <v-tooltip v-if="isCritical" location="top">
            <template #activator="{ props: tooltipProps }">
              <v-icon
                v-bind="tooltipProps"
                color="warning"
                size="small"
                class="ml-2"
              >
                mdi-alert-circle
              </v-icon>
            </template>
            Critique
          </v-tooltip>
        </v-card-title>
        <v-spacer></v-spacer>
        <v-btn
          v-if="showReset"
          icon="mdi-undo"
          variant="text"
          color="warning"
          density="comfortable"
          title="Restaurer la valeur par défaut"
          @click="onResetClick"
        ></v-btn>
        <v-btn
          icon="mdi-close"
          variant="text"
          density="comfortable"
          title="Fermer"
          @click="onCloseClick"
        ></v-btn>
      </div>
    </v-card-item>

    <!-- 2. Zone de saisie adaptative -->
    <v-card-text>
      <InputBool v-if="def.type === 'bool'" v-model="draftValue" :def="def" />
      <InputInt v-else-if="def.type === 'int'" v-model.number="draftValue" :def="def" />
      <InputFloat v-else-if="def.type === 'float'" v-model.number="draftValue" :def="def" />
      <InputSecret v-else-if="def.type === 'secret'" v-model="draftValue" :def="def" />
      <InputList v-else-if="def.type === 'list'" v-model="draftValue" :def="def" />
      <InputRgba v-else-if="def.type === 'rgba'" v-model="draftValue" :def="def" />
      <InputMaterialPrimary
        v-else-if="def.type === 'material_primary'"
        v-model="draftValue"
        :def="def"
      />
      <InputMaterialExtended
        v-else-if="def.type === 'material_extended'"
        v-model="draftValue"
        :def="def"
      />
      <div v-else class="text-error">
        Type non pris en charge : <code>{{ def.type }}</code>
      </div>

      <!-- 3. Documentation -->
      <div class="mt-4 markdown-doc" v-html="docHtml"></div>
    </v-card-text>

    <!-- 4. Barre d'actions -->
    <v-card-actions class="justify-end">
      <v-btn :color="closeBtnColor" variant="text" @click="onCloseClick">
        Fermer
      </v-btn>
      <v-btn
        v-if="isModified"
        color="primary"
        variant="elevated"
        @click="onSave"
      >
        Enregistrer
      </v-btn>
    </v-card-actions>

    <!-- Dialogue : réinitialisation d'un paramètre critique -->
    <v-dialog v-model="showResetDialog" max-width="420">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert-decagram</v-icon>
          Réinitialisation critique
        </v-card-title>
        <v-card-text>
          Ce paramètre est identifié comme <strong>critique</strong>.
          Voulez-vous vraiment rétablir sa valeur par défaut ?
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showResetDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="doReset">Réinitialiser</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Dialogue : fermeture avec modifications non enregistrées -->
    <v-dialog v-model="showCloseConfirmDialog" max-width="420">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert</v-icon>
          Modifications non enregistrées
        </v-card-title>
        <v-card-text>
          Des modifications non enregistrées seront perdues.
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showCloseConfirmDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="confirmCloseWithLoss">
            Fermer sans enregistrer
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>

  <v-card v-else class="mx-auto" width="600">
    <v-card-text class="text-error">Paramètre introuvable : {{ paramKey }}</v-card-text>
  </v-card>
</template>

<style scoped>
.parameter-card--critical {
  border: 2px solid rgb(var(--v-theme-warning));
}
.markdown-doc {
  font-size: 0.9rem;
  color: rgba(var(--v-theme-on-surface), 0.7);
  line-height: 1.4;
  padding: 8px 12px;
  background-color: rgba(var(--v-theme-on-surface), 0.03);
  border-radius: 4px;
  border-left: 3px solid rgba(var(--v-theme-primary), 0.6);
}
.markdown-doc :deep(strong) {
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.9);
}
.markdown-doc :deep(code) {
  background-color: rgba(var(--v-theme-on-surface), 0.1);
  padding: 2px 4px;
  border-radius: 3px;
  font-family: monospace;
}
.markdown-doc :deep(ul) {
  margin-left: 20px;
  margin-top: 4px;
  margin-bottom: 4px;
}
.markdown-doc :deep(a) {
  color: rgb(var(--v-theme-primary));
  text-decoration: none;
}
.markdown-doc :deep(a:hover) {
  text-decoration: underline;
}
</style>
