<template>
  <v-navigation-drawer permanent width="340" class="py-1">
    <div class="audit-findings">
      <div class="audit-findings-list">
        <v-list v-if="findings.length > 0" density="compact" nav>
          <v-list-item
            v-for="f in findings"
            :key="f.id"
            :active="f.id === selectedId"
            @click="emit('select', f.id)"
          >
            <template #prepend>
              <v-icon
                :icon="f.kind === 'rp' ? 'mdi-rotate-360' : 'mdi-swap-horizontal'"
                :color="kindColor(f)"
              />
            </template>

            <v-list-item-title>{{ f.label }}</v-list-item-title>
            <v-list-item-subtitle>{{ f.summary }}</v-list-item-subtitle>

            <template #append>
              <v-chip :color="statusColor(f)" size="x-small" variant="tonal">
                {{ statusText(f) }}
              </v-chip>
            </template>
          </v-list-item>
        </v-list>

        <v-empty-state
          v-else
          icon="mdi-check-circle"
          title="Aucune anomalie"
          text="La trace est propre."
        />
      </div>

      <v-divider />

      <AuditSynthesis :findings="findings" :total-distance-m="totalDistanceM" />
    </div>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
/**
 * Liste latérale des anomalies détectées, avec leur statut
 * (« À traiter », « Faux positif », « Points supprimés », « Routage… »).
 *
 * La sélection d'une ligne remonte au parent, qui la publie dans le store ;
 * le panneau d'action et la carte s'y synchronisent.
 */
import type { Finding } from '../../stores/audit'
import AuditSynthesis from './AuditSynthesis.vue'

defineProps<{
  findings: Finding[]
  selectedId: string | null
  totalDistanceM: number
}>()

const emit = defineEmits<{
  select: [findingId: string]
}>()

function kindColor(f: Finding): string {
  if (f.status === 'fp') return 'info'
  if (f.status === 'corrected') return 'success'
  return f.kind === 'rp' ? 'warning' : 'error'
}

function statusColor(f: Finding): string {
  if (f.status === 'fp') return 'warning'
  if (f.status === 'corrected') return 'success'
  return 'error'
}

function statusText(f: Finding): string {
  if (f.status === 'pending') return 'À traiter'
  if (f.status === 'fp') return 'Faux positif'
  if (f.correction === 'delete') return 'Points supprimés'
  if (f.correction === 'route-car') return 'Routage voiture'
  if (f.correction === 'route-bike') return 'Routage vélo'
  return 'Corrigé'
}
</script>

<style scoped>
.audit-findings {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.audit-findings-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
</style>
