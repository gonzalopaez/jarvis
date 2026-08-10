export function securityTelemetryPanel(): string {
  return `<section class="security-telemetry" aria-labelledby="security-telemetry-title">
    <div class="section-heading compact"><span>02</span><div id="security-telemetry-title">SECURITY TELEMETRY<small>WAZUH DATA PLANE</small></div></div>
    <div class="security-source"><i></i><span>SECURITY SOURCE</span><strong id="security-source-state">OFFLINE</strong></div>
    <div class="security-grid" id="security-telemetry-grid">
      ${securityRow("auth", "FAILED LOGIN ATTEMPTS", "24H", "security-failed-logins")}
      ${securityRow("sudo", "SUDO COMMANDS", "24H", "security-sudo")}
      ${securityRow("process", "NEW PROCESSES", "1H", "security-processes")}
      ${securityRow("file", "FILE INTEGRITY CHANGES", "24H", "security-fim")}
      ${securityRow("network", "NETWORK CONNECTIONS", "NOW", "security-connections")}
      ${securityRow("inbound", "INBOUND CONNECTIONS", "NOW", "security-inbound")}
      ${securityRow("outbound", "OUTBOUND CONNECTIONS", "NOW", "security-outbound")}
      ${securityRow("user", "PRIVILEGED USERS ONLINE", "NOW", "security-users")}
      ${securityRow("port", "LISTENING PORTS", "SYSTEM", "security-ports")}
      ${securityRow("load", "SYSTEM LOAD AVERAGE", "5M", "security-load")}
      ${securityRow("read", "DISK READ RATE", "5M", "security-disk-read")}
      ${securityRow("write", "DISK WRITE RATE", "5M", "security-disk-write")}
      ${securityRow("alert", "ALERTS", "24H", "security-alert-count")}
    </div>
  </section>`;
}

function securityRow(icon: string, label: string, period: string, id: string): string {
  return `<div class="security-row"><i class="security-icon icon-${icon}"></i><span>${label}<small>${period}</small></span><strong id="${id}">--</strong><svg class="security-sparkline" viewBox="0 0 80 18" aria-hidden="true"><path id="${id}-spark" d="M0 9H80"/></svg></div>`;
}

export function securityAlertsPanel(): string {
  return `<section class="security-alerts" aria-labelledby="security-alerts-title">
    <div class="section-heading compact"><span>04</span><div id="security-alerts-title">SECURITY ALERTS<small>RECENT THREATS</small></div></div>
    <div class="alerts-empty" id="security-alerts-empty"><i></i><strong>NO DATA SOURCE</strong><span>Wazuh adapter is not connected.</span></div>
    <div class="security-alert-list" id="security-alert-list"></div>
    <button type="button" class="view-alerts" disabled>VIEW ALL ALERTS</button>
  </section>`;
}
