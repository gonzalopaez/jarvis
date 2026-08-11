#!/usr/bin/env bash
set -euo pipefail

if ! command -v git >/dev/null 2>&1; then
  printf '%s\n' 'ERROR|git is required for the repository secret scan'
  exit 2
fi

blocking=0
advisory=0

# scan <severity> <category> <pattern>
#   severity=block  -> a match fails the scan (real embedded-secret shapes)
#   severity=warn   -> a match is reported as ADVISORY only (heuristic; false-positive prone)
scan() {
  local severity="$1"
  local category="$2"
  local pattern="$3"
  local matches

  matches="$(
    git grep -I -l -E -e "$pattern" -- \
      . \
      ':(exclude)scripts/secret-scan.sh' \
      ':(exclude)**/node_modules/**' \
      ':(exclude)**/dist/**' \
      ':(exclude)**/target/**' \
      2>/dev/null || true
  )"

  if [[ -n "$matches" ]]; then
    local label
    if [[ "$severity" == block ]]; then
      blocking=1
      label='FINDING'
    else
      advisory=1
      label='ADVISORY'
    fi
    while IFS= read -r file; do
      printf '%s|%s|%s\n' "$label" "$category" "$file"
    done <<<"$matches"
  fi
}

# High-confidence embedded-secret shapes: these fail the scan.
scan block 'private-key-material' \
  'BEGIN (OPENSSH|RSA|EC|DSA|PGP|ENCRYPTED )?PRIVATE KEY'
scan block 'known-api-token-format' \
  '(sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,})'
scan block 'bearer-value' \
  'Bearer[[:space:]]+[A-Za-z0-9._~+/-]{12,}'
scan block 'credentialed-uri' \
  '(postgres|postgresql|mysql|mongodb|redis|amqp)://[^[:space:]/:@]+:[^[:space:]@]+@'
# A credential keyword assigned a quoted string literal of >=8 chars. Requiring a
# quoted literal avoids flagging type declarations, function signatures, file/env
# reads and `.repeat()` test fixtures while still catching a hard-coded secret.
scan block 'credential-literal' \
  '(password|passwd|api[_-]?key|secret|token|master[_-]?key|client[_-]?secret|access[_-]?key)[[:space:]]*[:=][[:space:]]*["'"'"'][^"'"'"']{8,}["'"'"']'

# Heuristic, high false-positive categories: reported but non-blocking. This is an
# internal-infrastructure repository that legitimately references RFC1918 addresses.
scan warn 'private-ip' \
  '(^|[^0-9])((10|127)\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}|192\.168\.[0-9]{1,3}\.[0-9]{1,3}|172\.(1[6-9]|2[0-9]|3[01])\.[0-9]{1,3}\.[0-9]{1,3})([^0-9]|$)'

if [[ "$blocking" -ne 0 ]]; then
  printf '%s\n' 'SECRET_SCAN|blocked'
  exit 1
fi

if [[ "$advisory" -ne 0 ]]; then
  printf '%s\n' 'SECRET_SCAN|clean (advisories above are non-blocking)'
else
  printf '%s\n' 'SECRET_SCAN|clean'
fi
