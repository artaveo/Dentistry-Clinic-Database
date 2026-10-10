import { defineConfig } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

// E2E runs the real Rust Core (artaveo-dev-server, same router as Tauri IPC)
// against a fresh data folder, and the production UI build.
const dataDir = path.resolve(".e2e-data");
const exe = process.platform === "win32" ? "artaveo-dev-server.exe" : "artaveo-dev-server";
const server = process.env.ARTAVEO_DEV_SERVER ?? path.resolve("../target/debug", exe);

// The whole suite runs at 10:00 in the clinic (Kabul, UTC+04:30, no DST) on today's date, whatever
// hour CI starts: bookings "today at 9 PM", the default "next quarter hour", the 19:00 daily backup
// and the like would otherwise pass or fail by the time of day. The Core's clock (dev server) and
// every page's clock (e2e/clinicClock.ts) are moved by the same amount. Computed once here and
// inherited by the test workers through the environment.
const KABUL_MS = 4.5 * 3_600_000;
if (!process.env.ARTAVEO_E2E_CLOCK_SHIFT_MS) {
  const now = Date.now();
  const kabul = new Date(now + KABUL_MS);
  const tenAm = Date.UTC(kabul.getUTCFullYear(), kabul.getUTCMonth(), kabul.getUTCDate(), 10, 0) - KABUL_MS;
  process.env.ARTAVEO_E2E_CLOCK_SHIFT_MS = String(tenAm - now);
}

const chrome = [process.env.CHROME_PATH, "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"].find((p) => p && fs.existsSync(p));

export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  workers: 1,
  // On CI, failures also become GitHub annotations (readable without downloading the report).
  reporter: [...(process.env.CI ? [["github"] as const] : []), ["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
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
      env: { ARTAVEO_ENV: "test", ARTAVEO_E2E_CLOCK_SHIFT_MS: process.env.ARTAVEO_E2E_CLOCK_SHIFT_MS },
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
