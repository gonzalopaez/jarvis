import "./styles.css";
import { JarvisStore } from "./core/state";
import type { JarvisState } from "./core/types";
import { CoreClient } from "./core/client";
import { MockAgentAdapter } from "./agents/mock-adapter";
import { TelemetryClient } from "./telemetry/client";
import { appTemplate } from "./ui/template";
import { JarvisView } from "./ui/view";
import { RuntimeCapabilityError } from "./runtime/client";
import { createRuntimeClient } from "./runtime/environment";
import { RealtimeClient } from "./realtime/client";
import { MicrophoneAudioVisualizerSource } from "./audio/microphone-source";
import { VoiceCaptureClient } from "./voice/client";
import { TtsPlayback } from "./audio/tts-playback";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("JARVIS application root not found");
app.innerHTML = appTemplate;

const hud = app.querySelector<HTMLElement>(".hud");
if (!hud) throw new Error("JARVIS HUD root not found");

const store = new JarvisStore();
const view = new JarvisView(hud);
const runtime = createRuntimeClient();
const telemetry = new TelemetryClient(store.bus, runtime);
const core = new CoreClient(store.bus, runtime);
const realtime = new RealtimeClient(store.bus, runtime);
const microphone = new MicrophoneAudioVisualizerSource();
const mock = new MockAgentAdapter(store);
const stateSequence: Array<{ state: JarvisState; label?: string }> = [
  { state: "idle" }, { state: "listening" }, { state: "thinking" }, { state: "routing" },
  { state: "executing", label: "CODEX ANALYZING" },
  { state: "executing", label: "CODEX EXECUTING" },
  { state: "executing", label: "N8N EXECUTING" },
  { state: "speaking" }, { state: "authorization-required" }, { state: "warning" },
  { state: "error" }, { state: "offline" },
];

store.subscribe((model) => view.render(model));
store.addActivity("JARVIS", "UI CORE INITIALIZED", "success");
store.addActivity("EVENT BUS", "LOCAL CHANNEL ONLINE", "success");
store.addActivity("AGENTS", "PROVIDER-NEUTRAL MOCKS LOADED", "info", true);
store.addActivity("TELEMETRY", "REQUESTING RUST DATA LINK", "info");
store.addActivity("CORE", "NEGOTIATING HTTPS DATA LINK", "info");
store.addActivity("CLIENT", `${runtime.kind.toUpperCase()} RUNTIME DETECTED`, "success");

const devControls = app.querySelector<HTMLElement>("#dev-controls");
stateSequence.forEach(({ state, label }) => {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.state = state;
  button.textContent = label ?? state.replace(/-/g, " ").toUpperCase();
  button.addEventListener("click", () => mock.trigger(state, label));
  devControls?.append(button);
});

if (["127.0.0.1", "localhost", "::1"].includes(window.location.hostname)) {
  const visualTestState = new URLSearchParams(window.location.search).get("state") as JarvisState | null;
  if (visualTestState && stateSequence.some(({ state }) => state === visualTestState)) {
    mock.trigger(visualTestState, `VISUAL TEST ${visualTestState.toUpperCase()}`);
  }
}

app.querySelector("#dev-toggle")?.addEventListener("click", () => store.toggleDeveloperControls());

const loginModal = app.querySelector<HTMLElement>("#login-modal");
const sessionButton = app.querySelector<HTMLButtonElement>("#session-toggle");
const accessKey = app.querySelector<HTMLInputElement>("#access-key");
const loginError = app.querySelector<HTMLElement>("#login-error");
function showLogin(show: boolean): void {
  loginModal?.classList.toggle("is-visible", show);
  loginModal?.setAttribute("aria-hidden", String(!show));
  if (!show && accessKey) accessKey.value = "";
  if (show) accessKey?.focus();
}
sessionButton?.addEventListener("click", async () => {
  if (await runtime.hasSession()) {
    await runtime.logout();
    realtime.stop();
    sessionButton.textContent = "CONNECT";
    store.addActivity("SECURITY", "BROWSER SESSION CLOSED", "info", true);
  } else showLogin(true);
});
app.querySelector("#cancel-login")?.addEventListener("click", () => showLogin(false));
app.querySelector<HTMLFormElement>("#login-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!accessKey?.value) return;
  if (loginError) loginError.textContent = "";
  try {
    await runtime.login(accessKey.value);
    accessKey.value = "";
    showLogin(false);
    if (sessionButton) sessionButton.textContent = "DISCONNECT";
    store.addActivity("SECURITY", "AUTHENTICATED SESSION ESTABLISHED", "success", true);
    await realtime.start();
  } catch {
    accessKey.value = "";
    if (loginError) loginError.textContent = "Authentication rejected.";
    accessKey.focus();
  }
});

