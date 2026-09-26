import { defineConfig } from 'vite';

// The world server serves the built viewer at /app/. In development, Vite proxies the API to a
// running server (PROTOGAEA_API, by default http://127.0.0.1:8081).
export default defineConfig({
  base: '/app/',
  server: {
    proxy: { '/v0': process.env.PROTOGAEA_API ?? 'http://127.0.0.1:8081' },
  },
  build: { outDir: 'dist', chunkSizeWarningLimit: 1200 },
});
