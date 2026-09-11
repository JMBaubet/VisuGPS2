<template>
  <v-expansion-panels variant="accordion" class="ma-2">
    <v-expansion-panel>
      <v-expansion-panel-title>
        <v-icon icon="mdi-chart-box" class="mr-2" />
        Synthèse
      </v-expansion-panel-title>
      <v-expansion-panel-text>
        <v-list density="compact">
          <v-list-item>
            <v-list-item-title>Anomalies</v-list-item-title>
            <template #append>
              <b>{{ findings.length }}</b>
            </template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Aller-retours</v-list-item-title>
            <template #append>{{ countByKind('ar') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Boucles giratoires</v-list-item-title>
            <template #append>{{ countByKind('rp') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>À traiter</v-list-item-title>
            <template #append>{{ countByStatus('pending') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Faux positifs</v-list-item-title>
            <template #append>{{ countByStatus('fp') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Corrigées</v-list-item-title>
            <template #append>{{ countByStatus('corrected') }}</template>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Distance</v-list-item-title>
            <template #append>
              {{ (totalDistanceM / 1000).toFixed(1) }} km
            </template>
          </v-list-item>
        </v-list>
      </v-expansion-panel-text>
    </v-expansion-panel>
  </v-expansion-panels>
</template>

<script setup lang="ts">
/**
 * Synthèse globale de l'audit : compteurs par famille et par statut, et
 * longueur de la trace auditée.
 */
import type { Finding, FindingKind, FindingStatus } from '../../stores/audit'

const props = defineProps<{
  findings: Finding[]
  totalDistanceM: number
}>()

function countByKind(kind: FindingKind): number {
  return props.findings.filter((f) => f.kind === kind).length
}

function countByStatus(status: FindingStatus): number {
  return props.findings.filter((f) => f.status === status).length
}
</script>
