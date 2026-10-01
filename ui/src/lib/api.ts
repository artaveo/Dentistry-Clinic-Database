// The only way the UI talks to the Core (ADR-01/02). Types come from the
// generated contract; the transport is Tauri IPC in the desktop app and
// plain HTTP to the dev server in the browser (development / E2E tests).
import type { Api, ErrorCode, RpcRequest, RpcResponse } from "../../../shared/ts/contract";

export type Method = keyof Api;
export type Params<M extends Method> = Api[M]["params"];
export type Result<M extends Method> = Api[M]["result"];

export class ApiError extends Error {
  constructor(public code: ErrorCode, public detail: string) {
    super(`${code}: ${detail}`);
  }
}

const DEV_URL = import.meta.env.VITE_RPC_URL ?? "http://127.0.0.1:8787/rpc";

async function transport(request: RpcRequest): Promise<RpcResponse> {
  if ("__TAURI_INTERNALS__" in window) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<RpcResponse>("rpc", { request });
  }
  const res = await fetch(DEV_URL, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(request) });
  return (await res.json()) as RpcResponse;
}

let token: string | null = null;
const listeners = new Set<(e: ApiError) => void>();

export function setToken(t: string | null) {
  token = t;
}

/** Session-level errors (locked/expired) are broadcast so the shell can react. */
export function onSessionError(fn: (e: ApiError) => void) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export async function rpc<M extends Method>(method: M, params: Params<M>): Promise<Result<M>> {
  let response: RpcResponse;
  try {
    response = await transport({ method, params, token });
  } catch (e) {
    throw new ApiError("internal", String(e));
  }
  if (response.status === "ok") return response.result as Result<M>;
  const err = new ApiError(response.error.code, response.error.detail);
  if (err.code === "session_locked" || err.code === "session_expired") listeners.forEach((fn) => fn(err));
  throw err;
}
