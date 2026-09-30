import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

// The backend the dev and preview servers proxy to. Playwright starts one on this port.
const serverUrl = process.env.SERVER_URL ?? 'http://127.0.0.1:3100'
const proxy = {
  '/health': serverUrl,
  '/api': serverUrl,
  '/xrpc': serverUrl,
}

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      injectRegister: 'script',
      manifest: {
        name: 'Conference',
        short_name: 'Conference',
        start_url: '/',
        display: 'standalone',
        background_color: '#f4f6fb',
        theme_color: '#1f4ed8',
        icons: [
          { src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png' },
          { src: '/icons/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'any' },
        ],
      },
    }),
  ],
  server: { proxy },
  preview: { proxy },
})
