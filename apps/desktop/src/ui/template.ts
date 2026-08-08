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
      <button class="window-control" id="dev-toggle" aria-label="Alternar controles de desarrollo">DEV</button>
    </header>

    <section class="workspace">
      <aside class="left-rail panel-cut">
        <div class="section-heading"><span>01</span><div>LOCAL TELEMETRY<small>LIVE LINUX DATA</small></div></div>
        <div class="metric-stack">
          <article class="metric" data-metric="cpu">
            <div class="metric-orbit" style="--value:0"><span id="cpu-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>PROCESSOR</span><strong>CPU LOAD</strong><div class="mini-track"><i id="cpu-track"></i></div></div>
          </article>
          <article class="metric" data-metric="memory">
            <div class="metric-orbit" style="--value:0"><span id="memory-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>VOLATILE MEMORY</span><strong id="memory-detail">-- / --</strong><div class="mini-track"><i id="memory-track"></i></div></div>
          </article>
          <article class="metric" data-metric="disk">
            <div class="metric-orbit" style="--value:0"><span id="disk-value">--</span><small>%</small></div>
            <div class="metric-copy"><span>ROOT STORAGE</span><strong id="disk-detail">-- / --</strong><div class="mini-track"><i id="disk-track"></i></div></div>
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
      </aside>

      <section class="core-stage">
        <div class="coordinate coordinate-n">N // 00</div>
        <div class="coordinate coordinate-e">E // 90</div>
        <div class="coordinate coordinate-s">S // 180</div>
        <div class="coordinate coordinate-w">W // 270</div>

        <div class="core-shell" aria-label="Núcleo de estado JARVIS">
          <div class="core-crosshair"></div>
          <div class="orbit orbit-1"><i></i><i></i><i></i></div>
          <div class="orbit orbit-2"><i></i><i></i></div>
          <div class="orbit orbit-3"></div>
          <svg class="core-svg" viewBox="0 0 600 600" aria-hidden="true">
            <defs>
              <filter id="soft-glow"><feGaussianBlur stdDeviation="3" result="blur"/><feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
            </defs>
            <circle class="radar-field" cx="300" cy="300" r="274"/>
            <circle class="ring ring-a" cx="300" cy="300" r="248"/>
            <circle class="ring ring-b" cx="300" cy="300" r="218"/>
            <circle class="ring ring-c" cx="300" cy="300" r="180"/>
            <circle class="ring ring-d" cx="300" cy="300" r="142"/>
            <circle class="ticks ticks-outer" cx="300" cy="300" r="262"/>
            <circle class="ticks ticks-inner" cx="300" cy="300" r="164"/>
            <path class="arc arc-a" d="M 103 300 A 197 197 0 0 1 203 130"/>
            <path class="arc arc-b" d="M 467 196 A 197 197 0 0 1 487 362"/>
            <path class="arc arc-c" d="M 217 452 A 175 175 0 0 1 131 340"/>
            <g class="radar-sweep"><path d="M300 300 L300 37 A263 263 0 0 1 388 52 Z"/></g>
          </svg>
          <div class="core-heart">
            <div class="heart-lattice"></div>
            <div class="heart-ring"></div>
            <div class="heart-energy"></div>
            <div class="heart-mark">J</div>
          </div>
          <div class="orbital-node node-a"><span>01</span></div>
          <div class="orbital-node node-b"><span>07</span></div>
          <div class="orbital-node node-c"><span>42</span></div>
        </div>

        <div class="state-banner">
          <span class="state-index">CORE STATE</span>
          <strong id="state-label">SYSTEM // STANDBY</strong>
          <i></i>
        </div>

        <div class="wave-module">
          <div class="wave-label"><span id="wave-mode">SIGNAL // AMBIENT</span><b id="wave-level">00.0 dB</b></div>
          <canvas id="wave-canvas" width="900" height="108" aria-label="Visualizador de voz simulado"></canvas>
        </div>

        <div class="transcript-module">
          <div class="transcript-line"><span>USER</span><p id="user-transcript">Awaiting operator input.</p></div>
          <div class="transcript-line jarvis-line"><span>JARVIS</span><p id="jarvis-transcript">Local system interface initialized.</p></div>
        </div>
      </section>

      <aside class="right-rail panel-cut">
        <div class="section-heading"><span>02</span><div>AGENT MATRIX<small>COMPONENT STATUS</small></div></div>
        <div class="agent-list" id="agent-list"></div>

        <div class="section-heading stream-heading"><span>03</span><div>ACTIVITY STREAM<small>STRUCTURED EVENTS</small></div></div>
        <div class="activity-stream" id="activity-stream"></div>
      </aside>
    </section>

    <footer class="command-deck">
      <form id="command-form" class="command-line">
        <span>MANUAL INPUT</span>
        <input id="command-input" autocomplete="off" placeholder="Enter a simulated instruction..." />
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
  </main>
`;
