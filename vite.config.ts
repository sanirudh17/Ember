import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Ember runs inside a Tauri v2 WebView2 window. The dev server is fixed on
// port 1420 so tauri.conf.json's devUrl never drifts.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome110",
    // Vite 8 minifies with oxc by default; esbuild is no longer bundled, so
    // asking for it explicitly would require a separate install.
    minify: true,
    sourcemap: false,
  },
});
