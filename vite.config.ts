import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri drives this dev server, so the port is fixed and failure to bind must
// be loud rather than silently moving to another port the shell won't find.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // The Rust side has its own rebuild loop; watching it here just burns CPU.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
  build: {
    outDir: "dist",
    rollupOptions: {
      // Three windows, three pages. The overlay is deliberately separate: it is
      // shown and hidden constantly and must not carry the settings window's
      // code around with it. Onboarding is seen once and then never again.
      // Relative to the project root, so no node types are needed here.
      input: {
        main: "index.html",
        overlay: "overlay.html",
        onboarding: "onboarding.html",
      },
    },
    // Klar targets a known WebView2 / WKWebView, not the open web.
    target: "es2022",
    sourcemap: true,
  },
});
