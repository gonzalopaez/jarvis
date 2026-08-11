import { coreStateAndTranscript, jarvisCore, stateVoicePanel } from "./components/core";
import { securityAlertsPanel, securityTelemetryPanel } from "./components/security";

export const appTemplate = `
  <main class="hud" data-state="idle">
    <div class="ambient-grid" aria-hidden="true"></div>
    <div class="scanline" aria-hidden="true"></div>

    <header class="topbar">
      <div class="brand-block">
        <span class="eyebrow">STARK // LOCAL SYSTEMS</span>
        <div class="brand">J.A.R.V.I.S <span>v0.1</span></div>
      </div>
      <div class="top-readouts">
        <div class="readout"><span>NODE</span><strong id="host-name">INITIALIZING</strong></div>
        <div class="readout"><span>UPTIME</span><strong id="uptime">--:--:--</strong></div>
        <div class="clock-block"><strong id="clock">00:00:00</strong><span id="date">-- --- ----</span></div>
      </div>
      <div class="session-controls"><button class="window-control" id="session-toggle">CONNECT</button><button class="window-control" id="dev-toggle" aria-label="Alternar controles de desarrollo">DEV</button></div>
    </header>

    <section class="workspace">
      <aside class="left-rail panel-cut">
        <div class="section-heading"><span>01</span><div>SERVER CENTRAL<small>PROXMOX TELEMETRY</small></div></div>
        <div class="metric-stack">
          <article class="metric" data-metric="cpu">
            <div class="metric-orbit" style="--value:0"><span id="cpu-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>PROCESSOR</span><strong>CPU LOAD</strong><canvas class="metric-sparkline" id="cpu-sparkline" width="180" height="24"></canvas><div class="mini-track"><i id="cpu-track"></i></div></div>
          </article>
          <article class="metric" data-metric="memory">
            <div class="metric-orbit" style="--value:0"><span id="memory-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>VOLATILE MEMORY</span><strong id="memory-detail">-- / --</strong><canvas class="metric-sparkline" id="memory-sparkline" width="180" height="24"></canvas><div class="mini-track"><i id="memory-track"></i></div></div>
          </article>
          <article class="metric" data-metric="disk">
            <div class="metric-orbit" style="--value:0"><span id="disk-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>ROOT STORAGE</span><strong id="disk-detail">-- / --</strong><canvas class="metric-sparkline" id="disk-sparkline" width="180" height="24"></canvas><div class="mini-track"><i id="disk-track"></i></div></div>
          </article>
        </div>

        <div class="data-slab network-slab">
          <div class="slab-title">NETWORK I/O <span>REALTIME</span></div>
          <canvas id="network-canvas" width="420" height="92" aria-label="Historial de tráfico de red"></canvas>
          <div class="network-values">
            <div><span>RX</span><strong id="network-rx">--</strong></div>
            <div><span>TX</span><strong id="network-tx">--</strong></div>
          </div>
        </div>

        <div class="data-slab load-slab">
          <div><span>LOAD VECTOR</span><strong id="load-vector">-- / -- / --</strong></div>
          <div><span>KERNEL</span><strong id="kernel">--</strong></div>
        </div>

        ${securityTelemetryPanel()}
      </aside>

      <section class="core-stage">
        <div class="coordinate coordinate-n">N // 00</div>
        <div class="coordinate coordinate-e">E // 90</div>
        <div class="coordinate coordinate-s">S // 180</div>
        <div class="coordinate coordinate-w">W // 270</div>

        ${stateVoicePanel()}
        ${jarvisCore()}
        ${coreStateAndTranscript()}
      </section>

      <aside class="right-rail panel-cut">
        <div class="section-heading"><span>02</span><div>AGENT MATRIX<small>COMPONENT STATUS</small></div></div>
        <div class="agent-list" id="agent-list"></div>

        <div class="section-heading stream-heading"><span>03</span><div>ACTIVITY STREAM<small>STRUCTURED EVENTS</small></div></div>
        <div class="activity-stream" id="activity-stream"></div>

        ${securityAlertsPanel()}
      </aside>
    </section>

    <footer class="command-deck">
      <form id="command-form" class="command-line">
        <span>MANUAL INPUT</span>
        <input id="command-input" autocomplete="off" placeholder="Enter an instruction for Jarvis Core..." />
        <button type="submit">ROUTE</button>
      </form>
      <div class="dev-controls" id="dev-controls" aria-label="Controles de estados de demostración"></div>
    </footer>

    <section class="authorization-modal" id="authorization-modal" aria-hidden="true">
      <div class="auth-frame">
        <span class="auth-code">SECURITY GATE // LEVEL 03</span>
        <h2>AUTHORIZATION REQUIRED</h2>
        <p>Codex solicita ejecutar una operación protegida de demostración.</p>
        <div class="auth-command"><span>STRUCTURED ACTION</span><code>demo.protected_action</code></div>
        <div class="auth-impact"><span>IMPACT</span><p>Demonstrates policy and authorization flow. No executor is connected.</p></div>
        <div class="auth-actions"><button id="deny-action" class="deny">DENY</button><button id="approve-action">AUTHORIZE</button></div>
      </div>
    </section>

    <section class="authorization-modal" id="login-modal" aria-hidden="true">
      <form class="auth-frame" id="login-form">
        <span class="auth-code">LOCAL ACCESS // SECURE SESSION</span>
        <h2>CONNECT TO JARVIS</h2>
        <p>Enter the local Core access key. It is exchanged for an HttpOnly session and is not stored by the browser.</p>
        <label class="login-field"><span>ACCESS KEY</span><input id="access-key" type="password" autocomplete="current-password" minlength="32" maxlength="4096" required /></label>
        <p class="login-error" id="login-error" role="alert"></p>
        <div class="auth-actions"><button type="button" id="cancel-login" class="deny">CANCEL</button><button type="submit">CONNECT</button></div>
      </form>
    </section>
  </main>
`;
