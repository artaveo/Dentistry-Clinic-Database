// LAN mode (roadmap Phase 7): what only the desktop shell can do — find servers, pair with one, be
// started with Windows — and the live feed (changes made on other computers, connection state).
// The Core's own `lan.*` and `presence.*` methods go through `rpc` like everything else.
import { useEffect, useRef } from "react";
import type { LiveEvent } from "../../../shared/ts/contract";
import { isDesktop } from "../print/engine";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export type AppMode = { mode: "local" } | { mode: "client"; server_name: string; online: boolean };

/** Whether this computer works on its own database or through a clinic server. */
export async function appMode(): Promise<AppMode> {
  if (!isDesktop()) return { mode: "local" };
  try {
    return await invoke<AppMode>("app_mode");
  } catch {
    return { mode: "local" };
  }
}

export type FoundServer = { clinic_name: string; addr: string; fp_hint: string };

export async function discoverServers(): Promise<FoundServer[]> {
  return isDesktop() ? invoke<FoundServer[]>("lan_discover") : [];
}

export type PairFailure = "wrong_code" | "closed" | "limit_reached" | "unreachable" | "not_authentic" | "already_set_up";
export type PairResult = { ok: true; server_name: string } | { ok: false; reason: PairFailure; attempts_left?: number };

export function pairWithServer(addr: string, code: string, deviceName: string): Promise<PairResult> {
  return invoke<PairResult>("lan_pair", { addr, code, deviceName });
}

export function unpairFromServer(): Promise<void> {
  return invoke<void>("lan_unpair");
}

export async function autostartEnabled(): Promise<boolean | null> {
  if (!isDesktop()) return null;
  try {
    return await invoke<boolean>("autostart_get");
  } catch {
    return null;
  }
}

export function setAutostart(on: boolean): Promise<void> {
  return invoke<void>("autostart_set", { on });
}

// ── live feed ──

export type ConnectionState = { online: boolean; revoked: boolean; incompatible?: string };

/** The same events are also accepted as DOM events, so tests can simulate another computer. */
export const LIVE_DOM_EVENT = "artaveo:live";
export const CONNECTION_DOM_EVENT = "artaveo:connection";

function subscribe<T>(tauriEvent: string, domEvent: string, fn: (payload: T) => void): () => void {
  let off: (() => void) | null = null;
  let cancelled = false;
  if (isDesktop()) {
    import("@tauri-apps/api/event")
      .then(({ listen }) => listen<T>(tauriEvent, (e) => fn(e.payload)))
      .then((unlisten) => {
        if (cancelled) unlisten();
        else off = unlisten;
      })
      .catch(() => {});
  }
  const dom = (e: Event) => fn((e as CustomEvent<T>).detail);
  window.addEventListener(domEvent, dom);
  return () => {
    cancelled = true;
    off?.();
    window.removeEventListener(domEvent, dom);
  };
}

export function onLive(fn: (e: LiveEvent) => void): () => void {
  return subscribe<LiveEvent>("artaveo://live", LIVE_DOM_EVENT, fn);
}

export function onConnection(fn: (s: ConnectionState) => void): () => void {
  return subscribe<ConnectionState>("artaveo://connection", CONNECTION_DOM_EVENT, fn);
}

/** A request found the clinic server unreachable (the connection banner reacts at once). */
export const UNREACHABLE_DOM_EVENT = "artaveo:server-unreachable";

/**
 * Reloads what a page shows when another computer (or user) changes something it displays
 * (roadmap 7.3). `topics` are the Core's method prefixes (`appointments`, `patients`, …);
 * none means any change. Bursts are merged into one reload.
 */
export function useLive(topics: string[], reload: () => void) {
  const latest = useRef(reload);
  latest.current = reload;
  const key = topics.join(",");
  useEffect(() => {
    let timer = 0;
    const off = onLive((e) => {
      if (topics.length && !topics.includes(e.topic)) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => latest.current(), 300);
    });
    return () => {
      off();
      window.clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
