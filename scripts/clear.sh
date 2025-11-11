#!/usr/bin/env bash

# Clears all data from the default MySQL database used by the TSBX dev stack.
# Connects via docker exec to the running MySQL container and truncates every table.

set -euo pipefail

MYSQL_CONTAINER="${MYSQL_CONTAINER:-mysql}"
MYSQL_DATABASE="${MYSQL_DATABASE:-tsbx}"
MYSQL_USER="${MYSQL_USER:-root}"
MYSQL_PASSWORD="${MYSQL_PASSWORD:-root}"

if ! command -v docker >/dev/null 2>&1; then
  echo "[ERROR] docker is not installed or not on PATH." >&2
  exit 1
fi

if ! docker ps --format '{{.Names}}' | grep -Fxq "${MYSQL_CONTAINER}"; then
  echo "[ERROR] MySQL container '${MYSQL_CONTAINER}' is not running." >&2
  echo "        Start it with 'tsbx start mysql' before running this script." >&2
  exit 1
fi

tables="$(docker exec "${MYSQL_CONTAINER}" \
  mysql -u"${MYSQL_USER}" -p"${MYSQL_PASSWORD}" \
  -N -B -e "SELECT table_name FROM information_schema.tables WHERE table_schema = '${MYSQL_DATABASE}' AND table_type = 'BASE TABLE';")"

if [[ -z "${tables}" ]]; then
  echo "[INFO] No tables found in database '${MYSQL_DATABASE}'. Nothing to clear."
  exit 0
fi

truncate_sql="SET FOREIGN_KEY_CHECKS = 0; "
while IFS= read -r table; do
  [[ -z "${table}" ]] && continue
  truncate_sql+="TRUNCATE TABLE \`${table}\`; "
done <<< "${tables}"
truncate_sql+="SET FOREIGN_KEY_CHECKS = 1;"

docker exec "${MYSQL_CONTAINER}" \
  mysql -u"${MYSQL_USER}" -p"${MYSQL_PASSWORD}" \
  --database "${MYSQL_DATABASE}" \
  -e "${truncate_sql}"

echo "[SUCCESS] Cleared all tables in '${MYSQL_DATABASE}' on container '${MYSQL_CONTAINER}'."
