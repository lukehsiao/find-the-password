#!/bin/sh

if [ $# != 1 ]; then
    echo "Usage: $(basename "$0") <task-number>" >&2
    exit 1
fi
if [ ! -d .git ]; then
    echo "must be run from root of the repository" >&2
    exit 1
fi

name="$(printf "task%02d" "$1")"
cargo new --bin "$name"
touch "$name/README.md"
