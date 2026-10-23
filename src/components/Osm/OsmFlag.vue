<script setup lang="ts">
import { computed } from 'vue'
import 'flag-icons/css/flag-icons.min.css'

const props = defineProps<{
  code: string | null
  name?: string | null
}>()

const normalized = computed(() => (props.code ?? '').trim().toLowerCase())
const valid = computed(() => /^[a-z]{2}$/.test(normalized.value))
const label = computed(() => props.name ?? undefined)
</script>

<template>
  <span
    v-if="valid"
    :class="['fi', `fi-${normalized}`, 'osm-flag']"
    role="img"
    :aria-label="label"
    :title="label"
  />
  <v-icon
    v-else
    icon="mdi-flag"
    size="18"
    :aria-label="label"
    :title="label"
  />
</template>

<style scoped>
.osm-flag {
  font-size: 18px;
  line-height: 1;
  border-radius: 2px;
  box-shadow: 0 0 0 1px rgba(var(--v-border-color), var(--v-border-opacity));
}
</style>