const microphoneButton = app.querySelector<HTMLButtonElement>("#microphone-toggle");
let microphoneActive = false;
const ttsPlayback = new TtsPlayback();
const voiceCapture = new VoiceCaptureClient(runtime, {
  unavailable: (code) => {
    microphone.stop();
    microphoneActive = false;
    microphoneButton?.setAttribute("aria-pressed", "false");
    store.clearAudioLevel();
    store.setState("warning");
    store.setTranscript("jarvis", code === "STT_UNAVAILABLE"
      ? "Voice transport is online; speech recognition is not connected yet."
      : "Voice processing is unavailable.");
    store.addActivity("VOICE", code, "warning");
  },
  failed: (message) => {
    store.setState("warning");
    store.addActivity("VOICE", message, "warning");
  },
  processing: () => {
    store.clearAudioLevel();
    store.setState("thinking");
    store.addActivity("VOICE", "SPEECH PROCESSING", "info");
  },
  transcript: (text) => store.setTranscript("user", text),
  response: (text) => store.setTranscript("jarvis", text),
  output: (audio) => {
    store.setState("speaking");
    store.addActivity("VOICE", "SYNTHESIZED RESPONSE PLAYBACK", "success");
    void ttsPlayback.play(audio, (level) => store.bus.emit("voice.output.level", { level }))
      .then(() => { store.clearAudioLevel(); store.setState("idle"); })
      .catch(() => { store.clearAudioLevel(); store.setState("warning"); store.addActivity("VOICE", "AUDIO PLAYBACK FAILED", "warning"); });
  },
});
microphoneButton?.addEventListener("click", async () => {
  if (microphoneActive) {
    await voiceCapture.stop();
    microphone.stop();
    microphoneActive = false;
    microphoneButton.setAttribute("aria-pressed", "false");
    microphoneButton.setAttribute("aria-label", "Start microphone capture");
    store.clearAudioLevel();
    store.addActivity("VOICE", "MICROPHONE CAPTURE STOPPED", "info");
    return;
  }
  try {
    if (runtime.kind === "browser" && !await runtime.hasSession()) {
      store.setTranscript("jarvis", "Connect an authenticated session before opening the microphone.");
      showLogin(true);
      return;
    }
    await microphone.start(({ level }) => store.bus.emit("voice.input.level", { level }));
    const stream = microphone.mediaStream();
    if (!stream) throw new Error("Microphone stream is unavailable");
    await voiceCapture.start(stream);
    microphoneActive = true;
    microphoneButton.setAttribute("aria-pressed", "true");
    microphoneButton.setAttribute("aria-label", "Stop microphone capture");
    store.setState("listening");
    store.addActivity("VOICE", "MICROPHONE CAPTURE ACTIVE", "success");
  } catch {
    microphone.stop();
    store.setTranscript("jarvis", "Microphone permission was denied or capture is unavailable.");
    store.setState("warning");
    store.addActivity("VOICE", "MICROPHONE CAPTURE REJECTED", "warning");
  }
});

app.querySelector<HTMLFormElement>("#command-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const input = app.querySelector<HTMLInputElement>("#command-input");
  const instruction = input?.value.trim();
  if (!instruction) return;
  store.setTranscript("user", instruction);
  store.addActivity("MANUAL INPUT", "INSTRUCTION RECEIVED", "info");
  store.setState("thinking");
  if (input) input.value = "";
  if (input) input.disabled = true;
  try {
    store.setState("routing");
    const response = await core.conversation(instruction);
    store.setTranscript("jarvis", response.message);
    store.addActivity("CORE", `RESPONSE VERIFIED / ${response.auditId}`, "success");
    store.addActivity("CORE", `RESPONSE MODE: ${response.mode.toUpperCase()}`, "info", response.mode === "mock");
    store.setState("speaking");
    window.setTimeout(() => store.setState("idle"), 1800);
  } catch (error) {
    if (error instanceof RuntimeCapabilityError) {
      store.setTranscript("jarvis", "Browser command sessions will be enabled after the authenticated API phase.");
      store.addActivity("CLIENT", error.message, "warning");
      store.setState("warning");
      return;
    }
    store.bus.emit("core.health.failed", { message: String(error) });
    store.setTranscript("jarvis", "Core data link is unavailable. No action was executed.");
    store.setState("offline");
  } finally {
    if (input) input.disabled = false;
  }
});

app.querySelector("#approve-action")?.addEventListener("click", () => {
  store.bus.emit("authorization.approved", { action: "demo.protected_action" });
  store.addActivity("SECURITY", "OPERATION AUTHORIZED / DEMO ONLY", "success", true);
  store.setTranscript("jarvis", "Authorization recorded. No command was executed in demonstration mode.");
  store.setState("executing", true);
  window.setTimeout(() => store.setState("idle", true), 1800);
});

app.querySelector("#deny-action")?.addEventListener("click", () => {
  store.bus.emit("authorization.denied", { action: "demo.protected_action" });
  store.addActivity("SECURITY", "OPERATION DENIED", "warning", true);
  store.setTranscript("jarvis", "Operation denied. System state was not changed.");
  store.setState("idle", true);
});

function updateClock(): void {
  const now = new Date();
  const clock = app?.querySelector<HTMLElement>("#clock");
  const date = app?.querySelector<HTMLElement>("#date");
  if (clock) clock.textContent = now.toLocaleTimeString("es-AR", { hour12: false });
  if (date) {
    date.textContent = now.toLocaleDateString("en-GB", {
      day: "2-digit", month: "short", year: "numeric",
    }).toUpperCase();
  }
}

updateClock();
window.setInterval(updateClock, 1000);
telemetry.start();
core.start();
void realtime.start();
void runtime.hasSession().then((authenticated) => {
  if (sessionButton) sessionButton.textContent = authenticated ? "DISCONNECT" : "CONNECT";
});

window.addEventListener("beforeunload", () => {
  telemetry.stop();
  core.stop();
  realtime.stop();
  void voiceCapture.stop();
  microphone.stop();
  ttsPlayback.stop();
  view.destroy();
});
