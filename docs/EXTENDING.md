# Guide d'extension du Template

> Comment ajouter des fonctionnalités au template

## Table des matières

1. [Ajouter une nouvelle page](#ajouter-une-nouvelle-page)
2. [Créer un nouveau store](#créer-un-nouveau-store)
3. [Ajouter un composant réutilisable](#ajouter-un-composant-réutilisable)
4. [Intégrer une API externe](#intégrer-une-api-externe)
5. [Ajouter des commandes Tauri](#ajouter-des-commandes-tauri)
6. [Gérer l'authentification](#gérer-lauthentification)
7. [Ajouter une base de données locale](#ajouter-une-base-de-données-locale)
8. [Internationalisation (i18n)](#internationalisation-i18n)
9. [Notifications système](#notifications-système)
10. [Gestion des formulaires](#gestion-des-formulaires)
11. [Ajouter un paramètre de configuration](#ajouter-un-paramètre-de-configuration)

---

## Ajouter une nouvelle page

### Étape 1 : Créer la vue

`src/views/Products.vue`
```vue
<template>
  <v-container>
    <v-row>
      <v-col cols="12">
        <h1>Produits</h1>
        <v-card>
          <v-card-text>
            Liste des produits ici
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const products = ref<string[]>([])

onMounted(() => {
  // Charger les produits
})
</script>
```

### Étape 2 : Ajouter la route

`src/router/index.ts`
```typescript
import Products from '../views/Products.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    // ... routes existantes
    {
      path: '/products',
      name: 'products',
      component: Products
    }
  ]
})
```

### Étape 3 : Ajouter au menu

`src/App.vue`
```vue
<v-navigation-drawer v-model="drawer" temporary>
  <v-list>
    <!-- Items existants -->
    <v-list-item
      prepend-icon="mdi-package-variant"
      title="Produits"
      :to="{ name: 'products' }"
    ></v-list-item>
  </v-list>
</v-navigation-drawer>
```

### Avec lazy loading (recommandé)

```typescript
{
  path: '/products',
  name: 'products',
  component: () => import('../views/Products.vue')
}
```

---

## Créer un nouveau store

### Store simple

`src/stores/products.ts`
```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export interface Product {
  id: string
  name: string
  price: number
}

export const useProductsStore = defineStore('products', () => {
  // State
  const products = ref<Product[]>([])
  const loading = ref(false)

  // Getters
  const productCount = computed(() => products.value.length)
  const totalValue = computed(() =>
    products.value.reduce((sum, p) => sum + p.price, 0)
  )

  // Actions
  async function fetchProducts() {
    loading.value = true
    try {
      // Simuler un appel API
      const response = await fetch('/api/products')
      products.value = await response.json()
    } catch (error) {
      console.error('Failed to fetch products:', error)
    } finally {
      loading.value = false
    }
  }

  function addProduct(product: Product) {
    products.value.push(product)
  }

  function removeProduct(id: string) {
    products.value = products.value.filter(p => p.id !== id)
  }

  function reset() {
    products.value = []
    loading.value = false
  }

  return {
    // State
    products,
    loading,
    // Getters
    productCount,
    totalValue,
    // Actions
    fetchProducts,
    addProduct,
    removeProduct,
    reset
  }
})
```

### Utilisation dans un composant

```vue
<script setup lang="ts">
import { useProductsStore } from '@/stores/products'
import { onMounted } from 'vue'

const productsStore = useProductsStore()

onMounted(() => {
  productsStore.fetchProducts()
})
</script>

<template>
  <div>
    <p>Produits: {{ productsStore.productCount }}</p>
    <p v-if="productsStore.loading">Chargement...</p>

    <v-list>
      <v-list-item
        v-for="product in productsStore.products"
        :key="product.id"
      >
        {{ product.name }} - {{ product.price }}€
      </v-list-item>
    </v-list>
  </div>
</template>
```

---

## Ajouter un composant réutilisable

### Composant simple

`src/components/ProductCard.vue`
```vue
<template>
  <v-card>
    <v-card-title>{{ product.name }}</v-card-title>
    <v-card-text>
      <p>Prix: {{ product.price }}€</p>
      <p v-if="product.description">{{ product.description }}</p>
    </v-card-text>
    <v-card-actions>
      <v-btn color="primary" @click="emit('buy', product.id)">
        Acheter
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import type { Product } from '@/stores/products'

const props = defineProps<{
  product: Product
}>()

const emit = defineEmits<{
  buy: [productId: string]
}>()
</script>
```

### Utilisation

```vue
<template>
  <ProductCard
    v-for="product in products"
    :key="product.id"
    :product="product"
    @buy="handleBuy"
  />
</template>

<script setup lang="ts">
import ProductCard from '@/components/ProductCard.vue'

function handleBuy(productId: string) {
  console.log('Achat:', productId)
}
</script>
```

---

## Intégrer une API externe

### Créer un service API

`src/services/api.ts`
```typescript
const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000'

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message)
    this.name = 'ApiError'
  }
}

async function request<T>(
  endpoint: string,
  options?: RequestInit
): Promise<T> {
  const url = `${API_BASE_URL}${endpoint}`

  try {
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    })

    if (!response.ok) {
      throw new ApiError(response.status, `HTTP ${response.status}`)
    }

    return await response.json()
  } catch (error) {
    if (error instanceof ApiError) throw error
    throw new Error(`Network error: ${error}`)
  }
}

export const api = {
  get: <T>(endpoint: string) => request<T>(endpoint),

  post: <T>(endpoint: string, data: unknown) =>
    request<T>(endpoint, {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  put: <T>(endpoint: string, data: unknown) =>
    request<T>(endpoint, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: <T>(endpoint: string) =>
    request<T>(endpoint, { method: 'DELETE' }),
}
```

### Utilisation dans un store

```typescript
import { api } from '@/services/api'

export const useProductsStore = defineStore('products', () => {
  async function fetchProducts() {
    try {
      products.value = await api.get<Product[]>('/products')
    } catch (error) {
      console.error('Failed to fetch products:', error)
      throw error
    }
  }

  return { fetchProducts }
})
```

---

## Ajouter des commandes Tauri

### Côté Rust

`src-tauri/src/main.rs`
```rust
use tauri::command;

#[command]
fn greet(name: String) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[command]
async fn fetch_system_info() -> Result<SystemInfo, String> {
    // Récupérer des infos système
    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            greet,
            fetch_system_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Côté Frontend

`src/services/tauri.ts`
```typescript
import { invoke } from '@tauri-apps/api/tauri'

export interface SystemInfo {
  os: string
  arch: string
}

export const tauriApi = {
  async greet(name: string): Promise<string> {
    return await invoke<string>('greet', { name })
  },

  async getSystemInfo(): Promise<SystemInfo> {
    return await invoke<SystemInfo>('fetch_system_info')
  }
}
```

### Utilisation dans un composant

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { tauriApi } from '@/services/tauri'

const greeting = ref('')

async function loadGreeting() {
  try {
    greeting.value = await tauriApi.greet('World')
  } catch (error) {
    console.error('Tauri command failed:', error)
  }
}
</script>
```

---

## Gérer l'authentification

### Store d'authentification

`src/stores/auth.ts`
```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '@/services/api'

export interface User {
  id: string
  email: string
  name: string
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const token = ref<string | null>(localStorage.getItem('auth_token'))
  const router = useRouter()

  const isAuthenticated = computed(() => !!token.value)

  async function login(email: string, password: string) {
    try {
      const response = await api.post<{ token: string; user: User }>(
        '/auth/login',
        { email, password }
      )

      token.value = response.token
      user.value = response.user
      localStorage.setItem('auth_token', response.token)

      router.push({ name: 'home' })
    } catch (error) {
      console.error('Login failed:', error)
      throw error
    }
  }

  function logout() {
    token.value = null
    user.value = null
    localStorage.removeItem('auth_token')
    router.push({ name: 'login' })
  }

  async function checkAuth() {
    if (!token.value) return false

    try {
      user.value = await api.get<User>('/auth/me')
      return true
    } catch (error) {
      logout()
      return false
    }
  }

  return {
    user,
    isAuthenticated,
    login,
    logout,
    checkAuth
  }
})
```

### Guard de navigation

`src/router/index.ts`
```typescript
import { useAuthStore } from '@/stores/auth'

router.beforeEach(async (to, from, next) => {
  const authStore = useAuthStore()

  // Routes publiques
  const publicRoutes = ['login', 'register']
  if (publicRoutes.includes(to.name as string)) {
    return next()
  }

  // Vérifier l'authentification
  if (!authStore.isAuthenticated) {
    return next({ name: 'login' })
  }

  next()
})
```

---

## Ajouter une base de données locale

### Installer SQLite via Tauri

`src-tauri/Cargo.toml`
```toml
[dependencies]
rusqlite = { version = "0.30", features = ["bundled"] }
```

### Commande Rust pour la DB

`src-tauri/src/database.rs`
```rust
use rusqlite::{Connection, Result};

pub fn init_db() -> Result<Connection> {
    let conn = Connection::open("app.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE
        )",
        [],
    )?;

    Ok(conn)
}

#[tauri::command]
pub fn get_users() -> Result<Vec<User>, String> {
    let conn = Connection::open("app.db").map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT id, name, email FROM users")
        .map_err(|e| e.to_string())?;

    let users = stmt
        .query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(users)
}
```

---

## Internationalisation (i18n)

### Installation

```bash
npm install vue-i18n@9
```

### Configuration

`src/plugins/i18n.ts`
```typescript
import { createI18n } from 'vue-i18n'

const messages = {
  en: {
    welcome: 'Welcome',
    hello: 'Hello {name}'
  },
  fr: {
    welcome: 'Bienvenue',
    hello: 'Bonjour {name}'
  }
}

export const i18n = createI18n({
  locale: 'fr',
  fallbackLocale: 'en',
  messages
})
```

`src/main.ts`
```typescript
import { i18n } from './plugins/i18n'

createApp(App)
  .use(i18n)
  .mount('#app')
```

### Utilisation

```vue
<template>
  <div>
    <p>{{ $t('welcome') }}</p>
    <p>{{ $t('hello', { name: 'World' }) }}</p>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const { t, locale } = useI18n()

function changeLanguage() {
  locale.value = locale.value === 'en' ? 'fr' : 'en'
}
</script>
```

---

## Notifications système

### Tauri notifications

```typescript
import { sendNotification } from '@tauri-apps/api/notification'

async function notify(title: string, body: string) {
  try {
    await sendNotification({
      title,
      body,
      icon: 'icon.png'
    })
  } catch (error) {
    console.error('Notification failed:', error)
  }
}
```

### Configuration Tauri

`src-tauri/tauri.conf.json`
```json
{
  "tauri": {
    "allowlist": {
      "notification": {
        "all": true
      }
    }
  }
}
```

---

## Gestion des formulaires

### Avec Vuetify

```vue
<template>
  <v-form @submit.prevent="handleSubmit">
    <v-text-field
      v-model="form.name"
      label="Nom"
      :rules="[rules.required]"
    />

    <v-text-field
      v-model="form.email"
      label="Email"
      type="email"
      :rules="[rules.required, rules.email]"
    />

    <v-btn type="submit" color="primary">
      Soumettre
    </v-btn>
  </v-form>
</template>

<script setup lang="ts">
import { reactive } from 'vue'

const form = reactive({
  name: '',
  email: ''
})

const rules = {
  required: (value: string) => !!value || 'Champ requis',
  email: (value: string) => {
    const pattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    return pattern.test(value) || 'Email invalide'
  }
}

function handleSubmit() {
  console.log('Form submitted:', form)
}
</script>
```

---

## Checklist d'extension

Avant d'ajouter une nouvelle fonctionnalité :

- [ ] Vérifier si la fonctionnalité existe déjà
- [ ] Suivre les conventions de nommage
- [ ] Typer toutes les données (TypeScript)
- [ ] Gérer les erreurs (try/catch)
- [ ] Tester la fonctionnalité
- [ ] Documenter si complexe
- [ ] Mettre à jour le README si nécessaire

---

**Ressources utiles** :
- [Tauri Guides](https://tauri.app/v1/guides/)
- [Vue 3 Examples](https://vuejs.org/examples/)
- [Vuetify Components](https://vuetifyjs.com/en/components/all/)
- [Pinia Examples](https://pinia.vuejs.org/cookbook/)

---

## Ajouter un paramètre de configuration

Le système de paramètres repose sur le fichier de configuration par défaut.

### Étape 1 : Déclarer le paramètre
Ajoutez-le dans `src-tauri/settings.default.toml` :

```toml
[MaNouvelle.Section.monParametre]
description = "Description courte de ce paramètre"
doc = "Explication Markdown détaillée pour l'utilisateur"
type = "Entier" # Actuellement géré: "Entier", "Secret"
default = 42
min = 0 # Optionnel
max = 100 # Optionnel
```

### Étape 2 : Créer le composant d'édition (si type non géré)
Si vous créez un nouveau type (ex: "Chaine", "Couleur"), vous devrez :
1. Créer le composant Vue correspondant `SettingsEditChaine.vue` dans `src/components/Accueil/` (inspirez-vous de `SettingsEditEntier.vue`).
2. L'ajouter au `v-switch` (ou logique équivalente) dans `src/components/Accueil/SettingsDrawer.vue` pour l'affichage dynamique.

### Étape 3 : Utiliser le paramètre
Dans n'importe quel composant Vue :

```vue
<script setup lang="ts">
import { computed } from 'vue'
import { useSettingsStore } from '@/stores/settings'

const settingsStore = useSettingsStore()

const monParam = computed(() => {
  const setting = settingsStore.settings.find(s => s.path === 'MaNouvelle.Section.monParametre')
  return setting ? setting.value : 42
})
</script>
```
