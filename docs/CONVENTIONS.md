# Conventions de développement

> Guide des conventions de code, nommage et structure pour ce template

## Principes généraux

1. **Cohérence avant tout** : Suivre les patterns existants
2. **Simplicité** : Code lisible > code "clever"
3. **TypeScript strict** : Typer tout ce qui peut l'être
4. **Pas de duplication** : DRY (Don't Repeat Yourself)
5. **Documentation dans le code** : Noms explicites plutôt que commentaires

## Nommage

### Fichiers et dossiers

```
✅ Bon                          ❌ Mauvais
src/views/UserProfile.vue      src/views/userprofile.vue
src/stores/userAuth.ts         src/stores/UserAuth.ts
src/components/UserCard.vue    src/components/user-card.vue
src/router/index.ts            src/router/routes.ts
```

**Règles** :
- **Composants Vue** : PascalCase (ex: `UserProfile.vue`, `TodoList.vue`)
- **Stores** : camelCase (ex: `userAuth.ts`, `todoList.ts`)
- **Autres fichiers** : camelCase (ex: `helpers.ts`, `api.ts`)
- **Dossiers** : kebab-case si multi-mots (ex: `user-management/`)
- **Point d'entrée** : toujours `index.ts` (pas `main.ts` sauf racine)

### Variables et fonctions

```typescript
// ✅ Bon
const userName = ref('John')
const isAuthenticated = computed(() => !!user.value)
function fetchUserData() { ... }

// ❌ Mauvais
const UserName = ref('John')           // PascalCase pour variable
const authenticated = computed(...)    // Pas clair que c'est boolean
function getUserDataFromAPI() { ... }  // Trop verbeux
```

**Règles** :
- **Variables** : camelCase
- **Constantes** : camelCase ou UPPER_SNAKE_CASE si vraiment constante
- **Fonctions** : camelCase, verbe à l'impératif (fetch, create, update, delete)
- **Booléens** : Préfixe `is`, `has`, `should`, `can`
- **Handlers** : Préfixe `on` ou `handle` (ex: `onClick`, `handleSubmit`)

### Composants

```typescript
// ✅ Bon
const props = defineProps<{ userId: string }>()
const emit = defineEmits<{ submit: [value: string] }>()

// ❌ Mauvais
const props = defineProps<{ user_id: string }>()  // snake_case
const emit = defineEmits(['submit'])              // Pas typé
```

**Règles** :
- **Props** : camelCase
- **Events** : kebab-case dans le template, camelCase dans defineEmits
- **Slots** : kebab-case

### Stores Pinia

```typescript
// ✅ Bon
export const useUserStore = defineStore('user', () => { ... })
export const useCartStore = defineStore('cart', () => { ... })

// ❌ Mauvais
export const userStore = defineStore('user', () => { ... })      // Manque "use"
export const useUserAuthStore = defineStore('userAuth', () => { ... })  // ID != nom
```

**Règles** :
- **Nom de fonction** : `use` + Nom + `Store` en PascalCase
- **ID du store** : camelCase, simple (pas de suffix "Store")
- **Nom = ID + Store** : `useUserStore('user')`, `useCartStore('cart')`

### Routes

```typescript
// ✅ Bon
{
  path: '/user-profile',
  name: 'user-profile',
  component: UserProfile
}

// ❌ Mauvais
{
  path: '/userProfile',       // camelCase dans URL
  name: 'UserProfile',        // PascalCase dans name
  component: UserProfile
}
```

**Règles** :
- **path** : kebab-case avec `/` au début
- **name** : kebab-case (correspond au path)
- **Éviter** : IDs dynamiques dans le name (utiliser le path)

## Structure des fichiers

### Composant Vue

```vue
<template>
  <!-- Template en premier -->
  <div>
    <h1>{{ title }}</h1>
  </div>
</template>

<script setup lang="ts">
// Imports en premier
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'

// Props et emits
const props = defineProps<{
  title: string
}>()

const emit = defineEmits<{
  close: []
}>()

// Composables
const router = useRouter()

// State local
const count = ref(0)

// Computed
const doubleCount = computed(() => count.value * 2)

// Functions
function increment() {
  count.value++
}

// Lifecycle (si nécessaire)
onMounted(() => {
  console.log('Mounted')
})
</script>

<style scoped>
/* Styles en dernier, scoped si nécessaire */
</style>
```

**Ordre dans `<script setup>` (important)** :
1. Imports
2. Props (defineProps)
3. Emits (defineEmits)
4. Composables (use*)
5. State (ref, reactive)
6. Computed
7. Fonctions
8. Lifecycle hooks
9. Watchers (watch, watchEffect)

### Store Pinia (Setup pattern)

```typescript
// src/stores/user.ts
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useUserStore = defineStore('user', () => {
  // 1. State
  const name = ref<string>('')
  const age = ref<number>(0)

  // 2. Getters (computed)
  const isAdult = computed(() => age.value >= 18)
  const displayName = computed(() => name.value.toUpperCase())

  // 3. Actions
  function setUser(newName: string, newAge: number) {
    name.value = newName
    age.value = newAge
  }

  function reset() {
    name.value = ''
    age.value = 0
  }

  // 4. Return (API publique)
  return {
    // State
    name,
    age,
    // Getters
    isAdult,
    displayName,
    // Actions
    setUser,
    reset
  }
})
```

**Règles** :
- Toujours **setup pattern** (pas options API)
- **State en ref/reactive** (pas de state object)
- **Getters en computed** (pas de fonctions simples)
- **Actions en fonctions** (pas de méthodes async directement)
- **Grouper dans le return** par type (state, getters, actions)

### Router

```typescript
// src/router/index.ts
import { createRouter, createWebHistory } from 'vue-router'

// Imports de pages
import Home from '../views/Home.vue'

// Routes
const routes = [
  {
    path: '/',
    name: 'home',
    component: Home
  },
  {
    path: '/about',
    name: 'about',
    component: () => import('../views/About.vue') // Lazy loading
  }
]

// Création du router
const router = createRouter({
  history: createWebHistory(),
  routes
})

// Guards (si nécessaire)
router.beforeEach((to, from, next) => {
  // Logique de navigation
  next()
})

export default router
```

## TypeScript

### Types et interfaces

```typescript
// ✅ Bon
interface User {
  id: string
  name: string
  email: string
}

type UserId = string
type UserStatus = 'active' | 'inactive'

// ❌ Mauvais
interface IUser { ... }           // Pas de préfixe I
type user = { ... }                // PascalCase pour types
type UserStatusType = 'active' | 'inactive'  // Suffix "Type" inutile
```

**Règles** :
- **Interface** : PascalCase, singulier
- **Type** : PascalCase
- **Pas de préfixe** `I` pour interface
- **Pas de suffix** `Type` pour type
- **Union types** : Pour les valeurs littérales multiples

### Typage des props

```typescript
// ✅ Bon - Runtime + Type
interface Props {
  title: string
  count?: number
  items: string[]
}

const props = withDefaults(defineProps<Props>(), {
  count: 0,
  items: () => []
})

// ✅ Acceptable - Type seul
const props = defineProps<{
  title: string
  count?: number
}>()

// ❌ Mauvais - Pas typé
const props = defineProps({
  title: String,
  count: Number
})
```

### Typage des refs

```typescript
// ✅ Bon
const name = ref<string>('')
const user = ref<User | null>(null)
const items = ref<string[]>([])

// ❌ Mauvais
const name = ref('')           // Type inféré mais mieux explicite
const user = ref(null)         // Type unknown
const items = ref([])          // Type any[]
```

### Typage des fonctions

```typescript
// ✅ Bon
function fetchUser(id: string): Promise<User> {
  return api.get(`/users/${id}`)
}

function formatDate(date: Date, format: string = 'YYYY-MM-DD'): string {
  return ...
}

// ❌ Mauvais
function fetchUser(id) {       // Pas de type pour param
  return api.get(`/users/${id}`)  // Pas de type de retour
}
```

**Règles** :
- Toujours typer les **paramètres**
- Toujours typer le **retour** (sauf void évident)
- Utiliser **types génériques** quand approprié

## Vue et Composition API

### Refs vs Reactive

```typescript
// ✅ Bon - Utiliser ref
const count = ref(0)
const user = ref<User | null>(null)

// ❌ Éviter - reactive pour objets simples
const state = reactive({
  count: 0,
  user: null
})
```

**Règle** : Privilégier `ref` dans ce template (cohérence)

### Computed vs Functions

```typescript
// ✅ Bon - Computed pour valeurs dérivées
const fullName = computed(() => `${firstName.value} ${lastName.value}`)

// ❌ Mauvais - Fonction pour valeur dérivée
const getFullName = () => `${firstName.value} ${lastName.value}`
```

**Règle** : `computed` pour valeurs dérivées, `function` pour actions

### Watch vs watchEffect

```typescript
// ✅ Bon - watch pour source spécifique
watch(userId, (newId) => {
  fetchUser(newId)
})

// ✅ Bon - watchEffect pour multiples sources
watchEffect(() => {
  console.log(`${firstName.value} ${lastName.value}`)
})

// ❌ Mauvais - watchEffect quand la source est claire
watchEffect(() => {
  if (userId.value) fetchUser(userId.value)
})
```

## Vuetify

### Utilisation des composants

```vue
<!-- ✅ Bon -->
<v-btn color="primary" @click="handleClick">
  Cliquer
</v-btn>

<v-card>
  <v-card-title>Titre</v-card-title>
  <v-card-text>Contenu</v-card-text>
</v-card>

<!-- ❌ Mauvais - Imports inutiles -->
<script setup>
import { VBtn, VCard } from 'vuetify/components'
</script>

<template>
  <VBtn>...</VBtn>  <!-- Auto-import activé -->
</template>
```

**Règles** :
- **Pas d'imports** : Auto-import activé
- **Kebab-case** dans template : `<v-btn>` pas `<VBtn>`
- **Props Vuetify** : Utiliser les props natives (color, variant, etc.)

### Spacing et Layout

```vue
<!-- ✅ Bon - Classes Vuetify -->
<div class="ma-4 pa-2">
  <v-container>
    <v-row>
      <v-col cols="12" md="6">...</v-col>
    </v-row>
  </v-container>
</div>

<!-- ❌ Mauvais - CSS custom inutile -->
<div style="margin: 16px; padding: 8px">
  ...
</div>
```

**Classes utilitaires Vuetify** :
- `ma-X` : margin all sides
- `pa-X` : padding all sides
- `mt-X`, `pt-X` : margin/padding top
- `text-center`, `text-right` : alignement texte
- `d-flex`, `justify-center` : flexbox

## Gestion des erreurs

### Try/Catch

```typescript
// ✅ Bon
async function fetchData() {
  try {
    const data = await api.get('/data')
    return data
  } catch (error) {
    console.error('Failed to fetch data:', error)
    // Gérer l'erreur appropriément
    throw error  // ou return null selon le contexte
  }
}

// ❌ Mauvais
async function fetchData() {
  const data = await api.get('/data')  // Pas de gestion d'erreur
  return data
}
```

### Tauri invoke

```typescript
// ✅ Bon
try {
  const result = await invoke<string>('my_command', { arg: 'value' })
  console.log(result)
} catch (error) {
  console.error('Tauri command failed:', error)
}

// ❌ Mauvais
const result = await invoke('my_command', { arg: 'value' })
// Pas de typage, pas de gestion d'erreur
```

## Commentaires et documentation

### Quand commenter

```typescript
// ✅ Bon - Code auto-documenté
function calculateTotalPrice(items: CartItem[]): number {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0)
}

// ❌ Mauvais - Commentaire inutile
// Cette fonction calcule le prix total
function calc(items: any[]): number {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0)
}
```

**Règles** :
- **Noms explicites** > commentaires
- Commenter le **pourquoi**, pas le **quoi**
- **JSDoc** pour fonctions publiques/complexes

### JSDoc (optionnel)

```typescript
/**
 * Récupère les données utilisateur depuis l'API
 * @param userId - L'identifiant de l'utilisateur
 * @returns Les données de l'utilisateur
 * @throws {Error} Si l'utilisateur n'existe pas
 */
async function fetchUser(userId: string): Promise<User> {
  ...
}
```

## Import et Export

### Ordre des imports

```typescript
// 1. Imports Vue
import { ref, computed, onMounted } from 'vue'

// 2. Imports Vue Router / Pinia
import { useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'

// 3. Imports Tauri
import { invoke } from '@tauri-apps/api/core'

// 4. Imports locaux
import MyComponent from '@/components/MyComponent.vue'
import { helper } from '@/utils/helpers'

// 5. Imports types
import type { User } from '@/types'
```

### Exports

```typescript
// ✅ Bon - Named exports
export function myFunction() { ... }
export const myConst = 'value'

// ✅ Acceptable - Default export pour composants
export default defineComponent({ ... })

// ❌ Mauvais - Éviter export default ailleurs
export default function myFunction() { ... }
```

**Règles** :
- **Named exports** pour fonctions, constantes
- **Default export** uniquement pour composants Vue
- **Pas de mix** des deux dans un même fichier

## Performance

### Éviter les re-renders inutiles

```vue
<script setup lang="ts">
// ✅ Bon - Computed pour transformation
const filteredItems = computed(() =>
  items.value.filter(item => item.active)
)

// ❌ Mauvais - Fonction appelée à chaque render
const getFilteredItems = () => items.value.filter(item => item.active)
</script>

<template>
  <div v-for="item in filteredItems" :key="item.id">
    {{ item.name }}
  </div>
</template>
```

### Lazy loading

```typescript
// ✅ Bon - Routes non-critiques en lazy
{
  path: '/admin',
  component: () => import('../views/Admin.vue')
}

// ❌ Mauvais - Tout en eager loading
import Admin from '../views/Admin.vue'
{
  path: '/admin',
  component: Admin
}
```

## Points de contrôle

Avant de commit, vérifier :

- [ ] Tous les fichiers suivent les conventions de nommage
- [ ] Pas de `any` ou `unknown` non justifié
- [ ] Tous les imports nécessaires sont présents
- [ ] Pas d'imports inutilisés
- [ ] Les composants Vue suivent l'ordre établi
- [ ] Les stores utilisent le setup pattern
- [ ] Pas de console.log oubliés (sauf intentionnels)
- [ ] Les erreurs sont gérées (try/catch)
- [ ] Le code est lisible sans commentaire

---

**Note** : Ces conventions sont des guidelines, pas des règles absolues. En cas de doute, privilégier la **cohérence** avec le code existant.
