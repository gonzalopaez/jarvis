#!/usr/bin/env bash
set -euo pipefail

if [[ "${JARVIS_ALLOW_SOC_MIGRATIONS:-}" != "YES" ]]; then
  echo "Refusing: set JARVIS_ALLOW_SOC_MIGRATIONS=YES after reviewed approval." >&2
  exit 2
fi
if [[ -z "${JARVIS_SOC_MIGRATION_DATABASE_URL:-}" ]]; then
  echo "Refusing: JARVIS_SOC_MIGRATION_DATABASE_URL is required." >&2
  exit 2
fi
if [[ -z "${JARVIS_SOC_MIGRATION_EXPECTED_DATABASE:-}" ]]; then
  echo "Refusing: JARVIS_SOC_MIGRATION_EXPECTED_DATABASE is required." >&2
  exit 2
fi

migration_root="$(cd "${JARVIS_SOC_MIGRATION_ROOT:-$(dirname "${BASH_SOURCE[0]}")/../services/core/migrations}" && pwd)"
database_name="$(psql "${JARVIS_SOC_MIGRATION_DATABASE_URL}" -X -Atqc 'select current_database()')"
if [[ "${database_name}" != "${JARVIS_SOC_MIGRATION_EXPECTED_DATABASE}" ]]; then
  echo "Refusing: target database does not match the explicit expected database." >&2
  exit 2
fi

for migration_file in "${migration_root}"/[0-9][0-9][0-9][0-9]_*.sql; do
  version="$(basename "${migration_file}" .sql)"
  checksum="$(sha256sum "${migration_file}" | awk '{print $1}')"
  history_exists="$(psql "${JARVIS_SOC_MIGRATION_DATABASE_URL}" -X -Atqc "select to_regclass('public.jarvis_schema_migrations') is not null")"
  existing=""
  if [[ "${history_exists}" == "t" ]]; then
    existing="$(psql "${JARVIS_SOC_MIGRATION_DATABASE_URL}" -X -v ON_ERROR_STOP=1 -Atqc "select checksum_sha256 from public.jarvis_schema_migrations where version = '${version}'")"
  fi
  if [[ -n "${existing}" ]]; then
    if [[ "${existing}" != "${checksum}" ]]; then
      echo "Refusing: checksum mismatch for ${version}." >&2
      exit 3
    fi
    continue
  fi
  psql "${JARVIS_SOC_MIGRATION_DATABASE_URL}" -X -v ON_ERROR_STOP=1 \
    -v migration_file="${migration_file}" -v version="${version}" -v checksum="${checksum}" <<'SQL'
BEGIN;
SELECT pg_advisory_xact_lock(12413302);
\i :migration_file
INSERT INTO public.jarvis_schema_migrations(version, description, checksum_sha256)
VALUES (:'version', :'version', :'checksum');
COMMIT;
SQL
done
