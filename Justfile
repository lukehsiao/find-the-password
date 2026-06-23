# just manual: https://github.com/casey/just

_default:
	@just --list

# Build a docker image locally with tag (default: local-dev)
[group('packaging')]
image tag="local":
    podman build --tag find-the-password:{{tag}} --file Containerfile {{justfile_directory()}}

# Runs linters on the sources
[group('dev')]
check:
	cargo clippy --all-features --all-targets --locked -- -D warnings
	zizmor .

# Check links in markdown files
[group('dev')]
link-check:
	-lychee -E '**/*.md'

# Runs nextest
[group('dev')]
test:
	cargo nextest run

# Generate a coverage report via llvm-cov + nextest
[group('dev')]
coverage:
	cargo llvm-cov nextest

# Run the end-to-end (Playwright) tests; builds the app and starts the server
[group('dev')]
e2e:
	cargo leptos end-to-end

# Run the release binary
[group('dev')]
run addr="0.0.0.0:3000":
	LEPTOS_SITE_ADDR={{addr}} {{justfile_directory()}}/app/challenge

# Build the release binary
[group('dev')]
build:
	cargo leptos build --release
	mkdir -p {{justfile_directory()}}/app
	cp {{justfile_directory()}}/target/release/challenge {{justfile_directory()}}/app/
	cp -r {{justfile_directory()}}/target/site/* {{justfile_directory()}}/app/site
	cp {{justfile_directory()}}/Cargo.toml {{justfile_directory()}}/app/

# Format all sources, leptos-style
[group('dev')]
fmt:
	leptosfmt {{justfile_directory()}}

# Sets up a watcher that lints, tests, and builds
[group('dev')]
watch:
	bacon

# Install release tooling
[group('build')]
install:
	pnpm install

# Interactively create a changeset.
[group('release')]
changeset *args:
	pnpm changeset {{ args }}

# Sync version from package.json to Cargo manifest
_sync-versions:
	#!/usr/bin/env bash
	set -euxo pipefail

	# read version from package.json
	version=$(jaq -r '.version' package.json)

	# ensure we found a version
	[ -n "$version" ]
	# replace a version line that starts at column 1: version = "..."
	sd '^version\s+=\s+".*"$' "version = \"$version\"" Cargo.toml
	cargo generate-lockfile
	echo "Cargo.toml version set to $version"

# Append git-stats to the latest CHANGELOG entry
_append-git-stats:
	#!/usr/bin/env bash
	set -euo pipefail

	version=$(jaq -r '.version' package.json)
	prev_tag=$(git describe --tags --abbrev=0 2>/dev/null || true)

	if [ -z "$prev_tag" ]; then
	    echo "No previous tag found, skipping git-stats"
	    exit 0
	fi

	if ! command -v git-stats &> /dev/null; then
	    echo "Warning: git-stats not found, skipping"
	    exit 0
	fi

	if ! grep -q "^## ${version}$" CHANGELOG.md; then
	    echo "Warning: '## ${version}' not found in CHANGELOG.md, skipping"
	    exit 0
	fi

	stats=$(git-stats "${prev_tag}..HEAD")

	# Find the new version header line number
	version_line=$(grep -n "^## ${version}$" CHANGELOG.md | head -1 | cut -d: -f1)

	# Find the next section boundary (## or ---) after it
	next_section=$(tail -n "+$((version_line + 1))" CHANGELOG.md \
	    | grep -n "^## \|^---$" \
	    | head -1 \
	    | cut -d: -f1)

	if [ -n "$next_section" ]; then
	    insert_at=$((version_line + next_section - 1))
	else
	    insert_at=$(wc -l < CHANGELOG.md)
	fi

	# Build the stats block (HTML pre tag survives changesets processing)
	stats_block=$(printf '<pre>\n$ git-stats %s..v%s\n%s\n</pre>' "$prev_tag" "$version" "$stats")

	# Insert into CHANGELOG.md
	{
	    head -n "$insert_at" CHANGELOG.md
	    echo "$stats_block"
	    echo
	    tail -n "+$((insert_at + 1))" CHANGELOG.md
	} > CHANGELOG.md.tmp
	mv CHANGELOG.md.tmp CHANGELOG.md

	echo "Added git-stats to CHANGELOG.md for v${version}"

# Create a version bump
[group('release')]
version *args:
	pnpm changeset version {{ args }}
	just _sync-versions
	just _append-git-stats

# Tag a new version; the container workflow publishes images from main
[group('release')]
publish:
	pnpm changeset publish

# Show pending changesets and expected version bumps.
[group('release')]
status *args:
	pnpm changeset status {{ args }}
