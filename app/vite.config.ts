import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev port and serves the built files from dist/.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Tauri loads the dev server from 5173; a PORT override is for browser previews only.
  server: { port: Number(process.env.PORT) || 5173, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "es2022", outDir: "dist" },
});
