export function stateVoicePanel(): string {
  return `<section class="voice-panel" id="voice-panel" aria-live="polite">
    <div class="voice-copy">
      <span>VOICE // STATE CHANNEL</span>
      <canvas id="wave-canvas" width="900" height="108" aria-label="Visualizador contextual de voz"></canvas>
      <strong id="voice-state">STANDBY</strong>
      <p id="voice-context">Awaiting authenticated voice stream.</p>
      <div class="wave-label"><span id="wave-mode">SIGNAL // AMBIENT</span><b id="wave-level">-- dB</b></div>
    </div>
    <button class="voice-orb" id="microphone-toggle" type="button" aria-label="Start microphone capture" aria-pressed="false"><i></i><svg viewBox="0 0 24 24"><path d="M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3Zm-6 9a6 6 0 0 0 12 0M12 18v3M8 21h8"/></svg></button>
  </section>`;
}

export function jarvisCore(): string {
  return `<div class="core-shell" aria-label="Núcleo dinámico de estado JARVIS">
    <div class="core-crosshair"></div>
    <div class="orbit orbit-1"><i></i><i></i><i></i></div><div class="orbit orbit-2"><i></i><i></i></div><div class="orbit orbit-3"></div>
    <svg class="core-svg" viewBox="0 0 600 600" aria-hidden="true">
      <defs><filter id="soft-glow"><feGaussianBlur stdDeviation="3" result="blur"/><feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
      <circle class="radar-field" cx="300" cy="300" r="274"/>
      <circle class="ring ring-a" cx="300" cy="300" r="248"/><circle class="ring ring-b" cx="300" cy="300" r="218"/><circle class="ring ring-c" cx="300" cy="300" r="180"/><circle class="ring ring-d" cx="300" cy="300" r="142"/>
      <circle class="functional-ring processing-ring" cx="300" cy="300" r="124"/><circle class="functional-ring routing-ring" cx="300" cy="300" r="204"/><circle class="functional-ring execution-ring" cx="300" cy="300" r="231"/><circle class="functional-ring telemetry-ring" cx="300" cy="300" r="268"/>
      <circle class="ticks ticks-outer" cx="300" cy="300" r="262"/><circle class="ticks ticks-inner" cx="300" cy="300" r="164"/>
      <path class="arc arc-a" d="M 103 300 A 197 197 0 0 1 203 130"/><path class="arc arc-b" d="M 467 196 A 197 197 0 0 1 487 362"/><path class="arc arc-c" d="M 217 452 A 175 175 0 0 1 131 340"/>
      <g class="radar-sweep"><path d="M300 300 L300 37 A263 263 0 0 1 388 52 Z"/></g>
      <g class="radial-signals"><circle cx="300" cy="300" r="74"/><circle cx="300" cy="300" r="74"/><circle cx="300" cy="300" r="74"/></g>
    </svg>
    <div class="core-heart"><div class="heart-lattice"></div><div class="heart-ring"></div><div class="heart-energy"></div><div class="heart-mark">J</div></div>
    <div class="orbital-node node-a"><span>01</span></div><div class="orbital-node node-b"><span>07</span></div><div class="orbital-node node-c"><span>42</span></div>
  </div>`;
}

export function coreStateAndTranscript(): string {
  return `<div class="state-banner"><span class="state-index">CORE STATE</span><strong id="state-label">SYSTEM // STANDBY</strong><i></i></div>
    <div class="transcript-module"><div class="transcript-line"><span>USER</span><p id="user-transcript">Awaiting operator input.</p></div><div class="transcript-line jarvis-line"><span>JARVIS</span><p id="jarvis-transcript">Local system interface initialized.</p></div><div class="core-interaction-stream" id="core-interaction-stream"></div></div>`;
}
