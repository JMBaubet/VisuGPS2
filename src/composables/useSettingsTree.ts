import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useSettingsStore, type SettingDefinition, type SettingsMeta } from '../stores/settings'

/**
 * Icône MDI par défaut selon le type de paramètre.
 * Utilisée lorsqu'aucune icône n'est définie dans le TOML (`def.icon`).
 */
const TYPE_ICONS: Record<string, string> = {
  int: 'mdi-numeric',
  float: 'mdi-decimal',
  bool: 'mdi-toggle-switch-outline',
  secret: 'mdi-form-textbox-password',
  list: 'mdi-format-list-bulleted',
  rgba: 'mdi-palette',
  material_primary: 'mdi-palette-outline',
  material_extended: 'mdi-palette-swatch',
  monitor: 'mdi-projector',
}

/** Icône MDI par défaut pour une catégorie sans icône déclarée. */
const DEFAULT_GROUP_ICON = 'mdi-folder-outline'

/** Dernier segment d'un chemin pointé (ex: "Carte.Traces" -> "Traces"). */
function lastSegment(path: string): string {
  const idx = path.lastIndexOf('.')
  return idx === -1 ? path : path.slice(idx + 1)
}

/** Paramètre enrichi pour l'affichage dans le drawer. */
export interface ParamNode {
  def: SettingDefinition
  icon: string
  isCritical: boolean
  isOverridden: boolean
}

/** Catégorie (groupe) enrichie pour l'affichage, avec remontée criticité/surcharge. */
export interface CategoryNode {
  /** Identifiant du groupe (ex: "Carte.Traces"). */
  id: string
  label: string
  icon: string
  params: ParamNode[]
  /** Au moins un paramètre enfant est critique. */
  hasCritical: boolean
  /** Au moins un paramètre enfant est surchargé. */
  hasOverride: boolean
  /** Handler spécial (ex: "monitors") si la catégorie ne liste pas ses params individuellement. */
  handler: string | null
}

/**
 * Construit l'arbre des catégories à afficher pour une vue donnée.
 *
 * @param meta    métadonnées d'organisation (_meta).
 * @param settings liste plate des définitions de paramètres.
 * @param groupIds identifiants des groupes à inclure (vue active ou système).
 * @returns catégories ordonnées, filtrées et enrichies.
 */
function buildCategoryNodes(
  meta: SettingsMeta | null,
  settings: SettingDefinition[],
  groupIds: string[],
): CategoryNode[] {
  if (!meta) return []

  const nodes: CategoryNode[] = []
  for (const groupId of groupIds) {
    const groupMeta = meta.groups[groupId] ?? {}
    const label = groupMeta.label ?? lastSegment(groupId)
    const icon = groupMeta.icon ?? DEFAULT_GROUP_ICON
    const handler = meta.system.handlers[groupId] ?? null

    const params: ParamNode[] = settings
      .filter((s) => s.path === groupId || s.path.startsWith(groupId + '.'))
      .map((def) => ({
        def,
        icon: def.icon ?? TYPE_ICONS[def.type] ?? 'mdi-cog-outline',
        isCritical: !!def.critical,
        isOverridden: !!def.is_overridden,
      }))

    nodes.push({
      id: groupId,
      label,
      icon,
      params,
      // Une catégorie à handler (ex: moniteurs) n'expose pas ses params
      // individuellement : on force la remontée à false car l'affichage est
      // traité par la carte spécialisée.
      hasCritical: handler ? params.some((p) => p.isCritical) : params.some((p) => p.isCritical),
      hasOverride: handler ? params.some((p) => p.isOverridden) : params.some((p) => p.isOverridden),
      handler,
    })
  }

  // On écarte les catégories sans paramètres ni handler (rien à afficher).
  return nodes.filter((n) => n.handler || n.params.length > 0)
}

/**
 * Composable centralisant la logique de construction de l'arbre d'affichage
 * du drawer de paramètres, pilotée par `_meta` et filtrée selon la vue active.
 */
export function useSettingsTree() {
  const route = useRoute()
  const settingsStore = useSettingsStore()

  /** Nom de la vue active (aligné sur le nom de route, ex: "accueil", "carte"). */
  const activeView = computed<string>(() => String(route.name ?? ''))

  /** Métadonnées de la vue active (label, icône), ou null si inconnue. */
  const activeViewMeta = computed(() => {
    const views = settingsStore.meta?.views ?? {}
    return views[activeView.value] ?? null
  })

  /** Catégories de la section système (communes à toutes les vues). */
  const systemCategories = computed<CategoryNode[]>(() =>
    buildCategoryNodes(
      settingsStore.meta,
      settingsStore.settings,
      settingsStore.meta?.system.groups ?? [],
    ),
  )

  /** Actions de la section système (entrées non-paramètres, ex: Modes d'exécution). */
  const systemActions = computed(() => {
    const actions = settingsStore.meta?.system.actions ?? {}
    // On conserve l'ordre d'insertion du TOML (BTreeMap côté Rust = ordre clé).
    return Object.entries(actions).map(([key, entry]) => ({ key, ...entry }))
  })

  /** Catégories propres à la vue active. */
  const viewCategories = computed<CategoryNode[]>(() => {
    const view = settingsStore.meta?.views[activeView.value]
    if (!view) return []
    return buildCategoryNodes(settingsStore.meta, settingsStore.settings, view.groups)
  })

  return {
    activeView,
    activeViewMeta,
    systemActions,
    systemCategories,
    viewCategories,
  }
}
