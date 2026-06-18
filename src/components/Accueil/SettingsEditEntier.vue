<script setup lang="ts">
import { ref, watch } from 'vue'
import { SettingDefinition } from '../../stores/settings'
import { renderMarkdown } from '../../utils/markdown'

const props = defineProps<{
  setting: SettingDefinition
}>()

const emit = defineEmits<{
  (e: 'update', payload: { path: string; value: number }): void
  (e: 'reset', path: string): void
}>()

const currentValue = ref<number>(props.setting.value)

watch(() => props.setting.value, (newVal) => {
  currentValue.value = newVal
})

function onSave() {
  emit('update', { path: props.setting.path, value: currentValue.value })
}

function onReset() {
  if (props.setting.is_overridden) {
    emit('reset', props.setting.path)
  } else {
    currentValue.value = props.setting.default
  }
}
</script>

<template>
  <v-card class="mx-auto" width="600" elevation="2">
    <v-card-item>
      <div class="d-flex align-center justify-space-between w-100">
        <v-card-title class="text-h6 font-weight-bold text-primary">
          {{ setting.description }}
        </v-card-title>
        <v-btn
          v-if="setting.is_overridden || currentValue !== setting.default"
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
      <div class="d-flex align-center mt-2">
        <v-slider
          v-model="currentValue"
          :min="setting.min ?? 1"
          :max="setting.max ?? 100"
          :step="setting.step ?? 1"
          hide-details
          class="align-center mr-4"
          color="primary"
        ></v-slider>

        <v-text-field
          v-model.number="currentValue"
          type="number"
          style="width: 80px"
          density="compact"
          hide-details
          variant="outlined"
          :min="setting.min ?? 1"
          :max="setting.max ?? 100"
        ></v-text-field>
      </div>

      <div class="mt-4 markdown-doc" v-html="renderMarkdown(setting.doc)"></div>
    </v-card-text>

    <v-card-actions class="justify-end">
      <v-btn
        color="primary"
        variant="elevated"
        :disabled="currentValue === props.setting.value"
        @click="onSave"
      >
        Enregistrer
      </v-btn>
    </v-card-actions>
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
