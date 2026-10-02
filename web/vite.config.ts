import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

// The backend the dev and preview servers proxy to: the server's default port,
// unless SERVER_URL says otherwise (Playwright sets it).
const serverUrl = process.env.SERVER_URL ?? 'http://127.0.0.1:3100'
// Anchored like the service worker's denylist below, so /apis or /healthy
// stay app routes everywhere.
const proxy = {
  '^/health(\\?|$)': serverUrl,
  '^/api/': serverUrl,
  '^/xrpc/': serverUrl,
  '^/oauth/': serverUrl,
  '^/oauth-client-metadata\\.json(\\?|$)': serverUrl,
}

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      // Backend routes must reach the server, never the cached app shell.
      workbox: {
        navigateFallbackDenylist: [
          /^\/api\//,
          /^\/xrpc\//,
          /^\/health(\?|$)/,
          /^\/oauth\//,
          /^\/oauth-client-metadata\.json(\?|$)/,
        ],
      },
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
