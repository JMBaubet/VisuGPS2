<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useOsmStore, type OsmRegionOption } from '../../stores/osm'
import { useUiStore } from '../../stores/ui'
import { formatDate, formatDateTime, formatFileSize } from '../../utils/format'
import OsmFlag from './OsmFlag.vue'

interface RegionCountryNode {
  key: string
  name: string
  code: string | null
  regions: OsmRegionOption[]
}

interface RegionContinentNode {
  key: string
  name: string
  countries: RegionCountryNode[]
  count: number
}

const CONTINENT_ORDER = [
  'europe',
  'africa',
  'north-america',
  'south-america',
  'central-america',
  'asia',
  'australia-oceania',
  'antarctica',
  'other',
]

const CONTINENT_LABELS: Record<string, string> = {
  africa: 'Afrique',
  antarctica: 'Antarctique',
  asia: 'Asie',
  'australia-oceania': 'Océanie',
  'central-america': 'Amérique centrale',
  europe: 'Europe',
  'north-america': 'Amérique du Nord',
  'south-america': 'Amérique du Sud',
}

const osmStore = useOsmStore()
const ui = useUiStore()

const selectedId = ref<string | null>(null)
const search = ref('')

const catalogPresent = computed(() => osmStore.catalog?.present ?? false)
const regions = computed<OsmRegionOption[]>(
  () => osmStore.catalog?.regions ?? [],
)
const selected = computed<OsmRegionOption | null>(() => {
  const id = selectedId.value
  if (!id) return null
  return regions.value.find(region => region.id === id) ?? null
})
const probe = computed(() => {
  const state = osmStore.probe
  return state && state.regionId === selectedId.value ? state : null
})
const bytes = computed<number | null>(() => {
  if (!selected.value) return null
  if (probe.value?.loading) return null
  if (probe.value?.bytes != null) return probe.value.bytes
  return selected.value.sizeMb != null
    ? Math.round(selected.value.sizeMb * 1_048_576)
    : null
})
const available = computed(() => osmStore.overview?.freeBytes ?? null)
const insufficient = computed(
  () =>
    bytes.value != null &&
    available.value != null &&
    bytes.value > available.value,
)
const spaceUnknown = computed(() => available.value == null)
const sizeUnknown = computed(() => bytes.value == null)
const overCap = computed(() => {
  const max = osmStore.overview?.maxPbfMb ?? 0
  return max > 0 && bytes.value != null && bytes.value > max * 1_048_576
})
const downloadingThis = computed(
  () =>
    selected.value != null &&
    osmStore.downloading &&
    (osmStore.downloadingRegionId === selected.value.id ||
      osmStore.progress?.regionId === selected.value.id),
)
const canDownload = computed(
  () =>
    selected.value != null &&
    !osmStore.isBusy &&
    bytes.value != null &&
    available.value != null &&
    !insufficient.value,
)
const buttonLabel = computed(() => {
  if (!selected.value) return 'Sélectionnez une région'
  if (probe.value?.loading) return 'Calcul de la taille…'
  if (bytes.value == null) return 'Taille indisponible'
  if (spaceUnknown.value) return 'Espace disponible inconnu'
  if (insufficient.value) return 'Espace insuffisant'
  if (overCap.value) {
    return selected.value.present
      ? 'Renouveler quand même'
      : 'Télécharger quand même'
  }
  return selected.value.present ? 'Renouveler' : 'Télécharger'
})
const buttonColor = computed(() => (overCap.value ? 'warning' : 'primary'))
const buttonVariant = computed<'flat' | 'tonal'>(() =>
  overCap.value ? 'tonal' : 'flat',
)
const catalogDate = computed(() => {
  const catalog = osmStore.catalog
  if (!catalog) return 'chargement…'
  if (!catalog.present) return 'absent'
  return catalog.version
    ? `du ${formatDateTime(catalog.version)}`
    : 'date inconnue'
})
const catalogRefresh = computed(() => osmStore.progress?.stage === 'catalog')
const catalogLegacy = computed(
  () =>
    catalogPresent.value &&
    regions.value.length > 0 &&
    regions.value.every(region => !region.continent),
)

