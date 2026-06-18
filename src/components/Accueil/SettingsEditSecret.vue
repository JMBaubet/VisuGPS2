<script setup lang="ts">
import { ref, watch } from 'vue'
import { SettingDefinition } from '../../stores/settings'
import { renderMarkdown } from '../../utils/markdown'

const props = defineProps<{
  setting: SettingDefinition
}>()

const emit = defineEmits<{
  (e: 'update', payload: { path: string; value: string }): void
  (e: 'reset', path: string): void
}>()

const showPassword = ref(false)
const inputSecret = ref('')
const hasValue = ref(props.setting.value === '********')
const showConfirmDialog = ref(false)

watch(() => props.setting.value, (newVal) => {
  hasValue.value = newVal === '********'
  inputSecret.value = ''
})

function handleSave() {
  if (props.setting.critique) {
    showConfirmDialog.value = true
  } else {
    confirmSave()
  }
}

function confirmSave() {
  showConfirmDialog.value = false
  emit('update', { path: props.setting.path, value: inputSecret.value })
  inputSecret.value = ''
}

function onReset() {
  if (props.setting.is_overridden) {
    emit('reset', props.setting.path)
  } else {
    inputSecret.value = ''
  }
}
</script>

<template>
  <v-card class="mx-auto" width="600" elevation="2">
    <v-card-item>
      <div class="d-flex align-center justify-space-between w-100">
        <v-card-title class="text-h6 font-weight-bold text-primary d-flex align-center">
          <span>{{ setting.description }}</span>
          <v-chip
            v-if="setting.critique"
            color="error"
            size="x-small"
            class="ml-2"
            variant="flat"
          >
            Critique
          </v-chip>
        </v-card-title>
        <v-btn
          v-if="setting.is_overridden || inputSecret !== ''"
          icon="mdi-undo"
          variant="text"
          color="warning"
          density="comfortable"
          title="Restaurer la valeur par défaut"
          @click="onReset"
        ></v-btn>
      </div>
    </v-card-item>

    <v-card-text>
      <v-text-field
        v-model="inputSecret"
        :type="showPassword ? 'text' : 'password'"
        :label="hasValue ? 'Secret configuré (Saisissez une nouvelle valeur pour écraser)' : 'Saisissez le secret'"
        :placeholder="hasValue ? '********' : ''"
        variant="outlined"
        density="comfortable"
        :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
        @click:append-inner="showPassword = !showPassword"
        class="mt-2"
        hide-details
      ></v-text-field>

      <div class="mt-4 markdown-doc" v-html="renderMarkdown(setting.doc)"></div>
    </v-card-text>

    <v-card-actions class="justify-end">
      <v-btn
        color="primary"
        variant="elevated"
        :disabled="!inputSecret"
        @click="handleSave"
      >
        Enregistrer
      </v-btn>
    </v-card-actions>

    <!-- Dialogue de confirmation pour paramètre critique -->
    <v-dialog v-model="showConfirmDialog" max-width="400">
      <v-card>
        <v-card-title class="text-h6 text-warning font-weight-bold d-flex align-center">
          <v-icon color="warning" class="mr-2">mdi-alert-decagram</v-icon>
          Confirmation requise
        </v-card-title>
        <v-card-text>
          Ce paramètre est identifié comme <strong>critique</strong> pour la stabilité du système. 
          Voulez-vous vraiment appliquer cette modification ?
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" color="grey" @click="showConfirmDialog = false">Annuler</v-btn>
          <v-btn variant="flat" color="warning" @click="confirmSave">Confirmer</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<style scoped>
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
