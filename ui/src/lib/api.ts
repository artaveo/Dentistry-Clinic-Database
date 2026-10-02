// The only way the UI talks to the Core (ADR-01/02). Types come from the
// generated contract; the transport is Tauri IPC in the desktop app and
// plain HTTP to the dev server in the browser (development / E2E tests).
import type { Api, ErrorCode, RpcRequest, RpcResponse, ValidationRule } from "../../../shared/ts/contract";

export type Method = keyof Api;
export type Params<M extends Method> = Api[M]["params"];
export type Result<M extends Method> = Api[M]["result"];

export class ApiError extends Error {
  constructor(
    public code: ErrorCode,
    public detail: string,
    /** The request parameter at fault (OF-002), shown under that input. */
    public field: string | null = null,
    public rule: ValidationRule | null = null,
  ) {
    super(`${code}: ${detail}`);
  }
}

const DEV_URL = import.meta.env.VITE_RPC_URL ?? "http://127.0.0.1:8787/rpc";

async function transport(request: RpcRequest): Promise<RpcResponse> {
  // Development-only simulated Core (src/dev/mockCore.ts); statically false — and
  // removed — in every normal build.
  if (import.meta.env.VITE_MOCK === "1") {
    const { mockTransport } = await import("../dev/mockCore");
    return mockTransport(request);
  }
  if ("__TAURI_INTERNALS__" in window) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<RpcResponse>("rpc", { request });
  }
  const res = await fetch(DEV_URL, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(request) });
  return (await res.json()) as RpcResponse;
}

let token: string | null = null;
const listeners = new Set<(e: ApiError) => void>();
/**
 * Bumped on every successful unlock/login. A `session_locked` answer to a
 * request sent *before* that (still in flight while the user typed the
 * password) is stale and must not lock the screen again — OF-008.
 */
let sessionEpoch = 0;

export function setToken(t: string | null) {
  token = t;
  sessionEpoch++;
}

/** Session-level errors (locked/expired) are broadcast so the shell can react. */
export function onSessionError(fn: (e: ApiError) => void) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Fired after the auto-lock time changes, so the Shell re-checks the lock at once (OF-012). */
export const SESSION_CHECK_EVENT = "artaveo:session-check";

export function isSessionError(e: unknown): boolean {
  return e instanceof ApiError && (e.code === "session_locked" || e.code === "session_expired" || e.code === "unauthenticated");
}

export async function rpc<M extends Method>(method: M, params: Params<M>): Promise<Result<M>> {
  const sentEpoch = sessionEpoch;
  let response: RpcResponse;
  try {
    response = await transport({ method, params, token });
  } catch (e) {
    throw new ApiError("internal", String(e));
  }
  if (response.status === "ok") {
    if (method === "session.unlock") sessionEpoch++;
    return response.result as Result<M>;
  }
  const { code, detail, field, rule } = response.error;
  const err = new ApiError(code, detail, field, rule);
  if ((code === "session_locked" || code === "session_expired") && sentEpoch === sessionEpoch) {
    listeners.forEach((fn) => fn(err));
  }
  throw err;
}