const continents = computed<RegionContinentNode[]>(() => {
  const byKey = new Map<string, RegionContinentNode>()
  for (const region of regions.value) {
    // Les entrées de niveau continent (`africa`, `europe`…) d'un catalogue
    // généré avant l'ajout du champ ont `continent = null` : on les rattache
    // à leur propre continent pour stabiliser l'arborescence.
    const fallbackContinent = CONTINENT_LABELS[region.id]
    const continentKey =
      region.continent ?? (fallbackContinent ? region.id : 'other')
    const continentName =
      region.continentName ?? fallbackContinent ?? 'Autres'
    let continent = byKey.get(continentKey)
    if (!continent) {
      continent = {
        key: continentKey,
        name: continentName,
        countries: [],
        count: 0,
      }
      byKey.set(continentKey, continent)
    }
    continent.count += 1

    const countryName =
      region.countryName ??
      (continentKey !== 'other' && region.id === continentKey
        ? 'Continent entier'
        : 'Pays inconnu')
    let country = continent.countries.find(
      candidate => candidate.name === countryName,
    )
    if (!country) {
      country = {
        key: `${continentKey}:${countryName}`,
        name: countryName,
        code: region.countryCode,
        regions: [],
      }
      continent.countries.push(country)
    }
    country.regions.push(region)
  }

  return Array.from(byKey.values())
    .sort((a, b) => {
      const indexA = CONTINENT_ORDER.indexOf(a.key)
      const indexB = CONTINENT_ORDER.indexOf(b.key)
      const rankA = indexA === -1 ? CONTINENT_ORDER.length : indexA
      const rankB = indexB === -1 ? CONTINENT_ORDER.length : indexB
      return rankA - rankB || a.name.localeCompare(b.name, 'fr')
    })
    .map(continent => {
      continent.countries.sort((a, b) => a.name.localeCompare(b.name, 'fr'))
      for (const country of continent.countries) {
        country.regions.sort((a, b) => a.name.localeCompare(b.name, 'fr'))
      }
      return continent
    })
})

const filteredRegions = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return []
  return regions.value.filter(region =>
    [
      region.name,
      region.countryName,
      region.continentName,
      region.pbfFilename,
      region.id,
    ].some(
      part => typeof part === 'string' && part.toLowerCase().includes(query),
    ),
  )
})

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function selectRegion(region: OsmRegionOption): void {
  selectedId.value = region.id
  void osmStore.loadRegionGeometry(region.id)
}

function retryProbe(): void {
  if (selected.value) void osmStore.probeRegion(selected.value.id)
}

async function refreshCatalog(): Promise<void> {
  selectedId.value = null
  osmStore.clearPreview()
  try {
    await osmStore.refreshCatalog()
    await osmStore.loadPresentRegions()
    ui.showSuccess('Catalogue OSM mis à jour.')
  } catch (error) {
    ui.showError(errorMessage(error))
  }
}

async function download(): Promise<void> {
  const region = selected.value
  if (!region || bytes.value == null) return
  const renew = region.present
  try {
    const result = await osmStore.downloadRegion(
      region.id,
      renew,
      overCap.value,
    )
    ui.showSuccess(
      `${region.name} ${renew ? 'renouvelé' : 'téléchargé'} (${formatFileSize(
        result.sizeBytes,
      )}).`,
    )
  } catch (error) {
    ui.showError(errorMessage(error))
  }
}

watch(selectedId, value => {
  if (value) void osmStore.probeRegion(value)
})
</script>

