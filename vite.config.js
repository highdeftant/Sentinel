import { defineConfig } from "vite";

export default defineConfig(() => ({
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 11500,
    strictPort: true,
    allowedHosts: ["ai"],
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome105", "safari13"],
    minify: process.env.TAURI_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_DEBUG),
  },
}));
