import { invoke } from "@tauri-apps/api/core";
import type { CoreConversation, CoreHealth, TelemetrySnapshot } from "../core/types";
import type { JarvisRuntimeClient } from "./client";

export class TauriRuntimeClient implements JarvisRuntimeClient {
  readonly kind = "tauri" as const;

  coreHealth(): Promise<CoreHealth> {
    return invoke<CoreHealth>("get_core_health");
  }

  conversation(message: string): Promise<CoreConversation> {
    return invoke<CoreConversation>("send_core_conversation", { message });
  }

  telemetry(): Promise<TelemetrySnapshot> {
    return invoke<TelemetrySnapshot>("get_system_telemetry");
  }

  async hasSession(): Promise<boolean> {
    return false;
  }

  async login(): Promise<void> {
    throw new Error("Native runtime does not use browser sessions");
  }

  async logout(): Promise<void> {
    throw new Error("Native runtime does not use browser sessions");
  }

  websocketUrl(): string {
    throw new Error("The transitional Tauri runtime does not use browser WebSocket sessions");
  }

  voiceWebsocketUrl(): string {
    throw new Error("The native compatibility runtime does not use browser voice WebSockets");
  }
}
