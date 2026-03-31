import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vuetify from 'vite-plugin-vuetify'

export default defineConfig({
  plugins: [
    vue(),
    vuetify({ autoImport: true }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Permet aux webviews Tauri secondaires d'accéder au serveur de développement
    allowedHosts: 'all',
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2024', 'chrome120', 'safari17'], // Mis à jour vers ES2024 par le script update-metadata.js,
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
})
