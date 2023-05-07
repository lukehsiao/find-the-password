#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v sqlite3)" ]; then
  echo >&2 "Error: sqlite3 is not installed."
  exit 1
fi

if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: sqlx is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install sqlx-cli --no-default-features --features runtime-tokio-rustls,sqlite,chrono"
  echo >&2 "to install it."
  exit 1
fi

DB="${SQLITE_DB:=challenges.db}"

export DATABASE_URL=sqlite://${DB}
sqlx database create
sqlx migrate run

>&2 echo "sqlite has been migrated, ready to go!"
