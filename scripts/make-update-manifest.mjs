// Builds latest.json for the Tauri updater (roadmap 11.2) from the signed installers of one release.
// Usage: node scripts/make-update-manifest.mjs <dist-folder> <tag> <github-repo>
// Reads ArtaveoDental-Setup-<arch>.exe.sig for x64, x86 and arm64; writes <dist-folder>/latest.json.
// Prints nothing and writes nothing when no signature exists (the release was built without the
// signing key), so a release without updater support still publishes.
import fs from "node:fs";
import path from "node:path";

const [dist, tag, repo] = process.argv.slice(2);
if (!dist || !tag || !repo) {
  console.error("usage: make-update-manifest.mjs <dist> <tag> <owner/repo>");
  process.exit(2);
}
const version = tag.replace(/^v/, "");
const targets = { x64: "windows-x86_64", x86: "windows-i686", arm64: "windows-aarch64" };

const platforms = {};
for (const [arch, target] of Object.entries(targets)) {
  const exe = `ArtaveoDental-Setup-${arch}.exe`;
  const sig = path.join(dist, `${exe}.sig`);
  if (!fs.existsSync(sig)) continue;
  platforms[target] = {
    signature: fs.readFileSync(sig, "utf8").trim(),
    url: `https://github.com/${repo}/releases/download/${tag}/${exe}`,
  };
}
if (Object.keys(platforms).length === 0) {
  console.log("no signed installers: latest.json not written");
  process.exit(0);
}
const notesFile = path.join("docs", "releases", `${tag}.md`);
const manifest = {
  version,
  notes: fs.existsSync(notesFile) ? fs.readFileSync(notesFile, "utf8").split("\n")[0].replace(/^\*+|\*+$/g, "") : "",
  pub_date: new Date().toISOString(),
  platforms,
};
fs.writeFileSync(path.join(dist, "latest.json"), JSON.stringify(manifest, null, 2) + "\n");
console.log(`latest.json for ${version}: ${Object.keys(platforms).join(", ")}`);
