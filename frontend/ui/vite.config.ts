import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
// Port is pinned + strict so tauri.conf.json's devUrl (http://localhost:5173)
// stays valid; if the port is taken we want a hard failure, not a silent shift.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          // Split katex into its own lazy chunk — only loaded when a markdown
          // file containing math is opened. Keeps katex out of the main bundle.
          if (id.includes('node_modules/katex')) return 'katex'
        },
      },
    },
  },
})
