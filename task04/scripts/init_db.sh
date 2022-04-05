#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: sqlx is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install sqlx-cli --no-default-features --features sqlite"
  echo >&2 "to install it."
  exit 1
fi

export DATABASE_URL=sqlite:task04.db
sqlx database create
sqlx migrate run

>&2 echo "sqlite has been migrated, ready to go!"
