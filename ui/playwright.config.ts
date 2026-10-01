import { defineConfig } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

// E2E runs the real Rust Core (artaveo-dev-server, same router as Tauri IPC)
// against a fresh data folder, and the production UI build.
const dataDir = path.resolve(".e2e-data");
const exe = process.platform === "win32" ? "artaveo-dev-server.exe" : "artaveo-dev-server";
const server = process.env.ARTAVEO_DEV_SERVER ?? path.resolve("../target/debug", exe);
const chrome = [process.env.CHROME_PATH, "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"].find((p) => p && fs.existsSync(p));

export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  workers: 1,
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    launchOptions: chrome ? { executablePath: chrome } : {},
  },
  webServer: [
    {
      command: `node -e "require('fs').rmSync(process.argv[1],{recursive:true,force:true})" "${dataDir}" && "${server}" --data-dir "${dataDir}" --port 8787`,
      url: "http://127.0.0.1:8787/health",
      env: { ARTAVEO_ENV: "test" },
      reuseExistingServer: false,
      timeout: 60_000,
    },
    {
      command: "npm run build && npm run preview",
      url: "http://127.0.0.1:4173",
      reuseExistingServer: false,
      timeout: 120_000,
    },
  ],
});
