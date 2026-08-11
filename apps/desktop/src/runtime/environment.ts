import type { JarvisRuntimeClient, RuntimeKind } from "./client";
import { TauriRuntimeClient } from "./tauri-client";
import { WebRuntimeClient } from "./web-client";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function detectRuntime(target: Pick<Window, "__TAURI_INTERNALS__"> = window): RuntimeKind {
  return target.__TAURI_INTERNALS__ ? "tauri" : "browser";
}

export function createRuntimeClient(kind: RuntimeKind = detectRuntime()): JarvisRuntimeClient {
  return kind === "tauri" ? new TauriRuntimeClient() : new WebRuntimeClient();
}
