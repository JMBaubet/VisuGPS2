<script setup lang="ts">
// SettingsCategory : rendu d'une catégorie de paramètres.
//
// Trois modes de rendu selon la nature de la catégorie :
//   - handler spécial (ex: "monitors") : item simple, un clic ouvre la carte dédiée ;
//   - catégorie à un seul paramètre   : item direct (pas d'accordéon superflu) ;
//   - catégorie à plusieurs paramètres : accordéon repliable (v-list-group).
//
// Indicateurs visuels (sans icône additionnelle, pour préserver la flèche
// de l'accordéon) :
//   - criticité → l'icône (prepend) passe en orange (warning) ;
//   - surcharge → le libellé passe en bleu (info).
// Calculés par OR sur les paramètres enfants au niveau de la catégorie.
//
// NB : on utilise le slot #prepend avec un <v-icon :color> explicite plutôt
// que :prepend-icon + :color, car ce dernier ne colore l'icône qu'en état
// "active" (sélection nav/route) — or nos items ne sont jamais actifs.
import { computed } from 'vue'
import type { CategoryNode, ParamNode } from '../../composables/useSettingsTree'

const props = defineProps<{
  category: CategoryNode
  /** Autorise l'ouverture du handler spécial (ex: false si mono-écran pour les moniteurs). */
  canOpenHandler?: boolean
}>()

const emit = defineEmits<{
  (e: 'open-param', path: string): void
  (e: 'category-click', category: CategoryNode): void
}>()

/** Vrai si l'en-tête doit ouvrir une carte dédiée (handler spécial). */
const isOpenableHandler = computed(
  () => !!props.category.handler && props.canOpenHandler !== false,
)

/** Vrai si la catégorie est aplatissable en un item direct. */
const isFlat = computed(
  () => props.category.params.length === 1 && !props.category.handler,
)

/** Le paramètre unique quand la catégorie est aplatie. */
const singleParam = computed<ParamNode | null>(() =>
  isFlat.value ? props.category.params[0] : null,
)

/** Couleur de l'icône d'en-tête selon la remontée criticité/surcharge. */
const headerColor = computed<string | undefined>(() => {
  if (props.category.hasCritical) return 'warning'
  if (props.category.hasOverride) return 'info'
  return undefined
})

/** Couleur de l'icône d'un paramètre (warning si critique). */
function paramColor(param: ParamNode): string | undefined {
  return param.isCritical ? 'warning' : undefined
}

function onHeaderClick() {
  if (isOpenableHandler.value) {
    emit('category-click', props.category)
  }
}
</script>

<template>
  <!-- Catégorie à handler spécial : item simple (pas d'accordéon) -->
  <v-list-item
    v-if="category.handler"
    :title="category.label"
    :value="`cat-${category.id}`"
    :disabled="canOpenHandler === false"
    @click="onHeaderClick"
  >
    <template #prepend>
      <v-icon :color="headerColor">{{ category.icon }}</v-icon>
    </template>
  </v-list-item>

  <!-- Catégorie à un seul paramètre : item direct (pas d'accordéon superflu) -->
  <v-list-item
    v-else-if="isFlat && singleParam"
    :value="`param-${singleParam.def.path}`"
    @click="emit('open-param', singleParam.def.path)"
  >
    <template #prepend>
      <v-icon :color="paramColor(singleParam)">{{ singleParam.icon }}</v-icon>
    </template>
    <!-- Libellé coloré en bleu si le paramètre est surchargé -->
    <v-list-item-title :class="{ 'text-info': singleParam.isOverridden }">
      {{ singleParam.def.description }}
    </v-list-item-title>
  </v-list-item>

  <!-- Catégorie à plusieurs paramètres : accordéon repliable -->
  <v-list-group v-else :value="`cat-${category.id}`">
    <template #activator="{ props: activatorProps }">
      <v-list-item v-bind="activatorProps" :title="category.label">
        <template #prepend>
          <v-icon :color="headerColor">{{ category.icon }}</v-icon>
        </template>
      </v-list-item>
    </template>

    <v-list-item
      v-for="param in category.params"
      :key="param.def.path"
      :value="`param-${param.def.path}`"
      @click="emit('open-param', param.def.path)"
    >
      <template #prepend>
        <v-icon :color="paramColor(param)">{{ param.icon }}</v-icon>
      </template>
      <v-list-item-title :class="{ 'text-info': param.isOverridden }">
        {{ param.def.description }}
      </v-list-item-title>
    </v-list-item>
  </v-list-group>
</template>
