import "./styles.css";
import { JarvisStore } from "./core/state";
import type { JarvisState } from "./core/types";
import { MockAgentAdapter } from "./agents/mock-adapter";
import { TelemetryClient } from "./telemetry/client";
import { appTemplate } from "./ui/template";
import { JarvisView } from "./ui/view";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("JARVIS application root not found");
app.innerHTML = appTemplate;

const hud = app.querySelector<HTMLElement>(".hud");
if (!hud) throw new Error("JARVIS HUD root not found");

const store = new JarvisStore();
const view = new JarvisView(hud);
const telemetry = new TelemetryClient(store.bus);
const mock = new MockAgentAdapter(store);
const stateSequence: JarvisState[] = [
  "idle", "listening", "thinking", "routing", "codex-analyzing", "codex-executing",
  "n8n-executing", "speaking", "warning", "authorization-required", "error", "offline",
];

store.subscribe((model) => view.render(model));
store.addActivity("JARVIS", "UI CORE INITIALIZED", "success");
store.addActivity("EVENT BUS", "LOCAL CHANNEL ONLINE", "success");
store.addActivity("AGENTS", "PROVIDER-NEUTRAL MOCKS LOADED", "info", true);
store.addActivity("TELEMETRY", "REQUESTING RUST DATA LINK", "info");

const devControls = app.querySelector<HTMLElement>("#dev-controls");
stateSequence.forEach((state) => {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.state = state;
  button.textContent = state.replace(/-/g, " ").toUpperCase();
  button.addEventListener("click", () => mock.trigger(state));
  devControls?.append(button);
});

app.querySelector("#dev-toggle")?.addEventListener("click", () => store.toggleDeveloperControls());

app.querySelector<HTMLFormElement>("#command-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  const input = app.querySelector<HTMLInputElement>("#command-input");
  const instruction = input?.value.trim();
  if (!instruction) return;
  store.setTranscript("user", instruction);
  store.addActivity("MANUAL INPUT", "INSTRUCTION RECEIVED", "info", true);
  store.setState("thinking", true);
  window.setTimeout(() => {
    store.setState("routing", true);
    store.setTranscript("jarvis", "Mock router received the instruction. No external service was contacted.");
  }, 800);
  window.setTimeout(() => store.setState("idle", true), 2200);
  if (input) input.value = "";
});

app.querySelector("#approve-action")?.addEventListener("click", () => {
  store.bus.emit("authorization.approved", { action: "demo.protected_action" });
  store.addActivity("SECURITY", "OPERATION AUTHORIZED / DEMO ONLY", "success", true);
  store.setTranscript("jarvis", "Authorization recorded. No command was executed in demonstration mode.");
  store.setState("codex-executing", true);
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

window.addEventListener("beforeunload", () => {
  telemetry.stop();
  view.destroy();
});
