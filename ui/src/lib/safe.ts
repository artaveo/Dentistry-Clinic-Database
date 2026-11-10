// Safe Startup Mode (roadmap 11.3): what only the desktop shell can do when the clinic does not open.
import { isDesktop } from "../print/engine";

export type StartupState =
  | { safe: false }
  | { safe: true; reason: string; detail: string; recovering: boolean; can_recover: boolean };

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function startupState(): Promise<StartupState> {
  if (!isDesktop()) return { safe: false };
  try {
    return await invoke<StartupState>("startup_state");
  } catch {
    return { safe: false };
  }
}

export const safeRetry = () => invoke<StartupState>("safe_retry");
export const safeEnterRestore = () => invoke<void>("safe_enter_restore");
export const safeReport = () => invoke<{ file_name: string; path: string }>("safe_report");
export const safeRecover = () =>
  invoke<{ ok: boolean; file_name: string; created_at: string | null; patients: number }>("safe_recover");

export type UpdateInfo = { available: false } | { available: true; version: string; current: string; notes: string | null };
export const updateCheck = () => invoke<UpdateInfo>("update_check");
export const updateInstall = () => invoke<void>("update_install");
