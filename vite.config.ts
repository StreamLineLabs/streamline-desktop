import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // Tauri expects a fixed port so it can load the dev server
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // Tauri uses Chromium on Windows/Linux and WebKit on macOS
    target: ["es2021", "chrome100", "safari15"],
    outDir: "dist",
  },
});
