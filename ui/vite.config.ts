/// <reference types="vitest/config" />
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  define: {
    ...(mode === "mock" ? { "import.meta.env.VITE_NEXHUB_MOCK": JSON.stringify("1") } : {}),
  },
  build: {
    target: "esnext",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: Boolean(process.env.TAURI_DEBUG),
  },
  test: {
    environment: "jsdom",
  },
}));
