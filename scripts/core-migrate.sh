#!/usr/bin/env bash
set -euo pipefail

if [[ "${JARVIS_ALLOW_CORE_MIGRATIONS:-}" != "YES" ]]; then
  echo "Refusing: set JARVIS_ALLOW_CORE_MIGRATIONS=YES after reviewed approval." >&2
  exit 2
fi
if [[ -z "${JARVIS_CORE_MIGRATION_DATABASE_URL:-}" || -z "${JARVIS_CORE_MIGRATION_EXPECTED_DATABASE:-}" ]]; then
  echo "Refusing: explicit Core migration URL and expected database are required." >&2
  exit 2
fi
if [[ "${JARVIS_CORE_MIGRATION_EXPECTED_DATABASE}" == "jarvis_soc" ]]; then
  echo "Refusing: Core migrations cannot target jarvis_soc." >&2
  exit 2
fi

migration_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../services/core/core-migrations" && pwd)"
database_name="$(psql "${JARVIS_CORE_MIGRATION_DATABASE_URL}" -X -Atqc 'select current_database()')"
if [[ "${database_name}" != "${JARVIS_CORE_MIGRATION_EXPECTED_DATABASE}" ]]; then
  echo "Refusing: target database does not match the explicit expected database." >&2
  exit 2
fi

for migration_file in "${migration_root}"/[0-9][0-9][0-9][0-9]_*.sql; do
  version="$(basename "${migration_file}" .sql)"
  checksum="$(sha256sum "${migration_file}" | awk '{print $1}')"
  history_exists="$(psql "${JARVIS_CORE_MIGRATION_DATABASE_URL}" -X -Atqc "select to_regclass('jarvis_core.schema_migrations') is not null")"
  existing=""
  if [[ "${history_exists}" == "t" ]]; then
    existing="$(psql "${JARVIS_CORE_MIGRATION_DATABASE_URL}" -X -v ON_ERROR_STOP=1 -Atqc "select checksum_sha256 from jarvis_core.schema_migrations where version = '${version}'")"
  fi
  if [[ -n "${existing}" ]]; then
    if [[ "${existing}" != "${checksum}" ]]; then
      echo "Refusing: checksum mismatch for ${version}." >&2
      exit 3
    fi
    continue
  fi
  psql "${JARVIS_CORE_MIGRATION_DATABASE_URL}" -X -v ON_ERROR_STOP=1 \
    -v migration_file="${migration_file}" -v version="${version}" -v checksum="${checksum}" <<'SQL'
BEGIN;
SELECT pg_advisory_xact_lock(12415001);
\i :migration_file
INSERT INTO jarvis_core.schema_migrations(version, checksum_sha256)
VALUES (:'version', :'checksum');
COMMIT;
SQL
done
