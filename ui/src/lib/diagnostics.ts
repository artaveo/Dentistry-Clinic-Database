// Support goal (roadmap 11.3/11.4): when something breaks on a clinic's computer, the log says where.
// Errors the window itself hits (a screen that crashed, a failed promise) are sent to the same log as
// the Core's, tagged with the screen the user was on. Answers the Core rejects on purpose (a wrong
// password, a validation message) are normal traffic and are not logged here.
import { ApiError } from "./api";
import { isDesktop } from "../print/engine";

let currentPage = "start";

/** The shell tells which screen is open, so every logged error carries it. */
export function setCurrentPage(page: string) {
  currentPage = page;
}

export function reportUiError(place: string, error: unknown) {
  if (error instanceof ApiError && error.code !== "internal") return;
  const message = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack : undefined;
  console.error(`[${place}]`, error);
  if (!isDesktop()) return;
  import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("log_ui_error", { place: `${place} @ ${currentPage}`, message, stack }))
    .catch(() => {
      /* the log is best effort: it must never cause a second error */
    });
}

export function installGlobalErrorLogging() {
  window.addEventListener("error", (e) => reportUiError("window.error", e.error ?? e.message));
  window.addEventListener("unhandledrejection", (e) => reportUiError("unhandled promise", e.reason));
}

/** The six-character reference the Core puts on an internal error, for the phone call to support. */
export function errorReference(e: unknown): string | null {
  if (!(e instanceof ApiError)) return null;
  const m = /\[ref ([A-Z0-9]{6})\]/.exec(e.detail);
  return m ? m[1] : null;
}
