import { fileURLToPath, URL } from 'node:url'
import { readFileSync } from 'node:fs'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST
const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf-8'))

export default defineConfig(async () => ({
  plugins: [vue()],

  define: {
    'import.meta.env.PACKAGE_VERSION': JSON.stringify(pkg.version),
  },

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'naive-ui': ['naive-ui'],
          'tauri-api': [
            '@tauri-apps/api',
            '@tauri-apps/plugin-dialog',
            '@tauri-apps/plugin-opener',
          ],
          'vue-vendor': ['vue', 'vue-router', 'pinia'],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
}))
