#!/usr/bin/env bash
set -euo pipefail

if ! command -v git >/dev/null 2>&1; then
  printf '%s\n' 'ERROR|git is required for the repository secret scan'
  exit 2
fi

findings=0

scan() {
  local category="$1"
  local pattern="$2"
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
    findings=1
    while IFS= read -r file; do
      printf 'FINDING|%s|%s\n' "$category" "$file"
    done <<<"$matches"
  fi
}

scan 'private-key-material' \
  'BEGIN (OPENSSH|RSA|EC|DSA|PGP|ENCRYPTED )?PRIVATE KEY'
scan 'known-api-token-format' \
  '(sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,})'
scan 'bearer-value' \
  'Bearer[[:space:]]+[A-Za-z0-9._~+/-]{12,}'
scan 'credential-assignment' \
  '(password|passwd|api[_-]?key|secret|token|master[_-]?key|client[_-]?secret|access[_-]?key)[[:space:]]*[:=][[:space:]]*[^[:space:]$<{][^[:space:]]{5,}'
scan 'credentialed-uri' \
  '(postgres|postgresql|mysql|mongodb|redis|amqp)://[^[:space:]/:@]+:[^[:space:]@]+@'
scan 'private-ip' \
  '(^|[^0-9])((10|127)\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}|192\.168\.[0-9]{1,3}\.[0-9]{1,3}|172\.(1[6-9]|2[0-9]|3[01])\.[0-9]{1,3}\.[0-9]{1,3})([^0-9]|$)'

if [[ "$findings" -ne 0 ]]; then
  printf '%s\n' 'SECRET_SCAN|blocked'
  exit 1
fi

printf '%s\n' 'SECRET_SCAN|clean'
