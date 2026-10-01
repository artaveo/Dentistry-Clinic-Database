import fs from "node:fs";
import { chromium } from "playwright-core";

/**
 * On Windows we deliberately use Microsoft Edge — the same Chromium build
 * family as the WebView2 runtime the app ships with. Elsewhere use
 * CHROME_PATH or the sandbox's pre-installed Chromium.
 */
export async function launch() {
  if (process.platform === "win32") return chromium.launch({ channel: "msedge" });
  const candidates = [process.env.CHROME_PATH, "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", "/usr/bin/chromium", "/usr/bin/google-chrome"];
  const executablePath = candidates.find((p) => p && fs.existsSync(p));
  return chromium.launch(executablePath ? { executablePath } : {});
}
