<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import { useAppStore } from '../../stores/app'
import { useOsmStore } from '../../stores/osm'
import { describeOsmOverview } from '../../utils/osmFiles'

const appStore = useAppStore()
const osmStore = useOsmStore()

const display = computed(() =>
  describeOsmOverview(osmStore.overview, osmStore.loading, osmStore.lastError),
)

let refreshTimer: number | null = null

function refresh(): void {
  if (osmStore.loading) return
  void osmStore.refreshOverview()
}

function onClick(): void {
  if (display.value.action === 'settings') {
    appStore.isSettingsDrawerOpen = true
    return
  }
  osmStore.openDialog()
}

onMounted(() => {
  refresh()
  void osmStore.bindProgress()
  refreshTimer = window.setInterval(refresh, 30_000)
  window.addEventListener('focus', refresh)
})

onUnmounted(() => {
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer)
    refreshTimer = null
  }
  window.removeEventListener('focus', refresh)
})

watch(
  () => appStore.isSettingsDrawerOpen,
  (open, previous) => {
    if (previous && !open) refresh()
  },
)
</script>

<template>
  <v-tooltip location="bottom end" :open-delay="300" max-width="380">
    <template #activator="{ props }">
      <v-btn
        v-bind="props"
        :icon="display.icon"
        :color="display.color"
        variant="text"
        density="comfortable"
        slim
        :aria-label="display.tooltip.join(' ')"
        @click="onClick"
      />
    </template>
    <div class="osm-status-tooltip">
      <div v-for="(line, index) in display.tooltip" :key="index">
        {{ line }}
      </div>
    </div>
  </v-tooltip>
</template>

<style scoped>
.osm-status-tooltip {
  font-size: 0.8rem;
  line-height: 1.4;
}
</style>
