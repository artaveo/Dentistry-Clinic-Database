// One SemVer for the whole product (roadmap 1.1): Cargo workspace = UI package.
// Tauri reads its version from app/Cargo.toml (workspace), so it cannot drift.
import fs from "node:fs";

const cargo = fs.readFileSync("Cargo.toml", "utf8").match(/\[workspace\.package\][^[]*?version = "([^"]+)"/)[1];
const ui = JSON.parse(fs.readFileSync("ui/package.json", "utf8")).version;
const tauri = JSON.parse(fs.readFileSync("app/tauri.conf.json", "utf8")).version;
const semver = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const problems = [];
if (!semver.test(cargo)) problems.push(`workspace version ${cargo} is not SemVer`);
if (ui !== cargo) problems.push(`ui/package.json ${ui} != Cargo ${cargo}`);
if (tauri !== undefined) problems.push("app/tauri.conf.json must not set version (it comes from Cargo)");
const tag = process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : null;
if (tag && tag !== `v${cargo}`) problems.push(`tag ${tag} != v${cargo}`);
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`version ${cargo} ok`);
