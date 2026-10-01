import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built files from ../app (frontendDist = ../ui/dist).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
