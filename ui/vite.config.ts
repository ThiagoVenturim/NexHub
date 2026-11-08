/// <reference types="vitest/config" />
import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "esnext",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: Boolean(process.env.TAURI_DEBUG),
  },
  test: {
    environment: "node",
  },
});