<template>
  <div class="osm-download">
    <div class="d-flex align-center mb-1">
      <div class="text-subtitle-1">Télécharger une région</div>
      <v-spacer />
      <v-btn
        icon="mdi-refresh"
        variant="text"
        density="comfortable"
        size="small"
        title="Mettre à jour le catalogue Geofabrik"
        :disabled="osmStore.isBusy"
        @click="refreshCatalog"
      />
    </div>
    <div class="text-caption text-medium-emphasis mb-3">
      Catalogue Geofabrik : {{ catalogDate }}
    </div>

    <v-progress-linear
      v-if="catalogRefresh"
      :model-value="osmStore.progress?.percent ?? 0"
      :indeterminate="!osmStore.progress || osmStore.progress.bytesTotal === 0"
      color="info"
      height="4"
      class="mb-3"
    />

    <v-alert
      v-if="!catalogPresent"
      type="info"
      variant="tonal"
      density="compact"
      class="mb-3"
      text="Le catalogue regions.json est absent. Téléchargez-le pour choisir une région (index Geofabrik, environ 50 Mo)."
    />
    <v-btn
      v-if="!catalogPresent"
      block
      color="primary"
      variant="flat"
      prepend-icon="mdi-cloud-download"
      :loading="osmStore.catalogLoading"
      :disabled="osmStore.isBusy"
      @click="refreshCatalog"
    >
      Télécharger le catalogue
    </v-btn>

    <template v-else>
      <v-text-field
        v-model="search"
        label="Rechercher une région, un pays…"
        prepend-inner-icon="mdi-magnify"
        density="compact"
        variant="outlined"
        clearable
        hide-details
        class="mb-2"
      />

      <div class="osm-download__tree">
        <v-list v-if="!search.trim()" density="compact" nav>
          <v-list-group
            v-for="continent in continents"
            :key="continent.key"
            :value="continent.key"
          >
            <template #activator="{ props: groupProps }">
              <v-list-item
                v-bind="groupProps"
                :title="continent.name"
                prepend-icon="mdi-earth"
              >
                <template #append>
                  <v-chip size="x-small" variant="text">
                    {{ continent.count }}
                  </v-chip>
                </template>
              </v-list-item>
            </template>

            <v-list-group
              v-for="country in continent.countries"
              :key="country.key"
              :value="country.key"
            >
              <template #activator="{ props: countryProps }">
                <v-list-item v-bind="countryProps" :title="country.name">
                  <template #prepend>
                    <OsmFlag :code="country.code" :name="country.name" />
                  </template>
                  <template #append>
                    <v-chip size="x-small" variant="text">
                      {{ country.regions.length }}
                    </v-chip>
                  </template>
                </v-list-item>
              </template>

              <v-list-item
                v-for="region in country.regions"
                :key="region.id"
                :active="selectedId === region.id"
                @click="selectRegion(region)"
              >
                <template #prepend>
                  <OsmFlag
                    :code="region.countryCode"
                    :name="region.countryName"
                  />
                </template>
                <v-list-item-title>{{ region.name }}</v-list-item-title>
                <v-list-item-subtitle>
                  {{ region.pbfFilename }}
                </v-list-item-subtitle>
                <template #append>
                  <v-chip
                    v-if="region.present"
                    size="x-small"
                    color="success"
                    variant="tonal"
                  >
                    présent
                  </v-chip>
                </template>
              </v-list-item>
            </v-list-group>
          </v-list-group>
        </v-list>

        <v-list v-else density="compact" nav>
          <v-list-item
            v-for="region in filteredRegions"
            :key="region.id"
            :active="selectedId === region.id"
            @click="selectRegion(region)"
          >
            <template #prepend>
              <OsmFlag
                :code="region.countryCode"
                :name="region.countryName"
              />
            </template>
            <v-list-item-title>{{ region.name }}</v-list-item-title>
            <v-list-item-subtitle>
              {{ region.continentName ?? 'Continent inconnu' }} ·
              {{ region.countryName ?? 'Pays inconnu' }} ·
              {{ region.pbfFilename }}
            </v-list-item-subtitle>
            <template #append>
              <v-chip
                v-if="region.present"
                size="x-small"
                color="success"
                variant="tonal"
              >
                présent
              </v-chip>
            </template>
          </v-list-item>
          <v-list-item v-if="filteredRegions.length === 0">
            <v-list-item-title>
              Aucune région ne correspond à la recherche.
            </v-list-item-title>
          </v-list-item>
        </v-list>
      </div>

      <v-alert
        v-if="catalogLegacy"
        type="info"
        variant="tonal"
        density="compact"
        class="mt-2"
      >
        Catalogue ancien : mettez à jour le catalogue pour afficher
        l'arborescence Continent / pays / région et les drapeaux.
        <template #append>
          <v-btn
            size="small"
            variant="text"
            :disabled="osmStore.isBusy"
            @click="refreshCatalog"
          >
            Mettre à jour
          </v-btn>
        </template>
      </v-alert>

      <v-card v-if="selected" variant="tonal" class="mt-4 pa-3">
        <div class="d-flex align-center">
          <OsmFlag
            :code="selected.countryCode"
            :name="selected.countryName"
            class="mr-2"
          />
          <div class="osm-download__selected-text">
            <div class="text-body-2 font-weight-medium">
              {{ selected.name }}
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ selected.countryName ?? 'Pays inconnu' }} ·
              {{ selected.pbfFilename }}
            </div>
          </div>
        </div>
        <v-divider class="my-2" />
        <div class="text-caption">
          Taille : {{ bytes != null ? formatFileSize(bytes) : '—' }}
        </div>
        <div class="text-caption">
          Présent :
          {{
            selected.present
              ? `oui${
                  selected.localCreatedAt
                    ? ` (${formatDate(selected.localCreatedAt)})`
                    : ''
                }`
              : 'non'
          }}
        </div>

        <v-alert
          v-if="osmStore.previewApproximate"
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          Aperçu approximatif (rectangle de la bbox). Mettez à jour le catalogue
          pour afficher la forme exacte de la région.
          <template #append>
            <v-btn
              size="small"
              variant="text"
              :disabled="osmStore.isBusy"
              @click="refreshCatalog"
            >
              Générer
            </v-btn>
          </template>
        </v-alert>

        <v-alert
          v-if="probe?.error"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          {{ probe.error }}
          <template #append>
            <v-btn size="small" variant="text" @click="retryProbe">
              Réessayer
            </v-btn>
          </template>
        </v-alert>
        <v-alert
          v-else-if="sizeUnknown && !probe?.loading"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          Taille distante indisponible.
          <template #append>
            <v-btn size="small" variant="text" @click="retryProbe">
              Réessayer
            </v-btn>
          </template>
        </v-alert>
        <v-alert
          v-if="insufficient"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          Espace insuffisant : {{ formatFileSize(bytes) }} requis,
          {{ formatFileSize(available) }} disponibles.
        </v-alert>
        <v-alert
          v-else-if="spaceUnknown"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          Espace disponible illisible. Vérifiez que le support OSM est
          accessible.
        </v-alert>
        <v-alert
          v-if="overCap"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2"
        >
          Dépasse le plafond de
          {{ osmStore.overview?.maxPbfMb ?? 0 }} Mo
          (Osm.Fichiers.tailleMaxPbf). Le bouton permet de passer outre.
        </v-alert>
      </v-card>

      <v-progress-linear
        v-if="downloadingThis"
        :model-value="osmStore.progress?.percent ?? 0"
        :indeterminate="!osmStore.progress || osmStore.progress.bytesTotal === 0"
        color="info"
        height="4"
        class="mt-3"
      />

      <v-btn
        block
        class="mt-3"
        :color="buttonColor"
        :variant="buttonVariant"
        :disabled="!canDownload && !downloadingThis"
        :loading="downloadingThis"
        prepend-icon="mdi-download"
        @click="download"
      >
        {{ buttonLabel }}
      </v-btn>
    </template>

    <v-alert
      v-if="osmStore.lastError"
      type="error"
      variant="tonal"
      density="compact"
      class="mt-3"
      :text="osmStore.lastError"
    />
  </div>
</template>

<style scoped>
.osm-download__selected-text {
  min-width: 0;
}

.osm-download__tree {
  max-height: 38vh;
  overflow-y: auto;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 4px;
}

.osm-download__tree :deep(.v-list-item__prepend) > .osm-flag,
.osm-download__tree :deep(.v-list-item__prepend) > .v-icon {
  margin-inline-end: 8px;
}
</style>
