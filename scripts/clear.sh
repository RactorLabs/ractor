#!/usr/bin/env bash

# Clears and recreates the default MySQL database used by the TSBX dev stack.
# Connects via docker exec to the running MySQL container, drops the database, and creates it fresh.

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
  echo "        Start your MySQL container first (e.g., via Docker compose) before running this script." >&2
  exit 1
fi

docker exec "${MYSQL_CONTAINER}" \
  mysql -u"${MYSQL_USER}" -p"${MYSQL_PASSWORD}" \
  -e "DROP DATABASE IF EXISTS \`${MYSQL_DATABASE}\`; CREATE DATABASE \`${MYSQL_DATABASE}\` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

echo "[SUCCESS] Dropped and re-created '${MYSQL_DATABASE}' on container '${MYSQL_CONTAINER}'."
