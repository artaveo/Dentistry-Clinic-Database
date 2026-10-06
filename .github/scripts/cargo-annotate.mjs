// Reads `cargo <...> --message-format=json` from stdin (mixed with any
// plain-text program output, e.g. from `cargo run`/`cargo test`, which is
// simply skipped). For every compiler error/warning, prints the normal
// human-readable text to stdout (so the raw log still reads like a regular
// build) *and* a `::error file=...,line=...,col=...::message` /
// `::warning ...::...` workflow command, which GitHub turns into a real
// check-run annotation — readable afterwards via the public
// `GET /repos/{owner}/{repo}/check-runs/{id}/annotations` API, no sign-in
// needed. This is more reliable than a `::add-matcher::` problem matcher,
// which only highlights the log and does not reliably produce check-run
// annotations.
//
// Usage: cargo <...> --message-format=json 2>&1 | node cargo-annotate.mjs
import { createInterface } from "node:readline";

const esc = (s) => String(s).replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
let panicAt = null; // a test panic whose message lines are still being read

for await (const line of rl) {
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    console.log(line); // not JSON: cargo's own plain-text progress, or a ran program's own output
    // A failing test prints "thread '…' panicked at FILE:LINE:COL:" and the assertion details below it.
    // Turn that into an annotation too, so the failure is readable without opening the log.
    const panic = /panicked at (.+?):(\d+):\d+:?\s*$/.exec(line);
    if (panic) {
      panicAt = { file: panic[1], line: panic[2], lines: [] };
    } else if (panicAt && panicAt.lines.length < 4) {
      if (line.trim() === "") {
        console.log(`::error file=${panicAt.file},line=${panicAt.line}::${esc(`test panicked: ${panicAt.lines.join(" | ")}`)}`);
        panicAt = null;
      } else {
        panicAt.lines.push(line.trim());
      }
    }
    continue;
  }
  if (msg.reason !== "compiler-message" || !msg.message) continue;
  const m = msg.message;
  if (m.rendered) process.stdout.write(m.rendered);
  if (m.level !== "error" && m.level !== "warning") continue;
  const span = (m.spans || []).find((s) => s.is_primary) ?? m.spans?.[0];
  const cmd = m.level === "error" ? "error" : "warning";
  const text = esc(m.message);
  if (span) {
    console.log(`::${cmd} file=${span.file_name},line=${span.line_start},col=${span.column_start}::${text}`);
  } else {
    console.log(`::${cmd}::${text}`);
  }
}
// cargo's own exit code (not this script's) is what must fail the step —
// the caller pipes with `set -o pipefail` so that still happens.
