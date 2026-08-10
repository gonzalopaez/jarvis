import type { CoreConversation, CoreHealth, TelemetrySnapshot } from "../core/types";

export type RuntimeKind = "browser" | "tauri";

export interface JarvisRuntimeClient {
  readonly kind: RuntimeKind;
  coreHealth(): Promise<CoreHealth>;
  conversation(message: string): Promise<CoreConversation>;
  telemetry(): Promise<TelemetrySnapshot>;
  hasSession(): Promise<boolean>;
  login(accessKey: string): Promise<void>;
  logout(): Promise<void>;
  websocketUrl(location?: Pick<Location, "protocol" | "host">): string;
  voiceWebsocketUrl(location?: Pick<Location, "protocol" | "host">): string;
}

export class RuntimeCapabilityError extends Error {
  constructor(
    readonly capability: "conversation" | "telemetry",
    message: string,
  ) {
    super(message);
    this.name = "RuntimeCapabilityError";
  }
}
