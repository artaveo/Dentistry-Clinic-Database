/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built files from ../app (frontendDist = ../ui/dist).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { host: "127.0.0.1", port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
  // `e2e/` holds Playwright specs (run with `npm run e2e`), not Vitest unit tests.
  test: { include: ["src/**/*.test.ts"], environment: "node" },
});
